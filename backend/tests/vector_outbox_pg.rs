//! Behavioural integration tests for the transactional vector outbox
//! (`memory_vector_outbox`). These tests prove the five core reliability
//! guarantees of the outbox: idempotent insert, concurrent claim with
//! disjoint delivery, applied exclusion, retry/dead-letter escalation,
//! and stale-lock reclamation.
//!
//! Reports as `ignored` when `DATABASE_URL` is unset (no false-green pass).
//! The env-var guard inside the body still skips the test when
//! `--include-ignored` is used without DATABASE_URL.
//!
//! Run:
//!   DATABASE_URL=postgres://memory:memory@localhost:5432/memory \
//!     cargo test --test vector_outbox_pg -- --include-ignored --nocapture
//!
//! The outbox table is NOT under RLS, so tests use the owner connection
//! directly without setting any tenant GUC.
//!
//! ⚠️  THESE TESTS WRITE TO THE TARGET DATABASE. They INSERT outbox rows and
//! DELETE them again, always scoped to their own `vob_<label>_<nanos>` tenant
//! prefix, and they call the real `claim_batch` / `mark_*` / `reclaim_stale`
//! functions, which UPDATE row state. Point `DATABASE_URL` at a disposable
//! database. Notably `claim_batch` is a global queue (see below), so running
//! these against a database with live pending events will claim and mutate
//! those events too.
//!
//! Design note surfaced by these tests: `claim_batch` does NOT scope by
//! `tenant_id` — outbox draining is process-global with no per-tenant fairness.
//! One tenant with a large backlog can delay every other tenant's vector
//! indexing. That is the current design, not a bug, but it is a gap in the
//! multi-tenant story and is why these tests need a mutex plus tenant
//! post-filtering rather than being able to isolate themselves naturally.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use sqlx::Row;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use backend::db::vector_outbox::{self, OutboxOperation};

static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn test_suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_string()
}

fn test_tenant(label: &str) -> String {
    format!("vob_{label}_{}", test_suffix())
}

async fn setup_pool(db_url: &str, max_conn: u32) -> PgPool {
    let pool = PgPoolOptions::new()
        .max_connections(max_conn)
        .connect(db_url)
        .await
        .expect("connect to postgres");

    let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let migrator = sqlx::migrate::Migrator::new(migrations_path)
        .await
        .expect("build migrator");
    migrator.run(&pool).await.expect("run migrations");
    pool
}

/// Delete ONLY the rows this test created, identified by its unique tenant prefix.
///
/// There is deliberately **no** unscoped `DELETE FROM memory_vector_outbox` helper
/// in this file. An earlier revision had one, and it destroyed real non-test rows
/// (8 `applied` + 1 `dead_letter`) in a developer database. That is unacceptable:
/// CI runs these tests with `--include-ignored`, and a developer can point
/// `DATABASE_URL` at a shared or staging database, where wiping the outbox
/// silently discards pending vector-index events — the exact durability loss the
/// outbox exists to prevent, with no record that it happened.
///
/// Isolation from foreign rows is achieved WITHOUT destroying anything:
/// `TEST_MUTEX` serialises the tests against each other, `filter_by_tenant`
/// restricts every assertion to this test's own events, and the claim batch size
/// is set above the event count so unrelated rows cannot crowd the test's rows
/// out of `claim_batch`'s `ORDER BY created_at ... LIMIT`. Refuse or ignore —
/// never destroy.
async fn cleanup(pool: &PgPool, tenant: &str) {
    let _ = sqlx::query("DELETE FROM memory_vector_outbox WHERE tenant_id = $1")
        .bind(tenant)
        .execute(pool)
        .await;
}

fn filter_by_tenant(
    events: Vec<vector_outbox::OutboxEvent>,
    tenant: &str,
) -> Vec<vector_outbox::OutboxEvent> {
    events.into_iter().filter(|e| e.tenant_id == tenant).collect()
}

// ── Test 1: insert_event_tx idempotency ────────────────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn insert_event_tx_idempotent_duplicate_leaves_one_row() {
    let Ok(db_url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP vector_outbox_pg: DATABASE_URL not set");
        return;
    };

    let pool = setup_pool(&db_url, 2).await;
    let _guard = TEST_MUTEX.lock().await;
    let tenant = test_tenant("idem");
    cleanup(&pool, &tenant).await;
    let entry_id = format!("entry-{tenant}");
    let payload = r#"{"vector":[1.0,2.0,3.0]}"#;
    let payload_hash = "abc123";
    let idem_key = format!("vob.idem:{tenant}:dup");

    let mut tx = pool.begin().await.expect("begin tx");
    let r1 = vector_outbox::insert_event_tx(
        &mut tx, &tenant, &entry_id, OutboxOperation::Upsert,
        payload, payload_hash, &idem_key,
    )
    .await
    .expect("first insert");
    let r2 = vector_outbox::insert_event_tx(
        &mut tx, &tenant, &entry_id, OutboxOperation::Upsert,
        payload, payload_hash, &idem_key,
    )
    .await
    .expect("second insert (idempotent)");
    tx.commit().await.expect("commit");

    assert_ne!(r1, r2, "second call generates a new Ulid (not persisted)");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM memory_vector_outbox WHERE tenant_id = $1 AND idempotency_key = $2",
    )
    .bind(&tenant)
    .bind(&idem_key)
    .fetch_one(&pool)
    .await
    .expect("count query");

    assert_eq!(count, 1, "idempotent insert must leave exactly ONE row, got {count}");

    let stored_id: String = sqlx::query_scalar(
        "SELECT event_id FROM memory_vector_outbox WHERE tenant_id = $1 AND idempotency_key = $2",
    )
    .bind(&tenant)
    .bind(&idem_key)
    .fetch_one(&pool)
    .await
    .expect("fetch stored row");

    assert_eq!(
        stored_id, r1,
        "stored event_id must be the first insert's id (ON CONFLICT DO NOTHING preserves original)"
    );

    cleanup(&pool, &tenant).await;
}

// ── Test 2: claim_batch concurrent disjoint delivery ───────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn claim_batch_concurrent_disjoint_sets() {
    let Ok(db_url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP vector_outbox_pg: DATABASE_URL not set");
        return;
    };

    let pool = setup_pool(&db_url, 4).await;
    let _guard = TEST_MUTEX.lock().await;
    let tenant = test_tenant("claim");
    cleanup(&pool, &tenant).await;
    let num_events: usize = 20;

    for i in 0..num_events {
        let mut tx = pool.begin().await.expect("begin");
        let entry_id = format!("entry-{tenant}-{i}");
        let payload = format!(r#"{{"vector":[{i}.0,{i}.1,{i}.2]}}"#);
        let idem_key = format!("vob.claim:{tenant}:{i}");
        let _ = vector_outbox::insert_event_tx(
            &mut tx, &tenant, &entry_id, OutboxOperation::Upsert,
            &payload, &format!("hash-{i}"), &idem_key,
        )
        .await
        .expect("insert");
        tx.commit().await.expect("commit");
    }

    let pool_a = PgPoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("pool a");
    let pool_b = PgPoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("pool b");

    let batch_size = (num_events as i64) + 5;

    let (result_a, result_b) = tokio::join!(
        vector_outbox::claim_batch(&pool_a, "worker-a", batch_size),
        vector_outbox::claim_batch(&pool_b, "worker-b", batch_size),
    );

    let events_a = filter_by_tenant(result_a.expect("worker a claim"), &tenant);
    let events_b = filter_by_tenant(result_b.expect("worker b claim"), &tenant);

    let ids_a: std::collections::HashSet<_> = events_a.iter().map(|e| &e.event_id).collect();
    let ids_b: std::collections::HashSet<_> = events_b.iter().map(|e| &e.event_id).collect();

    let intersection: Vec<_> = ids_a.intersection(&ids_b).collect();
    assert!(
        intersection.is_empty(),
        "concurrent claimers must receive DISJOINT event sets; overlap: {:?}",
        intersection
    );

    let total = ids_a.len() + ids_b.len();
    assert_eq!(
        total, num_events,
        "all {num_events} events must be claimed exactly once, got {total} (worker-a={}, worker-b={})",
        ids_a.len(), ids_b.len()
    );

    cleanup(&pool, &tenant).await;
    pool_a.close().await;
    pool_b.close().await;
}

// ── Test 3: mark_applied excludes from subsequent claim ────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn mark_applied_excludes_event_from_claim() {
    let Ok(db_url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP vector_outbox_pg: DATABASE_URL not set");
        return;
    };

    let pool = setup_pool(&db_url, 2).await;
    let _guard = TEST_MUTEX.lock().await;
    let tenant = test_tenant("applied");
    cleanup(&pool, &tenant).await;

    let mut tx = pool.begin().await.expect("begin");
    let entry_id = format!("entry-{tenant}");
    let idem_key = format!("vob.applied:{tenant}:0");
    let event_id = vector_outbox::insert_event_tx(
        &mut tx, &tenant, &entry_id, OutboxOperation::Upsert,
        r#"{"vector":[1.0]}"#, "hash-0", &idem_key,
    )
    .await
    .expect("insert");
    tx.commit().await.expect("commit");

    let claimed = filter_by_tenant(
        vector_outbox::claim_batch(&pool, "worker-applied", 1).await.expect("claim"),
        &tenant,
    );
    assert_eq!(claimed.len(), 1, "must claim the single event");
    assert_eq!(claimed[0].event_id, event_id);

    vector_outbox::mark_applied(&pool, &event_id)
        .await
        .expect("mark_applied");

    let status: String = sqlx::query_scalar(
        "SELECT status FROM memory_vector_outbox WHERE event_id = $1",
    )
    .bind(&event_id)
    .fetch_one(&pool)
    .await
    .expect("status query");
    assert_eq!(status, "applied", "event must be in 'applied' status");

    let claimed_again = filter_by_tenant(
        vector_outbox::claim_batch(&pool, "worker-applied-2", 10).await.expect("claim again"),
        &tenant,
    );
    assert!(
        claimed_again.is_empty(),
        "applied event must NOT be re-claimed; got {} events",
        claimed_again.len()
    );

    cleanup(&pool, &tenant).await;
}

// ── Test 4: mark_failed retry counter and dead-letter escalation ───────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn mark_failed_increments_retry_and_dead_letters_after_max_attempts() {
    let Ok(db_url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP vector_outbox_pg: DATABASE_URL not set");
        return;
    };

    let pool = setup_pool(&db_url, 2).await;
    let _guard = TEST_MUTEX.lock().await;
    let tenant = test_tenant("failed");
    cleanup(&pool, &tenant).await;
    let max_attempts: i32 = 8;

    let mut tx = pool.begin().await.expect("begin");
    let entry_id = format!("entry-{tenant}");
    let idem_key = format!("vob.failed:{tenant}:0");
    let event_id = vector_outbox::insert_event_tx(
        &mut tx, &tenant, &entry_id, OutboxOperation::Upsert,
        r#"{"vector":[1.0]}"#, "hash-0", &idem_key,
    )
    .await
    .expect("insert");
    tx.commit().await.expect("commit");

    vector_outbox::mark_failed(&pool, &event_id, 0, max_attempts, "error #1")
        .await
        .expect("mark_failed #1");

    let row = sqlx::query(
        "SELECT status, attempt_count, next_retry_at > CURRENT_TIMESTAMP AS retry_in_future
         FROM memory_vector_outbox WHERE event_id = $1",
    )
    .bind(&event_id)
    .fetch_one(&pool)
    .await
    .expect("fetch after fail #1");

    let status: String = row.get("status");
    let attempt_count: i32 = row.get("attempt_count");
    let retry_in_future: bool = row.get("retry_in_future");

    assert_eq!(status, "failed", "first failure → status 'failed'");
    assert_eq!(attempt_count, 1, "attempt_count must increment to 1");
    assert!(retry_in_future, "next_retry_at must be in the future (backoff applied)");

    let claimed = filter_by_tenant(
        vector_outbox::claim_batch(&pool, "worker-failed", 10).await.expect("claim after failed"),
        &tenant,
    );
    assert!(
        claimed.is_empty(),
        "failed event with future next_retry_at must not be claimed; got {} events",
        claimed.len()
    );

    vector_outbox::mark_failed(&pool, &event_id, 7, max_attempts, "error #8 - final")
        .await
        .expect("mark_failed #8 (dead-letter)");

    let row = sqlx::query(
        "SELECT status, attempt_count, last_error,
                dead_lettered_at IS NOT NULL AS dead_letter_set
         FROM memory_vector_outbox WHERE event_id = $1",
    )
    .bind(&event_id)
    .fetch_one(&pool)
    .await
    .expect("fetch after dead-letter");

    let status: String = row.get("status");
    let attempt_count: i32 = row.get("attempt_count");
    let last_error: String = row.get("last_error");
    let dead_letter_set: bool = row.get("dead_letter_set");

    assert_eq!(status, "dead_letter", "final failure → status 'dead_letter'");
    assert_eq!(attempt_count, 8, "attempt_count must reach 8 (max_attempts)");
    assert!(
        last_error.contains("error #8"),
        "last_error must contain the final error message, got: {last_error}"
    );
    assert!(dead_letter_set, "dead_lettered_at must be set when entering dead_letter state");

    cleanup(&pool, &tenant).await;
}

// ── Test 5: reclaim_stale recovers abandoned processing events ─────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn reclaim_stale_recovers_abandoned_events() {
    let Ok(db_url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP vector_outbox_pg: DATABASE_URL not set");
        return;
    };

    let pool = setup_pool(&db_url, 2).await;
    let _guard = TEST_MUTEX.lock().await;
    let tenant = test_tenant("reclaim");
    cleanup(&pool, &tenant).await;

    let mut event_ids = Vec::new();
    for i in 0..2 {
        let mut tx = pool.begin().await.expect("begin");
        let entry_id = format!("entry-{tenant}-{i}");
        let idem_key = format!("vob.reclaim:{tenant}:{i}");
        let eid = vector_outbox::insert_event_tx(
            &mut tx, &tenant, &entry_id, OutboxOperation::Upsert,
            r#"{"vector":[1.0]}"#, &format!("hash-{i}"), &idem_key,
        )
        .await
        .expect("insert");
        tx.commit().await.expect("commit");
        event_ids.push(eid);
    }

    let claimed = filter_by_tenant(
        vector_outbox::claim_batch(&pool, "worker-reclaim", 10).await.expect("claim"),
        &tenant,
    );
    assert_eq!(claimed.len(), 2, "must claim both events");

    for eid in &event_ids {
        let status: String = sqlx::query_scalar(
            "SELECT status FROM memory_vector_outbox WHERE event_id = $1",
        )
        .bind(eid)
        .fetch_one(&pool)
        .await
        .expect("status query");
        assert_eq!(status, "processing", "event {eid} must be 'processing' after claim");
    }

    let reclaimed = vector_outbox::reclaim_stale(&pool, 0)
        .await
        .expect("reclaim_stale stale_secs=0");
    assert_eq!(reclaimed, 2, "both events must be reclaimed with stale_secs=0");

    for eid in &event_ids {
        let status: String = sqlx::query_scalar(
            "SELECT status FROM memory_vector_outbox WHERE event_id = $1",
        )
        .bind(eid)
        .fetch_one(&pool)
        .await
        .expect("status query after reclaim");
        assert_eq!(status, "pending", "event {eid} must be 'pending' after reclaim_stale");
    }

    let claimed_again = filter_by_tenant(
        vector_outbox::claim_batch(&pool, "worker-reclaim-2", 1).await.expect("claim again"),
        &tenant,
    );
    assert_eq!(claimed_again.len(), 1, "must claim one event");
    let freshly_claimed = &claimed_again[0].event_id;

    let reclaimed_fresh = vector_outbox::reclaim_stale(&pool, 120)
        .await
        .expect("reclaim_stale stale_secs=120");
    assert_eq!(reclaimed_fresh, 0, "freshly claimed event must NOT be reclaimed (locked_at < 120s ago)");

    let status: String = sqlx::query_scalar(
        "SELECT status FROM memory_vector_outbox WHERE event_id = $1",
    )
    .bind(freshly_claimed)
    .fetch_one(&pool)
    .await
    .expect("status query");
    assert_eq!(
        status, "processing",
        "freshly claimed event must remain 'processing' after unsuccessful reclaim"
    );

    cleanup(&pool, &tenant).await;
}