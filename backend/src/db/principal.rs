//! Principal identity graph repository (#126).
//!
//! Owns `memory_principals` + `principal_aliases`, the merge/unmerge redirects
//! between anonymous and person principals, and the audit trail both directions
//! must write. Resolution follows one-hop merges to a bounded depth so callers
//! always land on the current owner of a memory.
//!
//! Merge invariants (each enforced here before any row changes):
//! 1. only an **anonymous** principal is merged, into an **active person**;
//! 2. neither side may already sit inside a merge chain segment that would loop
//!    back through the other (depth-capped walk);
//! 3. the mutation, its compensation `system_event` in `memory_events`, and its
//!    audit rows in `memory_audit_events` all commit in ONE transaction —
//!    no half-merged state can ever be observed;
//! 4. unmerge is the exact inverse and writes its own event + audit rows.

use ulid::Ulid;

use crate::db::audit::AuditEvent;
use crate::db::tenant_scope::begin_tenant_tx;
use crate::error::AppError;
use crate::models::memory_event::MemoryEventType;
use crate::models::principal::{PrincipalAliasType, PrincipalKind, PrincipalStatus};
use crate::tenant::TenantId;
use sqlx::{PgPool, Row};

/// Longest accepted `merged_into_id` chain. Real merges are one-hop; the cap
/// exists so a corrupt chain fails loudly instead of hanging resolution.
pub const MAX_MERGE_DEPTH: usize = 8;

/// Result of [`PrincipalRepository::ensure_with_alias`].
#[derive(Debug)]
pub struct EnsureOutcome {
    pub principal: MemoryPrincipalRow,
    /// True when this call created the principal (and alias); false when it was
    /// resolved from an existing alias.
    pub created: bool,
}

pub use crate::models::principal::MemoryPrincipal as MemoryPrincipalRow;

/// Audit event-type markers for merge/unmerge (kept here so tests and the
/// service layer share one spelling).
pub const AUDIT_EVENT_MERGED: &str = "identity.principal_merged";
pub const AUDIT_EVENT_UNMERGED: &str = "identity.principal_unmerged";

const PRINCIPAL_COLS: &str = "id, tenant_id, kind, display_name, status, merged_into_id, \
     created_at::text AS created_at, updated_at::text AS updated_at";

/// Same columns as [`PRINCIPAL_COLS`], qualified for joins against aliases.
const PRINCIPAL_COLS_JOINED: &str = "p.id, p.tenant_id, p.kind, p.display_name, p.status, \
     p.merged_into_id, p.created_at::text AS created_at, p.updated_at::text AS updated_at";

pub struct PrincipalRepository {
    pool: PgPool,
}

impl PrincipalRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Creation & lookup ─────────────────────────────────────────────────── //

    /// Create a bare principal.
    pub async fn create(
        &self,
        tenant_id: &TenantId,
        kind: PrincipalKind,
        display_name: Option<&str>,
    ) -> Result<MemoryPrincipalRow, AppError> {
        let id = Ulid::new().to_string();
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let row = sqlx::query_as::<_, MemoryPrincipalRow>(&format!(
            r#"INSERT INTO memory_principals (id, tenant_id, kind, display_name)
               VALUES ($1, $2, $3, $4)
               RETURNING {PRINCIPAL_COLS}"#
        ))
        .bind(&id)
        .bind(tenant_id.as_str())
        .bind(kind.as_str())
        .bind(display_name)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to insert memory_principal: {e}")))?;
        tx.commit().await.ok();
        Ok(row)
    }

    /// Resolve by alias, creating the principal + alias when absent.
    ///
    /// This is THE entry point for "已认证 JWT 用户自动映射到 person principal":
    /// the same `(alias_type, value)` pair always lands on the same principal,
    /// across sessions and devices.
    ///
    /// Returns [`EnsureOutcome`] with `created=false` on existing resolution.
    pub async fn ensure_with_alias(
        &self,
        tenant_id: &TenantId,
        kind: PrincipalKind,
        display_name: Option<&str>,
        alias_type: PrincipalAliasType,
        alias_value: &str,
    ) -> Result<EnsureOutcome, AppError> {
        if alias_value.is_empty() {
            return Err(AppError::BadRequest(
                "alias_value must not be empty".to_string(),
            ));
        }
        // Structural guard mirrored from the model layer: never attach a device
        // key to a person or vice versa (shared-device no-auto-merge rule).
        if !alias_type.may_attach_to_kind(kind) {
            return Err(AppError::BadRequest(format!(
                "alias type '{}' cannot attach to a '{}' principal",
                alias_type.as_str(),
                kind.as_str()
            )));
        }

        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        if let Some(existing) =
            Self::find_by_alias_in(&mut *tx, tenant_id, alias_type, alias_value).await?
        {
            tx.commit().await.ok();
            return Ok(EnsureOutcome {
                principal: existing,
                created: false,
            });
        }

        let principal = Self::insert_principal_in(&mut *tx, tenant_id, kind, display_name).await?;
        sqlx::query("INSERT INTO principal_aliases (id, tenant_id, principal_id, alias_type, alias_value) VALUES ($1,$2,$3,$4,$5)")
            .bind(Ulid::new().to_string())
            .bind(tenant_id.as_str())
            .bind(principal.id.clone())
            .bind(alias_type.as_str())
            .bind(alias_value)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to insert principal_alias: {e}") ))?;
        tx.commit().await.ok();
        Ok(EnsureOutcome {
            principal,
            created: true,
        })
    }

    /// One principal by id; cross-tenant ids fail closed to `None`.
    pub async fn get(
        &self,
        tenant_id: &TenantId,
        principal_id: &str,
    ) -> Result<Option<MemoryPrincipalRow>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let row = sqlx::query_as::<_, MemoryPrincipalRow>(&format!(
            "SELECT {PRINCIPAL_COLS} FROM memory_principals WHERE tenant_id = $1 AND id = $2"
        ))
        .bind(tenant_id.as_str())
        .bind(principal_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get memory_principal: {e}")))?;
        tx.commit().await.ok();
        Ok(row)
    }

    /// Resolve which principal owns an alias today.
    pub async fn find_by_alias(
        &self,
        tenant_id: &TenantId,
        alias_type: PrincipalAliasType,
        alias_value: &str,
    ) -> Result<Option<MemoryPrincipalRow>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let row = Self::find_by_alias_in(&mut *tx, tenant_id, alias_type, alias_value).await?;
        tx.commit().await.ok();
        Ok(row)
    }

    async fn find_by_alias_in<'a, E>(
        executor: E,
        _tenant_id: &TenantId,
        alias_type: PrincipalAliasType,
        alias_value: &str,
    ) -> Result<Option<MemoryPrincipalRow>, AppError>
    where
        E: sqlx::PgExecutor<'a>,
    {
        sqlx::query_as::<_, MemoryPrincipalRow>(&format!(
            r#"
            SELECT {PRINCIPAL_COLS_JOINED}
            FROM memory_principals p
            JOIN principal_aliases a ON a.principal_id = p.id
            WHERE a.tenant_id = p.tenant_id
              AND a.alias_type = $1 AND a.alias_value = $2
            "#,
        ))
        .bind(alias_type.as_str())
        .bind(alias_value)
        .fetch_optional(executor)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to resolve principal alias: {e}")))
    }

    async fn insert_principal_in<'a, E>(
        executor: E,
        tenant_id: &TenantId,
        kind: PrincipalKind,
        display_name: Option<&str>,
    ) -> Result<MemoryPrincipalRow, AppError>
    where
        E: sqlx::PgExecutor<'a>,
    {
        let id = Ulid::new().to_string();
        sqlx::query_as::<_, MemoryPrincipalRow>(&format!(
            r#"
            INSERT INTO memory_principals (id, tenant_id, kind, display_name)
            VALUES ($1, $2, $3, $4)
            RETURNING {PRINCIPAL_COLS}
            "#,
        ))
        .bind(id)
        .bind(tenant_id.as_str())
        .bind(kind.as_str())
        .bind(display_name)
        .fetch_one(executor)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to insert memory_principal: {e}")))
    }

    // ── Resolution ────────────────────────────────────────────────────────── //

    /// Follow `merged_into_id` links until a non-redirecting principal.
    ///
    /// Returns the root plus the number of hops taken. Cycles and chains deeper
    /// than [`MAX_MERGE_DEPTH`] are hard errors — with single-hop merges they
    /// indicate data corruption, not a busy day.
    pub async fn follow_merge_chain(
        &self,
        tenant_id: &TenantId,
        start: MemoryPrincipalRow,
    ) -> Result<(MemoryPrincipalRow, usize), AppError> {
        let mut current = start;
        let mut hops = 0usize;
        while current.is_redirecting() {
            if hops >= MAX_MERGE_DEPTH {
                return Err(AppError::Internal(format!(
                    "merge chain exceeds depth {} starting at principal {}",
                    MAX_MERGE_DEPTH, current.id
                )));
            }
            hops += 1;
            let target = current.merged_into_id.clone().ok_or_else(|| {
                AppError::Internal("redirecting principal without target".to_string())
            })?;
            current = self.get(tenant_id, &target).await?.ok_or_else(|| {
                AppError::Internal(format!(
                    "merge target '{target}' vanished for tenant '{}'",
                    tenant_id.as_str()
                ))
            })?;
        }
        Ok((current, hops))
    }

    // ── Merge / unmerge ───────────────────────────────────────────────────── //

    /// Explicitly merge an anonymous principal into a person principal.
    ///
    /// One transaction performs: validation under `FOR UPDATE`, the redirect +
    /// status flip, a `system_event` row in `memory_events`, and audit rows in
    /// `memory_audit_events`. Reversible via [`Self::unmerge_anonymous`].
    #[allow(clippy::too_many_arguments)]
    pub async fn merge_anonymous(
        &self,
        tenant_id: &TenantId,
        anonymous_id: &str,
        person_id: &str,
        actor: Option<&str>,
    ) -> Result<(MemoryPrincipalRow, String), AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        // Lock both rows (order: source first) so two racing merges serialize.
        let anon = sqlx::query_as::<_, MemoryPrincipalRow>(&format!(
            "SELECT {PRINCIPAL_COLS} FROM memory_principals WHERE id = $1 FOR UPDATE"
        ))
        .bind(anonymous_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("merge: read anonymous failed: {e}")))?
        .ok_or_else(|| {
            AppError::NotFound(format!("anonymous principal '{anonymous_id}' not found"))
        })?;
        if anon.kind() != Ok(PrincipalKind::Anonymous) {
            return Err(AppError::BadRequest(format!(
                "only 'anonymous' principals may be merged; '{}' has kind '{}'",
                anon.id, anon.kind
            )));
        }
        match anon.status() {
            Ok(PrincipalStatus::Deactivated) => {
                return Err(AppError::BadRequest(format!(
                    "principal '{}' is deactivated and cannot be merged",
                    anon.id
                )));
            }
            Ok(PrincipalStatus::Merged) => {
                return Err(AppError::BadRequest(format!(
                    "principal '{}' is already merged; unmerge it first",
                    anon.id
                )));
            }
            _ => {}
        }

        let person = sqlx::query_as::<_, MemoryPrincipalRow>(&format!(
            "SELECT {PRINCIPAL_COLS} FROM memory_principals WHERE id = $1 FOR UPDATE"
        ))
        .bind(person_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("merge: read person failed: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("person principal '{person_id}' not found")))?;
        if person.kind() != Ok(PrincipalKind::Person) {
            return Err(AppError::BadRequest(format!(
                "merge target must be a person principal; '{}' has kind '{}'",
                person.id, person.kind
            )));
        }
        if person.status != PrincipalStatus::Active.as_str() || person.merged_into_id.is_some() {
            return Err(AppError::BadRequest(format!(
                "merge target '{}' is not active (status '{}')",
                person.id, person.status
            )));
        }

        // Cycle guard: walking from the TARGET must never reach the SOURCE.
        Self::guard_no_cycle(&mut *tx, person_id, anonymous_id).await?;

        sqlx::query(
            "UPDATE memory_principals SET merged_into_id = $1, status = 'merged', updated_at = NOW() WHERE id = $2",
        )
        .bind(person_id)
        .bind(anonymous_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("merge: update failed: {e}")))?;

        // Compensation/lifecycle event in the immutable stream (same tx).
        let payload = serde_json::json!({
            "action": "principal_merged",
            "anonymous_id": anonymous_id,
            "person_id": person_id,
        });
        append_lifecycle_event_in(&mut *tx, tenant_id, anonymous_id, actor, &payload).await?;

        // Audit rows, same transaction so a crash cannot lose them.
        let audit_event = AuditEvent::new(AUDIT_EVENT_MERGED, "memory_principal")
            .tenant(tenant_id.as_str())
            .resource_id(anonymous_id)
            .with_metadata(&serde_json::json!({
                "anonymous_id": anonymous_id,
                "person_id": person_id,
                "actor": actor,
            }));
        crate::db::audit::insert_tx(&mut tx, &audit_event).await?;

        tx.commit().await.ok();
        Ok((
            MemoryPrincipalRow {
                merged_into_id: Some(person_id.to_string()),
                status: PrincipalStatus::Merged.as_str().to_string(),
                ..anon
            },
            person_id.to_string(),
        ))
    }

    /// Undo an explicit merge. The anonymous principal returns to `active`
    /// ownership of its aliases/events; history stays intact because events were
    /// never re-pointed — only the redirect pointer moves.
    pub async fn unmerge_anonymous(
        &self,
        tenant_id: &TenantId,
        anonymous_id: &str,
        actor: Option<&str>,
    ) -> Result<String, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        let anon = sqlx::query_as::<_, MemoryPrincipalRow>(&format!(
            "SELECT {PRINCIPAL_COLS} FROM memory_principals WHERE id = $1 FOR UPDATE"
        ))
        .bind(anonymous_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("unmerge: read failed: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("principal '{anonymous_id}' not found")))?;
        let previous_target = anon.merged_into_id.clone().ok_or_else(|| {
            AppError::BadRequest(format!(
                "principal '{}' is not merged (status '{}'); nothing to unmerge",
                anonymous_id, anon.status
            ))
        })?;
        if anon.kind() != Ok(PrincipalKind::Anonymous) {
            return Err(AppError::BadRequest(
                "only 'anonymous' principals may be unmerged".to_string(),
            ));
        }

        sqlx::query(
            "UPDATE memory_principals SET merged_into_id = NULL, status = 'active', updated_at = NOW() WHERE id = $1",
        )
        .bind(anonymous_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("unmerge: update failed: {e}")))?;

        let payload = serde_json::json!({
            "action": "principal_unmerged",
            "anonymous_id": anonymous_id,
            "previous_person_id": previous_target,
        });
        append_lifecycle_event_in(&mut *tx, tenant_id, anonymous_id, actor, &payload).await?;

        let audit_event = AuditEvent::new(AUDIT_EVENT_UNMERGED, "memory_principal")
            .tenant(tenant_id.as_str())
            .resource_id(anonymous_id)
            .with_metadata(&serde_json::json!({
                "anonymous_id": anonymous_id,
                "previous_person_id": previous_target,
                "actor": actor,
            }));
        crate::db::audit::insert_tx(&mut tx, &audit_event).await?;

        tx.commit().await.ok();
        Ok(previous_target)
    }

    /// Walk from `start_id` along `merged_into_id`; fail if `forbidden_id`
    /// appears (would create a cycle) or the chain exceeds the depth cap.
    async fn guard_no_cycle(
        conn: &mut sqlx::PgConnection,
        start_id: &str,
        forbidden_id: &str,
    ) -> Result<(), AppError> {
        let mut current = start_id.to_string();
        for _ in 0..MAX_MERGE_DEPTH {
            let next: Option<Option<String>> =
                sqlx::query_scalar("SELECT merged_into_id FROM memory_principals WHERE id = $1")
                    .bind(&current)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(|e| AppError::Internal(format!("cycle guard failed: {e}")))?;
            match next {
                None => return Ok(()), // chain ends at a normal principal
                Some(None) => return Ok(()),
                Some(Some(next_id)) => {
                    if next_id == forbidden_id {
                        return Err(AppError::BadRequest(format!(
                            "merging would create a cycle through '{forbidden_id}'"
                        )));
                    }
                    current = next_id;
                }
            }
        }
        Err(AppError::Internal(format!(
            "merge chain from '{start_id}' exceeds depth {MAX_MERGE_DEPTH}"
        )))
    }

    /// Aliases currently bound to a principal (diagnostics/tests helper).
    pub async fn list_aliases(
        &self,
        tenant_id: &TenantId,
        principal_id: &str,
    ) -> Result<Vec<(String, String)>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = sqlx::query(
            "SELECT alias_type, alias_value FROM principal_aliases WHERE tenant_id = $1 AND principal_id = $2 ORDER BY created_at",
        )
        .bind(tenant_id.as_str())
        .bind(principal_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("list_aliases failed: {e}")))?;
        tx.commit().await.ok();
        Ok(rows
            .iter()
            .map(|r| (r.get::<String, _>(0), r.get::<String, _>(1)))
            .collect())
    }
}

/// Insert the lifecycle `system_event` marking a merge/unmerge, inside the
/// caller's transaction (so the immutable stream and the mutation agree even
/// under crash/retry).
async fn append_lifecycle_event_in<'a, E>(
    executor: E,
    tenant_id: &TenantId,
    subject_principal_id: &str,
    actor: Option<&str>,
    payload: &serde_json::Value,
) -> Result<(), AppError>
where
    E: sqlx::PgExecutor<'a>,
{
    let payload_str = serde_json::to_string(payload).expect("lifecycle payloads are plain maps");
    sqlx::query(
        r#"
        INSERT INTO memory_events
            (id, tenant_id, principal_id, session_id, event_type, actor,
             content_hash, payload_json, occurred_at, recorded_at, idempotency_key)
        VALUES ($1, $2, $3, NULL, $4, $5, $6, $7::jsonb, NOW(), NOW(), NULL)
        "#,
    )
    .bind(Ulid::new().to_string())
    .bind(tenant_id.as_str())
    .bind(subject_principal_id)
    .bind(MemoryEventType::SystemEvent.as_str())
    .bind(actor)
    .bind(crate::db::memory_event::content_hash_for(&payload_str))
    .bind(&payload_str)
    .execute(executor)
    .await
    .map_err(|e| AppError::Internal(format!("failed to record lifecycle event: {e}")))?;
    Ok(())
}
