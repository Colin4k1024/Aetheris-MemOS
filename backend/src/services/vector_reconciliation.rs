//! Vector reconciliation scanner (W1.1).
//!
//! Detects drift between PostgreSQL `knowledge_entries` and Qdrant points, and
//! (in `repair` mode) enqueues outbox events to fix the drift. The four drift
//! types are: `missing` (DB entry with no Qdrant point), `orphan` (Qdrant point
//! with no DB entry), `tenant_mismatch`, and `content_hash_mismatch`.
//!
//! PostgreSQL-only — the reconciliation tables and the vector outbox live in
//! the PostgreSQL schema. The Qdrant client is read via
//! [`crate::services::qdrant::get_qdrant_client`].

use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};

use crate::db::vector_outbox::{self, OutboxOperation};
use crate::db::vector_reconciliation::{
    DriftType, ReconciliationAction, ReconciliationMode, ReconciliationRepository,
};
use crate::db::{is_postgres, pool};
use crate::services::qdrant::get_qdrant_client;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationSummary {
    pub missing_count: usize,
    pub orphan_count: usize,
    pub tenant_mismatch_count: usize,
    pub content_hash_mismatch_count: usize,
    pub total_scanned: usize,
    pub mode: String,
}

impl ReconciliationSummary {
    pub fn total_drifts(&self) -> usize {
        self.missing_count
            + self.orphan_count
            + self.tenant_mismatch_count
            + self.content_hash_mismatch_count
    }
}

pub struct ReconciliationService;

impl ReconciliationService {
    pub fn new() -> Self {
        Self
    }

    #[instrument(skip(self))]
    pub async fn scan(&self, mode: &str) -> Result<ReconciliationSummary> {
        if !is_postgres() {
            anyhow::bail!("vector reconciliation requires PostgreSQL backend");
        }

        let mode_enum = ReconciliationMode::parse(mode).context("invalid reconciliation mode")?;
        let is_repair = matches!(mode_enum, ReconciliationMode::Repair);

        let run_id = ReconciliationRepository::create_run(mode)
            .await
            .context("failed to create reconciliation run")?;

        let result = self.run_scan(&run_id, is_repair, mode_enum).await;

        match result {
            Ok(summary) => {
                let summary_json =
                    serde_json::to_string(&summary).unwrap_or_else(|_| "{}".to_string());
                ReconciliationRepository::complete_run(&run_id, &summary_json)
                    .await
                    .context("failed to complete reconciliation run")?;

                info!(
                    mode = mode,
                    run_id = %run_id,
                    missing = summary.missing_count,
                    orphan = summary.orphan_count,
                    tenant_mismatch = summary.tenant_mismatch_count,
                    content_hash_mismatch = summary.content_hash_mismatch_count,
                    total_scanned = summary.total_scanned,
                    "Reconciliation scan complete"
                );
                Ok(summary)
            }
            Err(e) => {
                let msg = format!("{e:#}");
                if let Err(fail_err) =
                    ReconciliationRepository::fail_run(&run_id, &msg).await
                {
                    error!(run_id = %run_id, error = %fail_err, "failed to mark run as failed");
                }
                Err(e)
            }
        }
    }

    async fn run_scan(
        &self,
        run_id: &str,
        is_repair: bool,
        mode_enum: ReconciliationMode,
    ) -> Result<ReconciliationSummary> {
        let db_entries = load_db_entries()
            .await
            .context("failed to load DB entries")?;
        let total_scanned = db_entries.len();

        let qdrant_points = load_qdrant_points()
            .await
            .context("failed to load Qdrant points")?;

        let mut summary = ReconciliationSummary {
            missing_count: 0,
            orphan_count: 0,
            tenant_mismatch_count: 0,
            content_hash_mismatch_count: 0,
            total_scanned,
            mode: mode_enum.as_str().to_string(),
        };

        for (entry_id, db_entry) in &db_entries {
            if !qdrant_points.contains_key(entry_id) {
                summary.missing_count += 1;
                let action = if is_repair {
                    ReconciliationAction::Upsert
                } else {
                    ReconciliationAction::Report
                };
                ReconciliationRepository::add_item(
                    run_id,
                    DriftType::Missing.as_str(),
                    Some(entry_id),
                    action.as_str(),
                )
                .await
                .context("failed to add missing item")?;
                if is_repair {
                    enqueue_upsert(entry_id, db_entry)
                        .await
                        .with_context(|| format!("enqueue upsert for missing {entry_id}"))?;
                }
            }
        }

        for (point_id, qdrant_entry) in &qdrant_points {
            if !db_entries.contains_key(point_id) {
                summary.orphan_count += 1;
                let action = if is_repair {
                    ReconciliationAction::Delete
                } else {
                    ReconciliationAction::Report
                };
                ReconciliationRepository::add_item(
                    run_id,
                    DriftType::Orphan.as_str(),
                    Some(point_id),
                    action.as_str(),
                )
                .await
                .context("failed to add orphan item")?;
                if is_repair {
                    let tenant_id = qdrant_entry.tenant_id.as_deref().unwrap_or("");
                    enqueue_delete(point_id, tenant_id)
                        .await
                        .with_context(|| format!("enqueue delete for orphan {point_id}"))?;
                }
            }
        }

        for (entry_id, db_entry) in &db_entries {
            let Some(qdrant_entry) = qdrant_points.get(entry_id) else {
                continue;
            };

            let db_tenant = db_entry.tenant_id.as_deref();
            let qdrant_tenant = qdrant_entry.tenant_id.as_deref();
            if db_tenant != qdrant_tenant {
                summary.tenant_mismatch_count += 1;
                let action = if is_repair {
                    ReconciliationAction::RewritePayload
                } else {
                    ReconciliationAction::Report
                };
                ReconciliationRepository::add_item(
                    run_id,
                    DriftType::TenantMismatch.as_str(),
                    Some(entry_id),
                    action.as_str(),
                )
                .await
                .context("failed to add tenant_mismatch item")?;
                if is_repair {
                    enqueue_upsert(entry_id, db_entry)
                        .await
                        .with_context(|| {
                            format!("enqueue tenant rewrite for {entry_id}")
                        })?;
                }
            }

            let db_hash = db_entry.content_hash.as_deref();
            let qdrant_hash = qdrant_entry.content_hash.as_deref();
            if db_hash != qdrant_hash {
                summary.content_hash_mismatch_count += 1;
                let action = if is_repair {
                    ReconciliationAction::Upsert
                } else {
                    ReconciliationAction::Report
                };
                ReconciliationRepository::add_item(
                    run_id,
                    DriftType::ContentHashMismatch.as_str(),
                    Some(entry_id),
                    action.as_str(),
                )
                .await
                .context("failed to add content_hash_mismatch item")?;
                if is_repair {
                    enqueue_upsert(entry_id, db_entry)
                        .await
                        .with_context(|| {
                            format!("enqueue content rewrite for {entry_id}")
                        })?;
                }
            }
        }

        Ok(summary)
    }
}

#[derive(Debug, Clone)]
struct DbEntry {
    tenant_id: Option<String>,
    content_hash: Option<String>,
    embedding_vector: Option<String>,
}

#[derive(Debug, Clone)]
struct QdrantEntry {
    tenant_id: Option<String>,
    content_hash: Option<String>,
}

async fn load_db_entries() -> Result<HashMap<String, DbEntry>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        entry_id: String,
        tenant_id: Option<String>,
        content_hash: String,
        embedding_vector: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT entry_id, tenant_id, content_hash, embedding_vector
        FROM knowledge_entries
        WHERE status = 'active'
        "#,
    )
    .fetch_all(pool())
    .await
    .map_err(|e| {
        error!("Failed to load DB entries: {}", e);
        anyhow::anyhow!("Database error: {}", e)
    })?;

    let mut map = HashMap::with_capacity(rows.len());
    for row in rows {
        map.insert(
            row.entry_id,
            DbEntry {
                tenant_id: row.tenant_id,
                content_hash: Some(row.content_hash),
                embedding_vector: Some(row.embedding_vector),
            },
        );
    }
    Ok(map)
}

async fn load_qdrant_points() -> Result<HashMap<String, QdrantEntry>> {
    let qdrant = get_qdrant_client().context("Qdrant client not initialized")?;
    let point_ids = qdrant.scroll_point_ids().await?;

    let mut map = HashMap::with_capacity(point_ids.len());
    for point_id in &point_ids {
        let payload = qdrant.get_point_payload(point_id).await?;
        let Some(payload) = payload else {
            warn!(point_id = %point_id, "Qdrant point payload missing on re-fetch");
            continue;
        };
        let tenant_id = payload
            .get("tenantId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let content_hash = payload
            .get("contentHash")
            .or_else(|| payload.get("content_hash"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        map.insert(
            point_id.clone(),
            QdrantEntry {
                tenant_id,
                content_hash,
            },
        );
    }
    Ok(map)
}

async fn enqueue_upsert(entry_id: &str, db_entry: &DbEntry) -> Result<()> {
    let tenant_id = db_entry.tenant_id.as_deref().unwrap_or("").to_string();
    let embedding_json = db_entry.embedding_vector.as_deref().unwrap_or("[]");
    let vector: Vec<f32> = serde_json::from_str(embedding_json)
        .with_context(|| format!("failed to parse embedding vector for {entry_id}"))?;
    let content_hash = db_entry.content_hash.as_deref().unwrap_or("").to_string();

    let metadata = serde_json::json!({
        "tenantId": tenant_id,
        "contentHash": content_hash,
        "entryId": entry_id,
    });
    let outbox_payload = serde_json::json!({
        "vector": vector,
        "metadata": metadata,
        "content_hash": content_hash,
    });
    let payload_json = serde_json::to_string(&outbox_payload)?;
    let payload_hash = crate::services::information_guard::compute_sha256(&payload_json);
    let idempotency_key = vector_outbox::upsert_idempotency_key(entry_id, &payload_hash);

    let mut tx = pool().begin().await.map_err(|e| {
        error!("failed to begin outbox tx: {}", e);
        anyhow::anyhow!("Database error: {}", e)
    })?;
    vector_outbox::insert_event_tx(
        &mut tx,
        &tenant_id,
        entry_id,
        OutboxOperation::Upsert,
        &payload_json,
        &payload_hash,
        &idempotency_key,
    )
    .await
    .map_err(|e| anyhow::anyhow!("failed to insert outbox event: {e}"))?;
    tx.commit().await.map_err(|e| {
        error!("failed to commit outbox tx: {}", e);
        anyhow::anyhow!("Database error: {}", e)
    })?;
    Ok(())
}

async fn enqueue_delete(entry_id: &str, tenant_id: &str) -> Result<()> {
    let idempotency_key = vector_outbox::delete_idempotency_key(entry_id);

    let mut tx = pool().begin().await.map_err(|e| {
        error!("failed to begin outbox tx: {}", e);
        anyhow::anyhow!("Database error: {}", e)
    })?;
    vector_outbox::insert_event_tx(
        &mut tx,
        tenant_id,
        entry_id,
        OutboxOperation::Delete,
        "",
        "",
        &idempotency_key,
    )
    .await
    .map_err(|e| anyhow::anyhow!("failed to insert outbox event: {e}"))?;
    tx.commit().await.map_err(|e| {
        error!("failed to commit outbox tx: {}", e);
        anyhow::anyhow!("Database error: {}", e)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_total_drifts_sums_all_counts() {
        let s = ReconciliationSummary {
            missing_count: 2,
            orphan_count: 3,
            tenant_mismatch_count: 1,
            content_hash_mismatch_count: 4,
            total_scanned: 100,
            mode: "dry_run".to_string(),
        };
        assert_eq!(s.total_drifts(), 10);
    }

    #[test]
    fn summary_no_drifts_has_zero_total() {
        let s = ReconciliationSummary {
            missing_count: 0,
            orphan_count: 0,
            tenant_mismatch_count: 0,
            content_hash_mismatch_count: 0,
            total_scanned: 50,
            mode: "dry_run".to_string(),
        };
        assert_eq!(s.total_drifts(), 0);
    }

    #[test]
    fn summary_serializes_to_json() {
        let s = ReconciliationSummary {
            missing_count: 1,
            orphan_count: 2,
            tenant_mismatch_count: 0,
            content_hash_mismatch_count: 0,
            total_scanned: 10,
            mode: "repair".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: ReconciliationSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(back.missing_count, 1);
        assert_eq!(back.orphan_count, 2);
        assert_eq!(back.mode, "repair");
    }
}
