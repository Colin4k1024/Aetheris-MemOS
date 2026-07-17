//! Background worker: claim `memory_vector_outbox` → deliver to Qdrant → mark status.
//!
//! PostgreSQL only. Started from `main` after the DB pool is ready.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use tracing::{error, info, warn};
use ulid::Ulid;

use crate::db::vector_outbox::{self, OutboxOperation};
use crate::db::{pool, DatabasePool, DATABASE_POOL};
use crate::services::qdrant::get_qdrant_client;

static STARTED: OnceLock<()> = OnceLock::new();
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const BATCH_SIZE: i64 = 32;
const MAX_ATTEMPTS: i32 = 8;
const STALE_LOCK_SECS: i64 = 120;
const RECLAIM_EVERY_N_LOOPS: u32 = 15;

/// Start the outbox worker (idempotent). No-op when not on PostgreSQL.
pub fn init_outbox_worker() {
    if STARTED.set(()).is_err() {
        return;
    }
    if !matches!(DATABASE_POOL.get(), Some(DatabasePool::Postgres(_))) {
        info!("Outbox worker skipped (not PostgreSQL)");
        return;
    }

    let worker_id = format!("outbox-{}", Ulid::new());
    tokio::spawn(async move {
        info!(%worker_id, "Vector outbox worker started");
        run_loop(&worker_id).await;
        info!(%worker_id, "Vector outbox worker stopped");
    });
}

/// Signal the worker to stop after the current batch (best-effort).
#[allow(dead_code)]
pub fn request_shutdown() {
    SHUTDOWN.store(true, Ordering::Relaxed);
}

async fn run_loop(worker_id: &str) {
    let mut loops: u32 = 0;
    while !SHUTDOWN.load(Ordering::Relaxed) {
        loops = loops.wrapping_add(1);
        if loops % RECLAIM_EVERY_N_LOOPS == 0 {
            if let Err(e) = vector_outbox::reclaim_stale(pool(), STALE_LOCK_SECS).await {
                warn!("outbox reclaim_stale: {}", e);
            }
        }

        match process_batch(worker_id).await {
            Ok(0) => tokio::time::sleep(POLL_INTERVAL).await,
            Ok(n) => {
                info!(applied_or_failed = n, "outbox batch processed");
            }
            Err(e) => {
                error!("outbox batch error: {}", e);
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    }
}

async fn process_batch(worker_id: &str) -> Result<usize, crate::AppError> {
    let events = vector_outbox::claim_batch(pool(), worker_id, BATCH_SIZE).await?;
    if events.is_empty() {
        return Ok(0);
    }

    let qdrant = match get_qdrant_client() {
        Ok(c) => c,
        Err(e) => {
            for ev in &events {
                let _ = vector_outbox::mark_failed(
                    pool(),
                    &ev.event_id,
                    ev.attempt_count,
                    MAX_ATTEMPTS,
                    &format!("qdrant client unavailable: {e}"),
                )
                .await;
            }
            return Err(crate::AppError::Internal(format!(
                "qdrant client unavailable: {e}"
            )));
        }
    };

    let mut handled = 0usize;
    for ev in events {
        let result = deliver_one(&qdrant, &ev).await;
        match result {
            Ok(()) => {
                if let Err(e) = vector_outbox::mark_applied(pool(), &ev.event_id).await {
                    error!(event_id = %ev.event_id, "mark_applied failed: {}", e);
                } else {
                    handled += 1;
                }
            }
            Err(err) => {
                warn!(
                    event_id = %ev.event_id,
                    entry_id = %ev.entry_id,
                    "outbox delivery failed: {}",
                    err
                );
                let _ = vector_outbox::mark_failed(
                    pool(),
                    &ev.event_id,
                    ev.attempt_count,
                    MAX_ATTEMPTS,
                    &err,
                )
                .await;
                handled += 1;
            }
        }
    }
    Ok(handled)
}

async fn deliver_one(
    qdrant: &crate::services::qdrant::QdrantClient,
    ev: &vector_outbox::OutboxEvent,
) -> Result<(), String> {
    match ev.operation {
        OutboxOperation::Delete => qdrant
            .delete_vectors(vec![ev.entry_id.clone()])
            .await
            .map_err(|e| e.to_string()),
        OutboxOperation::Upsert => {
            let payload: serde_json::Value =
                serde_json::from_str(&ev.payload_json).map_err(|e| e.to_string())?;
            let vector = payload
                .get("vector")
                .and_then(|v| serde_json::from_value::<Vec<f32>>(v.clone()).ok())
                .ok_or_else(|| "outbox payload missing vector".to_string())?;
            let mut metadata = payload
                .get("metadata")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            if let Some(obj) = metadata.as_object_mut() {
                obj.insert(
                    "tenantId".to_string(),
                    serde_json::Value::String(ev.tenant_id.clone()),
                );
                if let Some(hash) = payload.get("content_hash") {
                    obj.insert("contentHash".to_string(), hash.clone());
                }
                obj.insert(
                    "entryId".to_string(),
                    serde_json::Value::String(ev.entry_id.clone()),
                );
            }
            qdrant
                .insert_vectors(vec![vector], vec![ev.entry_id.clone()], vec![metadata])
                .await
                .map_err(|e| e.to_string())
        }
    }
}
