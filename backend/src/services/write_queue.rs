//! Issue #57: Batch Async Write Queue for SQLite
//!
//! Eliminates SQLite write-amplification and lock-contention in high-concurrency scenarios
//! by funnelling all writes through a single tokio channel. Writes are coalesced into
//! time-window batches and applied sequentially, so the SQLite WAL sees one writer at a time.

use std::sync::OnceLock;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tracing::{error, info, warn};

/// A pending write operation: a raw SQL string + optional bind values serialised as JSON.
#[derive(Debug)]
pub struct WriteOp {
    /// Raw SQL statement (must not contain user-controlled data unsanitised)
    pub sql: String,
    /// JSON-encoded positional parameters for the statement (SQLite only supports ?-style binds)
    pub params_json: Option<String>,
    /// Optional reply channel: caller awaits the result if they care about the outcome.
    pub reply: Option<oneshot::Sender<Result<u64, String>>>,
}

/// Global write-queue sender — set once at startup via [`init_write_queue`].
static WRITE_QUEUE: OnceLock<mpsc::Sender<WriteOp>> = OnceLock::new();

/// Queue capacity: 4096 pending ops before back-pressure kicks in.
const QUEUE_CAPACITY: usize = 4096;

/// Maximum number of ops to batch together in one flush cycle.
const MAX_BATCH_SIZE: usize = 256;

/// Time window to collect ops before flushing (milliseconds).
const FLUSH_WINDOW_MS: u64 = 5;

/// Initialise the background write-queue worker.
///
/// Must be called once after the database pool is ready. Subsequent calls are no-ops.
pub fn init_write_queue() {
    if WRITE_QUEUE.get().is_some() {
        return;
    }

    let (tx, rx) = mpsc::channel::<WriteOp>(QUEUE_CAPACITY);
    WRITE_QUEUE
        .set(tx)
        .expect("write queue already initialised");

    tokio::spawn(write_queue_worker(rx));
    info!(
        "SQLite write-queue worker started (batch={}, window={}ms)",
        MAX_BATCH_SIZE, FLUSH_WINDOW_MS
    );
}

/// Enqueue a fire-and-forget write.
///
/// Returns `Err` if the queue is full (back-pressure).
pub fn enqueue(sql: impl Into<String>, params_json: Option<String>) -> Result<(), String> {
    let sender = WRITE_QUEUE
        .get()
        .ok_or_else(|| "write queue not initialised".to_string())?;

    sender
        .try_send(WriteOp {
            sql: sql.into(),
            params_json,
            reply: None,
        })
        .map_err(|e| format!("write queue full: {}", e))
}

/// Enqueue a write and await its result.
pub async fn enqueue_and_wait(
    sql: impl Into<String>,
    params_json: Option<String>,
) -> Result<u64, String> {
    let sender = WRITE_QUEUE
        .get()
        .ok_or_else(|| "write queue not initialised".to_string())?;

    let (reply_tx, reply_rx) = oneshot::channel();
    sender
        .send(WriteOp {
            sql: sql.into(),
            params_json,
            reply: Some(reply_tx),
        })
        .await
        .map_err(|e| format!("write queue closed: {}", e))?;

    timeout(Duration::from_secs(10), reply_rx)
        .await
        .map_err(|_| "write queue timeout".to_string())?
        .map_err(|_| "write queue reply channel dropped".to_string())?
}

/// Background worker: drains the channel in batches and executes them sequentially.
async fn write_queue_worker(mut rx: mpsc::Receiver<WriteOp>) {
    loop {
        // Block until the first op arrives
        let first = match rx.recv().await {
            Some(op) => op,
            None => {
                info!("Write-queue channel closed, worker exiting");
                return;
            }
        };

        // Collect additional ops within the flush window
        let mut batch = vec![first];
        let deadline = tokio::time::sleep(Duration::from_millis(FLUSH_WINDOW_MS));
        tokio::pin!(deadline);

        loop {
            if batch.len() >= MAX_BATCH_SIZE {
                break;
            }
            tokio::select! {
                biased;
                op = rx.recv() => {
                    match op {
                        Some(op) => batch.push(op),
                        None => break,
                    }
                }
                _ = &mut deadline => break,
            }
        }

        flush_batch(batch).await;
    }
}

/// Execute a batch of write operations sequentially on the SQLite pool.
async fn flush_batch(ops: Vec<WriteOp>) {
    use crate::db::DatabasePool;

    let pool = match crate::db::DATABASE_POOL.get() {
        Some(DatabasePool::Sqlite(p)) => p,
        _ => {
            // Not running SQLite — reply with error to any waiting callers
            for op in ops {
                if let Some(reply) = op.reply {
                    let _ = reply.send(Err("write queue: not a SQLite backend".to_string()));
                }
            }
            return;
        }
    };

    for op in ops {
        let result = sqlx::query(&op.sql).execute(pool).await;
        if let Some(reply) = op.reply {
            let mapped = result.map(|r| r.rows_affected()).map_err(|e| e.to_string());
            if reply.send(mapped).is_err() {
                warn!("Write-queue: caller dropped its reply receiver");
            }
        } else if let Err(e) = result {
            error!("Write-queue batch error (fire-and-forget): {}", e);
        }
    }
}

/// Queue depth for observability.
pub fn queue_depth() -> Option<usize> {
    WRITE_QUEUE.get().map(|tx| QUEUE_CAPACITY - tx.capacity())
}
