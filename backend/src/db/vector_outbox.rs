//! Durable vector outbox repository (ADR-0002 / P1 W0.1).
//!
//! PostgreSQL-only. Events are inserted in the same transaction as LTM fact rows,
//! then claimed and applied asynchronously by [`crate::services::outbox_worker`].

use std::sync::Mutex;
use std::time::{Duration, Instant};

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

/// Per-tenant fair-share cap for a single [`claim_batch`] call.
///
/// A tenant with a large backlog must not be able to fill an entire batch and
/// stall every other tenant's vector indexing (backlog C-2). This divides the
/// batch evenly across the tenants that currently have claimable work, rounding
/// UP so the batch still fills to `batch_size` whenever there is enough work:
/// `n` tenants each capped at `ceil(batch_size / n)` sum to at least `batch_size`.
///
/// Pure and `pub(crate)` so the fairness policy is pinned by unit tests even
/// though the ranking that consumes this cap runs in Postgres — a runtime query,
/// not a compile-checked `query!`, so `cargo check` does not validate that SQL.
///
/// Boundaries (mirrored by the unit tests):
/// - `active_tenants == 1` → cap == `batch_size`: a bulk single-tenant import
///   runs at full speed, never throttled to a fraction of the batch.
/// - `active_tenants >= batch_size` → cap == 1: maximally fair, and the batch
///   still fills from `batch_size` distinct tenants — no throughput loss.
/// - `active_tenants == 0` (no claimable work) → cap == `batch_size`: the value
///   is irrelevant because the claim returns nothing, but it must be defined and
///   must never divide by zero.
pub(crate) fn per_tenant_claim_limit(batch_size: i64, active_tenants: i64) -> i64 {
    if batch_size <= 0 {
        return 0;
    }
    let tenants = active_tenants.max(1);
    // Ceiling division written as divide-then-adjust rather than the usual
    // `(a + b - 1) / b`: that form overflows for large `tenants` (panics in debug,
    // wraps to a nonsense cap in release). Dividing first cannot overflow — both
    // operands are positive here. `i64::div_ceil` would be clearer but is still
    // unstable for signed integers (`int_roundings`).
    //
    // The count comes from `COUNT(DISTINCT tenant_id)`, so a huge value is not
    // reachable today; this is a pure `pub(crate)` helper with nothing
    // constraining its inputs, and "ceiling, floored at 1" must hold for all of
    // them.
    let cap = batch_size / tenants + i64::from(batch_size % tenants != 0);
    cap.max(1)
}

/// How long a cached active-tenant count stays usable.
///
/// The claim loop polls every 2s; refreshing roughly every 15th cycle mirrors the
/// `RECLAIM_EVERY_N_LOOPS` throttle in `services::outbox_worker`.
const ACTIVE_TENANT_COUNT_TTL: Duration = Duration::from_secs(30);

/// Cached `(counted_at, count)` for [`cached_active_tenant_count`].
static ACTIVE_TENANT_COUNT: Mutex<Option<(Instant, i64)>> = Mutex::new(None);

/// Active-tenant count with a [`ACTIVE_TENANT_COUNT_TTL`] cache.
///
/// Only feeds [`per_tenant_claim_limit`]; see the call site for why staleness is
/// safe. Errors propagate rather than falling back to a default: silently
/// substituting a made-up tenant count would make the fairness cap wrong in a way
/// no caller could observe.
async fn cached_active_tenant_count(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<i64, AppError> {
    // Held across no `.await`: read the cache, drop the guard, then query.
    if let Some((counted_at, count)) = *ACTIVE_TENANT_COUNT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
    {
        if counted_at.elapsed() < ACTIVE_TENANT_COUNT_TTL {
            return Ok(count);
        }
    }

    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT tenant_id)
        FROM memory_vector_outbox
        WHERE status IN ('pending', 'failed')
          AND (next_retry_at IS NULL OR next_retry_at <= CURRENT_TIMESTAMP)
        "#,
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| {
        error!("outbox claim tenant-count failed: {}", e);
        AppError::Internal(format!("Database error: {e}"))
    })?;

    *ACTIVE_TENANT_COUNT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((Instant::now(), count));
    Ok(count)
}

/// Claim a batch of pending/failed events for processing (`FOR UPDATE SKIP LOCKED`),
/// with per-tenant fairness so one tenant's backlog cannot starve the rest (C-2).
///
/// # `cargo check` proves nothing about the SQL below
///
/// The queries here are built with the runtime `sqlx::query*` functions, not the
/// compile-checked `query!` macros. Nothing validates the CTE syntax, the
/// window-function-plus-`FOR UPDATE` layering, or the column names at build time —
/// a green `cargo check` says only that the Rust compiles.
///
/// The only real coverage is the five behavioural tests in
/// `tests/vector_outbox_pg.rs`, which are `#[ignore]`d because they need a live
/// PostgreSQL. Plain `cargo test` skips them. **If you change this function, you
/// must run them against a real database** — CI does so via
/// `cargo test --tests -- --include-ignored`.
///
/// Two properties in particular have no other guard:
/// - the repeated `status` / `next_retry_at` predicates on the outer query, which
///   keep EvalPlanQual from re-claiming a row a concurrent worker already took
///   (see the inline comment at the query);
/// - disjointness across concurrent workers, covered by the `tokio::join!` test.
pub async fn claim_batch(
    pool: &PgPool,
    worker_id: &str,
    batch_size: i64,
) -> Result<Vec<OutboxEvent>, AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        error!("outbox claim begin failed: {}", e);
        AppError::Internal(format!("Database error: {e}"))
    })?;

    // Fairness knob: how many tenants currently have claimable work.
    //
    // Throttled behind a short TTL rather than recomputed every cycle. The claim
    // loop polls every 2s and this is a `COUNT(DISTINCT tenant_id)` over the whole
    // eligible set, so recomputing it per cycle doubles the scan cost of a claim
    // for a number that moves slowly.
    //
    // A stale count is safe by construction: it only nudges the per-tenant cap up
    // or down. Claim correctness — disjointness across workers, no double-claim —
    // comes solely from `FOR UPDATE SKIP LOCKED` plus the re-checked predicates
    // below, never from this number. Over-estimating narrows each tenant's slice
    // for one window; under-estimating widens it. Both self-correct on the next
    // refresh.
    let active_tenants = cached_active_tenant_count(&mut tx).await?;

    let per_tenant_limit = per_tenant_claim_limit(batch_size, active_tenants);

    // Fair claim: rank each tenant's claimable events by age, keep only each
    // tenant's oldest `per_tenant_limit`, then take the globally oldest
    // `batch_size` of that fair-limited set.
    //
    // Postgres forbids `FOR UPDATE` in a query that itself uses a window
    // function, so `ROW_NUMBER()` lives in the non-locking `ranked` CTE and the
    // lock is taken by the outer plain select over the base table (whose rows map
    // 1:1 to table rows, which `FOR UPDATE` requires). Disjointness across
    // concurrent workers still comes solely from `SKIP LOCKED` on that outer
    // select — the CTE takes no locks.
    //
    // The `status`/`next_retry_at` predicates are REPEATED on the outer query on
    // purpose: under READ COMMITTED, EvalPlanQual re-checks the outer WHERE
    // against the latest row version at lock time, so a row a concurrent worker
    // already advanced to `processing` and committed is rejected here rather than
    // double-claimed. Dropping them would reintroduce that race.
    let rows = sqlx::query_as::<_, OutboxRow>(
        r#"
        WITH ranked AS (
            SELECT event_id,
                   ROW_NUMBER() OVER (
                       PARTITION BY tenant_id
                       ORDER BY created_at, event_id
                   ) AS rn
            FROM memory_vector_outbox
            WHERE status IN ('pending', 'failed')
              AND (next_retry_at IS NULL OR next_retry_at <= CURRENT_TIMESTAMP)
        )
        SELECT event_id, tenant_id, entry_id, operation, payload_json, payload_hash,
               idempotency_key, attempt_count
        FROM memory_vector_outbox
        WHERE event_id IN (SELECT event_id FROM ranked WHERE rn <= $1)
          AND status IN ('pending', 'failed')
          AND (next_retry_at IS NULL OR next_retry_at <= CURRENT_TIMESTAMP)
        ORDER BY created_at
        LIMIT $2
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(per_tenant_limit)
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

/// Outcome of [`mark_failed`]: whether the event was rescheduled for another
/// delivery attempt, or escalated to the dead-letter queue after exhausting
/// retries.
///
/// Returned so the outbox worker can emit `outbox_dead_letter_total` exactly
/// once per dead-letter transition **without re-deriving** the
/// `attempt_count + 1 >= max_attempts` decision — that decision lives here, in
/// one place. `claim_batch` only re-claims `pending`/`failed` rows, so a row
/// that reaches `dead_letter` is never processed again: one transition yields
/// exactly one `DeadLettered`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkFailedOutcome {
    /// Rescheduled with backoff (status = `failed`).
    Retried,
    /// Escalated to the dead-letter queue (status = `dead_letter`).
    DeadLettered,
}

/// Whether the next failure of an event currently at `attempt_count` exhausts
/// the retry budget and dead-letters it.
///
/// Pure and `pub(crate)` so both [`mark_failed`] (which acts on it) and the unit
/// tests (which pin the boundary) share one definition of the threshold.
pub(crate) fn is_dead_letter_transition(attempt_count: i32, max_attempts: i32) -> bool {
    attempt_count + 1 >= max_attempts
}

/// Mark event failed with exponential backoff, or dead-letter after `max_attempts`.
///
/// Returns [`MarkFailedOutcome`] so callers can observe dead-letter transitions
/// (see the enum docs) without duplicating the threshold logic.
pub async fn mark_failed(
    pool: &PgPool,
    event_id: &str,
    attempt_count: i32,
    max_attempts: i32,
    error_msg: &str,
) -> Result<MarkFailedOutcome, AppError> {
    let next_attempt = attempt_count + 1;
    if is_dead_letter_transition(attempt_count, max_attempts) {
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
        return Ok(MarkFailedOutcome::DeadLettered);
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
    Ok(MarkFailedOutcome::Retried)
}

/// Count events awaiting first delivery (`status = 'pending'`).
///
/// Published as the `outbox_pending_total` gauge by the worker. Deliberately
/// counts only `pending`, matching that gauge's name and the staged
/// `OutboxBacklogHigh` alert (`monitoring/alerts-staged/aetheris-pending-instrumentation.yml`,
/// which documents `... WHERE status='pending'`): it is the "work not yet
/// started" backlog. Events in `failed` (waiting on retry backoff) and
/// `processing` (in flight) are intentionally excluded — surfacing a retry
/// backlog would need its own series rather than conflating it with fresh work.
pub async fn count_pending(pool: &PgPool) -> Result<i64, AppError> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM memory_vector_outbox WHERE status = 'pending'")
            .fetch_one(pool)
            .await
            .map_err(|e| {
                error!("outbox count_pending failed: {}", e);
                AppError::Internal(format!("Database error: {e}"))
            })?;
    Ok(count)
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
        assert_eq!(
            OutboxOperation::parse("upsert").unwrap(),
            OutboxOperation::Upsert
        );
        assert_eq!(
            OutboxOperation::parse("delete").unwrap(),
            OutboxOperation::Delete
        );
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

    /// The dead-letter boundary drives `outbox_dead_letter_total`; pin it so the
    /// metric can never silently start counting ordinary retries as dead-letters.
    /// Mirrors the integration test in `tests/vector_outbox_pg.rs` (max_attempts=8:
    /// attempt_count 0..=6 retry, 7 dead-letters).
    #[test]
    fn dead_letter_transition_boundary() {
        let max = 8;
        for attempt in 0..=6 {
            assert!(
                !is_dead_letter_transition(attempt, max),
                "attempt_count={attempt} (next={}) must still retry, not dead-letter",
                attempt + 1
            );
        }
        assert!(
            is_dead_letter_transition(7, max),
            "attempt_count=7 (next=8) must dead-letter at max_attempts=8"
        );
        // Defensive: an event already at/over the budget must also dead-letter.
        assert!(is_dead_letter_transition(8, max));
    }

    // ── per_tenant_claim_limit: fairness cap policy (backlog C-2) ──────────────

    #[test]
    fn per_tenant_limit_single_tenant_uses_full_batch() {
        // A lone tenant (e.g. a bulk import) must NOT be throttled below the
        // batch size, or single-tenant throughput collapses. This is the
        // regression the naive `fixed cap` and `batch / tenants` designs risk.
        assert_eq!(per_tenant_claim_limit(32, 1), 32);
        assert_eq!(per_tenant_claim_limit(1, 1), 1);
    }

    #[test]
    fn per_tenant_limit_zero_tenants_is_defined_and_safe() {
        // No claimable work: the cap is irrelevant (claim returns nothing) but
        // must be well-defined and must never divide by zero.
        assert_eq!(per_tenant_claim_limit(32, 0), 32);
    }

    #[test]
    fn per_tenant_limit_splits_evenly_rounding_up() {
        assert_eq!(per_tenant_claim_limit(32, 2), 16);
        assert_eq!(per_tenant_claim_limit(32, 3), 11); // ceil(32/3) = 11
        assert_eq!(per_tenant_claim_limit(32, 4), 8);
    }

    #[test]
    fn per_tenant_limit_many_tenants_floor_is_one() {
        // At or beyond one-per-tenant, every tenant still gets at least one slot.
        assert_eq!(per_tenant_claim_limit(32, 32), 1);
        assert_eq!(per_tenant_claim_limit(32, 100), 1);
    }

    #[test]
    fn per_tenant_limit_batch_still_fills() {
        // Rounding-up guarantee: for any tenant count up to the batch size, `n`
        // tenants each contributing `cap` events cover the whole batch, so
        // fairness never forces the batch to underfill when work exists.
        let batch = 32;
        for n in 1..=batch {
            let cap = per_tenant_claim_limit(batch, n);
            assert!(
                n * cap >= batch,
                "n={n} cap={cap}: {n}*{cap} must cover batch {batch}"
            );
            assert!(cap >= 1, "cap must never drop below 1 (n={n})");
        }
    }

    #[test]
    fn per_tenant_limit_non_increasing_in_tenant_count() {
        // More contending tenants → a tighter (never looser) per-tenant share.
        let batch = 32;
        let mut prev = per_tenant_claim_limit(batch, 1);
        for n in 2..=64 {
            let cap = per_tenant_claim_limit(batch, n);
            assert!(cap <= prev, "cap must not increase as tenants grow (n={n})");
            prev = cap;
        }
    }

    #[test]
    fn per_tenant_limit_zero_batch_is_zero() {
        assert_eq!(per_tenant_claim_limit(0, 5), 0);
    }

    /// The active-tenant count is cached behind a TTL, so the cap is sometimes
    /// computed from a stale number. This pins that a stale count can only widen
    /// or narrow a tenant's slice — it can never produce a cap that breaks the
    /// claim: never zero (which would stall a tenant permanently) and never above
    /// `batch_size` (which would let one tenant take the whole batch and defeat
    /// the fairness this function exists for).
    ///
    /// Correctness of the claim itself does not depend on this value at all —
    /// disjointness comes from `FOR UPDATE SKIP LOCKED` plus the re-checked outer
    /// predicates. This test guards the fairness property, not correctness.
    #[test]
    fn per_tenant_limit_stays_sane_for_any_stale_tenant_count() {
        const BATCH: i64 = 32;
        // Includes counts a stale cache could plausibly hold: zero, one, fewer
        // and more than the real value, and far more than the batch size.
        for stale in [0, 1, 2, 7, 31, 32, 33, 1_000, i64::MAX] {
            let cap = per_tenant_claim_limit(BATCH, stale);
            assert!(
                cap >= 1,
                "cap must never be 0 for stale count {stale} — a tenant would stall forever"
            );
            assert!(
                cap <= BATCH,
                "cap {cap} exceeds batch {BATCH} for stale count {stale} — one tenant \
                 could take the whole batch"
            );
        }
    }
}
