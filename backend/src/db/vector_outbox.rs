//! Durable vector outbox repository (ADR-0002 / P1 W0.1).
//!
//! PostgreSQL-only. Events are inserted in the same transaction as LTM fact rows,
//! then claimed and applied asynchronously by [`crate::services::outbox_worker`].

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use tracing::error;
use ulid::Ulid;

use crate::AppError;

/// Outbox operation kind (matches `memory_vector_outbox.operation` CHECK).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboxOperation {
    Upsert,
    Delete,
}

impl OutboxOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "upsert",
            Self::Delete => "delete",
        }
    }

    fn parse(s: &str) -> Result<Self, AppError> {
        match s {
            "upsert" => Ok(Self::Upsert),
            "delete" => Ok(Self::Delete),
            other => Err(AppError::Internal(format!(
                "unknown outbox operation: {other}"
            ))),
        }
    }
}

/// Row claimed by a worker for delivery.
#[derive(Debug, Clone)]
pub struct OutboxEvent {
    pub event_id: String,
    pub tenant_id: String,
    pub entry_id: String,
    pub operation: OutboxOperation,
    pub payload_json: String,
    pub payload_hash: String,
    pub idempotency_key: String,
    pub attempt_count: i32,
}

/// Build upsert/delete idempotency keys (pure, unit-testable).
pub fn upsert_idempotency_key(entry_id: &str, payload_hash: &str) -> String {
    format!("upsert:{entry_id}:{payload_hash}")
}

pub fn delete_idempotency_key(entry_id: &str) -> String {
    format!("delete:{entry_id}")
}

/// Insert one outbox event inside an open transaction.
///
/// Duplicate `(tenant_id, idempotency_key)` is treated as success (idempotent retry).
pub async fn insert_event_tx(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: &str,
    entry_id: &str,
    operation: OutboxOperation,
    payload_json: &str,
    payload_hash: &str,
    idempotency_key: &str,
) -> Result<String, AppError> {
    let event_id = Ulid::new().to_string();
    let result = sqlx::query(
        r#"
        INSERT INTO memory_vector_outbox (
            event_id, tenant_id, entry_id, operation, payload_json, payload_hash,
            idempotency_key, status, attempt_count, next_retry_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', 0, CURRENT_TIMESTAMP)
        ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
        "#,
    )
    .bind(&event_id)
    .bind(tenant_id)
    .bind(entry_id)
    .bind(operation.as_str())
    .bind(payload_json)
    .bind(payload_hash)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await
    .map_err(|e| {
        error!("Failed to insert vector outbox event: {}", e);
        AppError::Internal(format!("Database error: {e}"))
    })?;

    if result.rows_affected() == 0 {
        // Conflict: another identical event already exists — still success for the writer.
        return Ok(event_id);
    }
    Ok(event_id)
}

/// Claim a batch of pending/failed events for processing (`FOR UPDATE SKIP LOCKED`).
pub async fn claim_batch(
    pool: &PgPool,
    worker_id: &str,
    batch_size: i64,
) -> Result<Vec<OutboxEvent>, AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        error!("outbox claim begin failed: {}", e);
        AppError::Internal(format!("Database error: {e}"))
    })?;

    let rows = sqlx::query_as::<_, OutboxRow>(
        r#"
        SELECT event_id, tenant_id, entry_id, operation, payload_json, payload_hash,
               idempotency_key, attempt_count
        FROM memory_vector_outbox
        WHERE status IN ('pending', 'failed')
          AND (next_retry_at IS NULL OR next_retry_at <= CURRENT_TIMESTAMP)
        ORDER BY created_at
        LIMIT $1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(batch_size)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| {
        error!("outbox claim select failed: {}", e);
        AppError::Internal(format!("Database error: {e}"))
    })?;

    if rows.is_empty() {
        tx.commit().await.ok();
        return Ok(Vec::new());
    }

    let ids: Vec<String> = rows.iter().map(|r| r.event_id.clone()).collect();
    sqlx::query(
        r#"
        UPDATE memory_vector_outbox
        SET status = 'processing',
            locked_at = CURRENT_TIMESTAMP,
            locked_by = $1,
            updated_at = CURRENT_TIMESTAMP
        WHERE event_id = ANY($2)
        "#,
    )
    .bind(worker_id)
    .bind(&ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("outbox claim update failed: {}", e);
        AppError::Internal(format!("Database error: {e}"))
    })?;

    tx.commit().await.map_err(|e| {
        error!("outbox claim commit failed: {}", e);
        AppError::Internal(format!("Database error: {e}"))
    })?;

    rows.into_iter()
        .map(|r| {
            Ok(OutboxEvent {
                event_id: r.event_id,
                tenant_id: r.tenant_id,
                entry_id: r.entry_id,
                operation: OutboxOperation::parse(&r.operation)?,
                payload_json: r.payload_json,
                payload_hash: r.payload_hash,
                idempotency_key: r.idempotency_key,
                attempt_count: r.attempt_count,
            })
        })
        .collect()
}

/// Mark event successfully applied to Qdrant.
pub async fn mark_applied(pool: &PgPool, event_id: &str) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE memory_vector_outbox
        SET status = 'applied',
            applied_at = CURRENT_TIMESTAMP,
            updated_at = CURRENT_TIMESTAMP,
            locked_at = NULL,
            locked_by = NULL,
            last_error = NULL
        WHERE event_id = $1
        "#,
    )
    .bind(event_id)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("outbox mark_applied failed: {}", e);
        AppError::Internal(format!("Database error: {e}"))
    })?;
    Ok(())
}

/// Mark event failed with exponential backoff, or dead-letter after `max_attempts`.
pub async fn mark_failed(
    pool: &PgPool,
    event_id: &str,
    attempt_count: i32,
    max_attempts: i32,
    error_msg: &str,
) -> Result<(), AppError> {
    let next_attempt = attempt_count + 1;
    if next_attempt >= max_attempts {
        sqlx::query(
            r#"
            UPDATE memory_vector_outbox
            SET status = 'dead_letter',
                attempt_count = $2,
                last_error = $3,
                dead_lettered_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP,
                locked_at = NULL,
                locked_by = NULL
            WHERE event_id = $1
            "#,
        )
        .bind(event_id)
        .bind(next_attempt)
        .bind(error_msg)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("outbox mark_dead_letter failed: {}", e);
            AppError::Internal(format!("Database error: {e}"))
        })?;
        return Ok(());
    }

    // Exponential backoff: base 5s * 2^attempt, capped at 1h.
    let backoff_secs = (5_i64 * (1_i64 << next_attempt.min(10))).min(3600);

    sqlx::query(
        r#"
        UPDATE memory_vector_outbox
        SET status = 'failed',
            attempt_count = $2,
            last_error = $3,
            next_retry_at = CURRENT_TIMESTAMP + ($4 * INTERVAL '1 second'),
            updated_at = CURRENT_TIMESTAMP,
            locked_at = NULL,
            locked_by = NULL
        WHERE event_id = $1
        "#,
    )
    .bind(event_id)
    .bind(next_attempt)
    .bind(error_msg)
    .bind(backoff_secs)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("outbox mark_failed failed: {}", e);
        AppError::Internal(format!("Database error: {e}"))
    })?;
    Ok(())
}

/// Reclaim stale `processing` rows whose lock is older than `stale_secs`.
pub async fn reclaim_stale(pool: &PgPool, stale_secs: i64) -> Result<u64, AppError> {
    let result = sqlx::query(
        r#"
        UPDATE memory_vector_outbox
        SET status = 'pending',
            locked_at = NULL,
            locked_by = NULL,
            updated_at = CURRENT_TIMESTAMP,
            last_error = COALESCE(last_error, 'reclaimed_stale_lock')
        WHERE status = 'processing'
          AND locked_at IS NOT NULL
          AND locked_at < CURRENT_TIMESTAMP - ($1 * INTERVAL '1 second')
        "#,
    )
    .bind(stale_secs)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("outbox reclaim_stale failed: {}", e);
        AppError::Internal(format!("Database error: {e}"))
    })?;
    Ok(result.rows_affected())
}

#[derive(sqlx::FromRow)]
struct OutboxRow {
    event_id: String,
    tenant_id: String,
    entry_id: String,
    operation: String,
    payload_json: String,
    payload_hash: String,
    idempotency_key: String,
    attempt_count: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::information_guard::compute_sha256;

    #[test]
    fn upsert_key_includes_entry_and_hash() {
        let k = upsert_idempotency_key("e1", "abc");
        assert_eq!(k, "upsert:e1:abc");
    }

    #[test]
    fn delete_key_is_stable() {
        assert_eq!(delete_idempotency_key("e1"), "delete:e1");
    }

    #[test]
    fn operation_roundtrip() {
        assert_eq!(OutboxOperation::parse("upsert").unwrap(), OutboxOperation::Upsert);
        assert_eq!(OutboxOperation::parse("delete").unwrap(), OutboxOperation::Delete);
        assert!(OutboxOperation::parse("nope").is_err());
    }

    #[test]
    fn test_upsert_idempotency_key_format() {
        let key = upsert_idempotency_key("entry-123", "hash-abc");
        assert_eq!(key, "upsert:entry-123:hash-abc");
    }

    #[test]
    fn test_delete_idempotency_key_format() {
        let key = delete_idempotency_key("entry-456");
        assert_eq!(key, "delete:entry-456");
    }

    #[test]
    fn test_upsert_idempotency_key_different_hashes() {
        let entry_id = "entry-789";
        let key_a = upsert_idempotency_key(entry_id, "hash-a");
        let key_b = upsert_idempotency_key(entry_id, "hash-b");
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn test_upsert_idempotency_key_same_hashes() {
        let entry_id = "entry-789";
        let key_a = upsert_idempotency_key(entry_id, "hash-same");
        let key_b = upsert_idempotency_key(entry_id, "hash-same");
        assert_eq!(key_a, key_b);
    }

    #[test]
    fn test_payload_hash_deterministic() {
        let input = "the quick brown fox";
        let hash_a = compute_sha256(input);
        let hash_b = compute_sha256(input);
        assert_eq!(hash_a, hash_b);
    }

    #[test]
    fn test_payload_hash_different_inputs() {
        let hash_a = compute_sha256("input one");
        let hash_b = compute_sha256("input two");
        assert_ne!(hash_a, hash_b);
    }
}
