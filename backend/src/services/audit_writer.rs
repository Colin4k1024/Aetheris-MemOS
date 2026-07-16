//! Asynchronous, best-effort audit writer (P1 子项 b).
//!
//! Governance/audit callbacks enqueue [`AuditEvent`]s via [`record_audit`] on the
//! request hot path — a non-blocking, fire-and-forget enqueue. A single background
//! task batches them within a short time window and persists them to
//! `memory_audit_events` via [`crate::db::audit::insert_event`].
//!
//! Consistency model: audit here is **weakly consistent / best-effort**. Writes are
//! never retried and never back-pressure the request — a full queue or a failed
//! INSERT degrades to an `error!`/`warn!` log and a dropped-event counter (see the
//! governance plan, 子项 b). If a specific event needs strong consistency, write it
//! in the mutation's own transaction with [`crate::db::audit::insert_tx`] instead.
//!
//! Persistence targets PostgreSQL only; on a SQLite (dev) backend the batch is
//! dropped and counted. Callers should therefore gate startup on `is_postgres()`.
//!
//! The background-queue shape mirrors [`crate::services::write_queue`].

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::db::audit::AuditEvent;

/// Global sender — set once at startup via [`init_audit_writer`].
static AUDIT_QUEUE: OnceLock<mpsc::Sender<AuditEvent>> = OnceLock::new();

/// Count of audit events dropped (queue full, closed, uninitialised, or no PG pool).
static DROPPED: AtomicU64 = AtomicU64::new(0);

/// Queue capacity before enqueues start dropping (audit is best-effort, so we favour
/// a generous buffer over back-pressure on the request path).
const QUEUE_CAPACITY: usize = 8192;

/// Maximum number of events flushed to the database in one batch.
const MAX_BATCH_SIZE: usize = 256;

/// Time window to collect events before flushing (milliseconds).
const FLUSH_WINDOW_MS: u64 = 50;

/// Start the background audit writer. Idempotent — subsequent calls are no-ops.
///
/// Must be called after the database pool is initialised. On a non-PostgreSQL
/// backend the worker still starts but drops batches; gate the call on
/// `crate::db::is_postgres()` at the startup site to avoid the wasted task.
pub fn init_audit_writer() {
    if AUDIT_QUEUE.get().is_some() {
        return;
    }

    let (tx, rx) = mpsc::channel::<AuditEvent>(QUEUE_CAPACITY);
    if AUDIT_QUEUE.set(tx).is_err() {
        // Lost an init race with another caller; that caller owns the worker.
        return;
    }

    tokio::spawn(audit_writer_worker(rx));
    info!(
        "Audit writer started (capacity={}, batch={}, window={}ms)",
        QUEUE_CAPACITY, MAX_BATCH_SIZE, FLUSH_WINDOW_MS
    );
}

/// Enqueue an audit event without blocking. If the writer is not initialised or the
/// queue is full, the event is dropped and counted — the caller never waits and never
/// sees an error, keeping the request hot path free of audit-persistence latency.
pub fn record_audit(event: AuditEvent) {
    let Some(sender) = AUDIT_QUEUE.get() else {
        // Writer not started (called before init, or in a context without a writer).
        DROPPED.fetch_add(1, Ordering::Relaxed);
        return;
    };

    if sender.try_send(event).is_err() {
        let dropped = DROPPED.fetch_add(1, Ordering::Relaxed) + 1;
        // Log sparsely so sustained overflow cannot itself flood the logs.
        if dropped % 100 == 1 {
            warn!(
                "Audit queue full or closed; dropped {} audit events so far",
                dropped
            );
        }
    }
}

/// Total audit events dropped so far (observability / alerting hook).
pub fn dropped_count() -> u64 {
    DROPPED.load(Ordering::Relaxed)
}

/// Approximate number of in-flight (unconsumed) events, or `None` before init.
pub fn queue_depth() -> Option<usize> {
    AUDIT_QUEUE
        .get()
        .map(|tx| QUEUE_CAPACITY - tx.capacity())
}

/// Background worker: drains the channel into time-windowed batches and flushes each.
async fn audit_writer_worker(mut rx: mpsc::Receiver<AuditEvent>) {
    loop {
        // Block until the first event of the next batch arrives.
        let first = match rx.recv().await {
            Some(ev) => ev,
            None => {
                info!("Audit queue closed, writer exiting");
                return;
            }
        };

        // Collect additional events within the flush window (bounded by batch size).
        let mut batch = vec![first];
        let deadline = tokio::time::sleep(Duration::from_millis(FLUSH_WINDOW_MS));
        tokio::pin!(deadline);

        loop {
            if batch.len() >= MAX_BATCH_SIZE {
                break;
            }
            tokio::select! {
                biased;
                ev = rx.recv() => match ev {
                    Some(ev) => batch.push(ev),
                    None => break,
                },
                _ = &mut deadline => break,
            }
        }

        flush_batch(batch).await;
    }
}

/// Persist a batch, best-effort: one INSERT per event so a single bad row cannot lose
/// the whole batch. Failures are logged and counted; they are never retried.
async fn flush_batch(batch: Vec<AuditEvent>) {
    use crate::db::{DatabasePool, DATABASE_POOL};

    let pool = match DATABASE_POOL.get() {
        Some(DatabasePool::Postgres(p)) => p,
        _ => {
            // Audit persistence targets PostgreSQL; drop (and count) on other backends.
            let dropped = batch.len() as u64;
            DROPPED.fetch_add(dropped, Ordering::Relaxed);
            error!(
                "Audit writer: no PostgreSQL pool available; dropped {} audit events",
                dropped
            );
            return;
        }
    };

    for event in &batch {
        if let Err(e) = crate::db::audit::insert_event(pool, event).await {
            // Best-effort: log and continue; do not retry or back-pressure the caller.
            error!(
                "Audit writer: failed to persist event {}: {}",
                event.event_id, e
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_audit_before_init_drops_and_counts() {
        // No test initialises the writer, so record_audit takes the uninitialised
        // branch and drops the event. Use a before/after delta so the assertion is
        // robust regardless of test execution order within the binary.
        let before = dropped_count();
        record_audit(AuditEvent::new("memory.write", "ltm_entry"));
        assert!(
            dropped_count() >= before + 1,
            "uninitialised record_audit must drop and count the event"
        );
    }

    #[test]
    fn batching_constants_are_sane() {
        assert!(MAX_BATCH_SIZE <= QUEUE_CAPACITY);
        assert!(MAX_BATCH_SIZE > 0);
        assert!(FLUSH_WINDOW_MS > 0);
    }
}
