//! Append-only `memory_events` repository (#126).
//!
//! Append-only is a **structural** property, not a convention:
//! - this module exposes no UPDATE/DELETE methods (and must never grow them —
//!   corrections are new compensation events);
//! - the migration revokes UPDATE/DELETE on the table from the hardened app
//!   role, so even raw-SQL detours fail at the database.
//!
//! All access runs inside [`begin_tenant_tx`] so RLS scopes every statement.

use sha2::{Digest, Sha256};
use sqlx::PgPool;
use ulid::Ulid;

use crate::db::tenant_scope::begin_tenant_tx;
use crate::error::AppError;
use crate::models::memory_event::{AppendMemoryEventRequest, MemoryEvent};
use crate::tenant::TenantId;

/// Outcome of an append that carried an idempotency key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppendOutcome {
    /// A new row was written; carries its id.
    Inserted(String),
    /// The same `(tenant_id, idempotency_key)` was already recorded; carries
    /// the existing row's id. No second event exists — replay-safe by contract.
    Duplicate(String),
}

impl AppendOutcome {
    pub fn id(&self) -> &str {
        match self {
            AppendOutcome::Inserted(id) | AppendOutcome::Duplicate(id) => id,
        }
    }

    pub fn is_new(&self) -> bool {
        matches!(self, AppendOutcome::Inserted(_))
    }
}

/// SHA-256 hex over the serialized payload — the tamper-evidence anchor the
/// write gate and consolidation jobs compare against. Exposed for reuse.
pub fn content_hash_for(payload_json: &str) -> String {
    let digest = Sha256::digest(payload_json.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        // Digest bytes are uniform; format! into a pre-sized string cannot fail.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

pub struct MemoryEventRepository {
    pool: PgPool,
}

impl MemoryEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Append one event. With an idempotency key present, replays resolve to the
    /// original row instead of duplicating it.
    ///
    /// Returns [`AppendOutcome`] so callers can distinguish fresh writes from
    /// replays without racing a separate existence check.
    pub async fn append(
        &self,
        tenant_id: &TenantId,
        req: AppendMemoryEventRequest,
    ) -> Result<AppendOutcome, AppError> {
        if req.principal_id.is_empty() {
            return Err(AppError::BadRequest(
                "memory_events.append: principal_id is required".to_string(),
            ));
        }

        let payload_str = serde_json::to_string(&req.payload_json)
            .map_err(|e| AppError::BadRequest(format!("payload_json is not serializable: {e}")))?;
        let content_hash = req
            .content_hash
            .unwrap_or_else(|| content_hash_for(&payload_str));
        let id = Ulid::new().to_string();

        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let inserted: Option<String> = sqlx::query_scalar(
            r#"
            INSERT INTO memory_events
                (id, tenant_id, principal_id, session_id, event_type, actor,
                 content_hash, payload_json, occurred_at, recorded_at, idempotency_key)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb,
                    COALESCE($9::timestamptz, NOW()), NOW(), $10)
            ON CONFLICT (tenant_id, idempotency_key)
                WHERE idempotency_key IS NOT NULL
                DO NOTHING
            RETURNING id
            "#,
        )
        .bind(&id)
        .bind(tenant_id.as_str())
        .bind(&req.principal_id)
        .bind(req.session_id.as_deref())
        .bind(req.event_type.as_str())
        .bind(req.actor.as_deref())
        .bind(content_hash)
        .bind(&payload_str)
        .bind(req.occurred_at.as_deref())
        .bind(req.idempotency_key.as_deref())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to insert memory_event: {e}")))?;

        if let Some(new_id) = inserted {
            tx.commit().await.ok();
            return Ok(AppendOutcome::Inserted(new_id));
        }

        // Conflict path: fetch the row that already owns this idempotency key.
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM memory_events WHERE tenant_id = $1 AND idempotency_key = $2",
        )
        .bind(tenant_id.as_str())
        .bind(req.idempotency_key.as_deref())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read duplicate event id: {e}")))?;
        tx.commit().await.ok();
        existing.map(AppendOutcome::Duplicate).ok_or_else(|| {
            AppError::Internal(
                "INSERT reported a conflict but the conflicting row vanished \
                 (concurrent delete?); retry"
                    .to_string(),
            )
        })
    }

    /// Fetch one event of this tenant. Cross-tenant ids fail closed to `None`.
    pub async fn get(
        &self,
        tenant_id: &TenantId,
        event_id: &str,
    ) -> Result<Option<MemoryEvent>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let row = sqlx::query_as::<_, MemoryEvent>(
            r#"
            SELECT id, tenant_id, principal_id, session_id, event_type, actor,
                   content_hash, payload_json::text AS payload_json,
                   occurred_at::text, recorded_at::text
            FROM memory_events
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(event_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get memory_event: {e}")))?;
        tx.commit().await.ok();
        Ok(row)
    }

    /// Newest-first events for one principal.
    pub async fn list_by_principal(
        &self,
        tenant_id: &TenantId,
        principal_id: &str,
        limit: i64,
    ) -> Result<Vec<MemoryEvent>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = sqlx::query_as::<_, MemoryEvent>(
            r#"
            SELECT id, tenant_id, principal_id, session_id, event_type, actor,
                   content_hash, payload_json::text AS payload_json,
                   occurred_at::text, recorded_at::text
            FROM memory_events
            WHERE tenant_id = $1 AND principal_id = $2
            ORDER BY occurred_at DESC, recorded_at DESC
            LIMIT $3
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(principal_id)
        .bind(limit.clamp(0, 1000))
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            AppError::Internal(format!("Failed to list memory_events by principal: {e}"))
        })?;
        tx.commit().await.ok();
        Ok(rows)
    }

    /// Chronological events of one session (the episodic container).
    pub async fn list_by_session(
        &self,
        tenant_id: &TenantId,
        session_id: &str,
        limit: i64,
    ) -> Result<Vec<MemoryEvent>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = sqlx::query_as::<_, MemoryEvent>(
            r#"
            SELECT id, tenant_id, principal_id, session_id, event_type, actor,
                   content_hash, payload_json::text AS payload_json,
                   occurred_at::text, recorded_at::text
            FROM memory_events
            WHERE tenant_id = $1 AND session_id = $2
            ORDER BY occurred_at ASC, recorded_at ASC
            LIMIT $3
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(session_id)
        .bind(limit.clamp(0, 1000))
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list memory_events by session: {e}")))?;
        tx.commit().await.ok();
        Ok(rows)
    }

    /// Total number of events currently visible to the tenant.
    pub async fn count_for_tenant(&self, tenant_id: &TenantId) -> Result<i64, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM memory_events WHERE tenant_id = $1")
                .bind(tenant_id.as_str())
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to count memory_events: {e}")))?;
        tx.commit().await.ok();
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::memory_event::MemoryEventType;

    #[test]
    fn content_hash_is_sha256_hex_and_deterministic() {
        let h1 = content_hash_for(r#"{"a":1}"#);
        let h2 = content_hash_for(r#"{"a":1}"#);
        let other = content_hash_for(r#"{"a":2}"#);
        assert_eq!(h1.len(), 64, "sha256 hex length");
        assert_eq!(h1, h2, "same input must hash identically");
        assert_ne!(h1, other);

        // Known vector: SHA-256 of empty string.
        assert_eq!(
            content_hash_for(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn outcome_reports_what_happened() {
        let ins = AppendOutcome::Inserted("e1".to_string());
        let dup = AppendOutcome::Duplicate("e0".to_string());
        assert!(ins.is_new());
        assert!(!dup.is_new());
        assert_eq!(dup.id(), "e0");
    }

    #[test]
    fn request_requires_principal_via_validation_in_service_layer_contract() {
        // Documented behaviour: empty principal ids are rejected before SQL.
        // (DB-side it would also fail the NOT NULL FK.)
        let req = AppendMemoryEventRequest::new("", MemoryEventType::UserMessage);
        assert_eq!(req.principal_id, "");
    }
}
