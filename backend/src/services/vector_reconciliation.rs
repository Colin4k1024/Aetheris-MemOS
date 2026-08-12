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
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};

use crate::db::vector_outbox::{self, OutboxOperation};
use crate::db::vector_reconciliation::{
    DriftType, ReconciliationAction, ReconciliationMode, ReconciliationRepository,
};
use crate::db::{is_postgres, pool, DatabasePool, DATABASE_POOL};
use crate::services::prometheus_exporter::get_exporter;
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

impl Default for ReconciliationService {
    fn default() -> Self {
        Self::new()
    }
}

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
                if let Err(fail_err) = ReconciliationRepository::fail_run(&run_id, &msg).await {
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
                        .with_context(|| format!("enqueue tenant rewrite for {entry_id}"))?;
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
                        .with_context(|| format!("enqueue content rewrite for {entry_id}"))?;
                }
            }
        }

        Ok(summary)
    }
}

// ---------------------------------------------------------------------------
// Periodic scanner daemon
// ---------------------------------------------------------------------------

static STARTED: OnceLock<()> = OnceLock::new();

/// Floor for the scan interval. A scan scrolls every Qdrant point id and then
/// fetches each payload individually, so a small interval would keep Qdrant
/// saturated; `interval_seconds = 0` would additionally panic a timer.
const MIN_INTERVAL: Duration = Duration::from_secs(60);

/// Delay before the first scan so it does not compete with startup and the
/// Qdrant client is initialised — [`load_qdrant_points`] fails without it.
const FIRST_SCAN_DELAY: Duration = Duration::from_secs(60);

/// Start the periodic reconciliation scanner (idempotent).
///
/// No-op when disabled by config or when the backend is not PostgreSQL — the
/// reconciliation tables and the vector outbox both live in the PG schema.
///
/// This is the only backstop for outbox events that never reach Qdrant, so the
/// loop is deliberately hard to kill: an invalid mode degrades to `dry_run` and
/// a failed scan is logged and retried on the next cycle rather than breaking
/// out.
pub fn init_reconciliation_scanner(cfg: &crate::config::ReconciliationConfig) {
    if STARTED.set(()).is_err() {
        return;
    }
    if !cfg.enabled {
        info!("Vector reconciliation scanner disabled by config");
        return;
    }
    if !matches!(DATABASE_POOL.get(), Some(DatabasePool::Postgres(_))) {
        info!("Vector reconciliation scanner skipped (not PostgreSQL)");
        return;
    }

    // Validate the mode once here rather than on every scan: an unparseable
    // value would otherwise fail each scan an interval apart, in a log nobody
    // is watching.
    let mode = resolve_mode(&cfg.mode);
    let interval = resolve_interval(cfg.interval_seconds);

    tokio::spawn(async move {
        info!(
            mode = mode.as_str(),
            interval_seconds = interval.as_secs(),
            first_scan_in_seconds = FIRST_SCAN_DELAY.as_secs(),
            "Vector reconciliation scanner started"
        );
        run_periodic(interval, mode).await;
    });
}

/// Resolve the configured mode string, falling back to `dry_run` on anything
/// unparseable.
///
/// The fallback direction matters: `dry_run` is read-only, so a typo degrades
/// into "detect but do not touch Qdrant" and can never escalate into
/// unintended bulk writes.
fn resolve_mode(configured: &str) -> ReconciliationMode {
    match ReconciliationMode::parse(configured) {
        Ok(m) => m,
        Err(e) => {
            warn!(
                configured_mode = %configured,
                error = %e,
                "Invalid reconciliation.mode; falling back to dry_run (read-only)"
            );
            ReconciliationMode::DryRun
        }
    }
}

/// Clamp the configured interval up to [`MIN_INTERVAL`].
///
/// `interval_seconds = 0` would otherwise turn the loop into a hot spin against
/// PostgreSQL and Qdrant.
fn resolve_interval(configured_seconds: u64) -> Duration {
    let interval = Duration::from_secs(configured_seconds).max(MIN_INTERVAL);
    if interval.as_secs() != configured_seconds {
        warn!(
            configured_seconds,
            effective_seconds = interval.as_secs(),
            "reconciliation.interval_seconds raised to the supported minimum"
        );
    }
    interval
}

async fn run_periodic(interval: Duration, mode: ReconciliationMode) {
    tokio::time::sleep(FIRST_SCAN_DELAY).await;

    let service = ReconciliationService::new();
    let exporter = get_exporter();

    // Seed the staleness gauge before the first scan. Unset it reads 0, and
    // `time() - 0` would make the staleness alert fire on every boot.
    exporter.stamp_reconciliation_scan_time();

    loop {
        match service.scan(mode.as_str()).await {
            Ok(summary) => {
                exporter.set_reconciliation_missing(summary.missing_count as f64);
                exporter.set_reconciliation_orphan(summary.orphan_count as f64);
                exporter.set_reconciliation_tenant_mismatch(summary.tenant_mismatch_count as f64);
                exporter.set_reconciliation_content_hash_mismatch(
                    summary.content_hash_mismatch_count as f64,
                );
                exporter.set_reconciliation_scanned(summary.total_scanned as f64);
                exporter.stamp_reconciliation_scan_time();

                if summary.total_drifts() > 0 {
                    warn!(
                        missing = summary.missing_count,
                        orphan = summary.orphan_count,
                        tenant_mismatch = summary.tenant_mismatch_count,
                        content_hash_mismatch = summary.content_hash_mismatch_count,
                        total_drifts = summary.total_drifts(),
                        mode = mode.as_str(),
                        "Vector drift detected between PostgreSQL and Qdrant"
                    );
                }
            }
            Err(e) => {
                // Never break the loop — a transient Qdrant or DB outage must
                // not permanently disable the backstop for the rest of the
                // process lifetime.
                //
                // Deliberately do NOT stamp the staleness gauge here. Stamping
                // on failure would report "scanning fine" for a scanner that
                // fails every single cycle, which is precisely the state the
                // staleness alert exists to catch. A persistently failing
                // scanner must go stale.
                error!(
                    error = %format!("{e:#}"),
                    "Reconciliation scan failed; retrying next cycle"
                );
            }
        }

        // Sleep *after* the scan instead of using `tokio::time::interval`: a
        // scan can outlast its period, and `interval` would then fire
        // back-to-back with no gap between scans.
        tokio::time::sleep(interval).await;
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

    // --- Daemon config resolution ---------------------------------------- //

    /// Anti-drift guard: the shipped config default must be a mode the scanner
    /// can actually parse.
    ///
    /// This exists because it was violated: the default was first written as
    /// `"report"`, which `ReconciliationMode::parse` rejects — the scanner
    /// would have started, then failed every scan an hour apart while looking
    /// wired up. Nothing in the type system connects a `String` default to the
    /// enum, so only a test can hold the two together.
    #[test]
    fn config_default_mode_parses() {
        let default_cfg = crate::config::ReconciliationConfig::default();
        assert!(
            ReconciliationMode::parse(&default_cfg.mode).is_ok(),
            "default reconciliation mode {:?} is not accepted by \
             ReconciliationMode::parse — the scanner would fail every scan",
            default_cfg.mode
        );
    }

    /// The default must be the read-only mode: `repair` enqueues bulk Qdrant
    /// writes and has to be opted into explicitly.
    #[test]
    fn config_default_mode_is_read_only() {
        let default_cfg = crate::config::ReconciliationConfig::default();
        assert_eq!(
            resolve_mode(&default_cfg.mode),
            ReconciliationMode::DryRun,
            "default mode must be read-only"
        );
    }

    #[test]
    fn resolve_mode_accepts_both_valid_modes() {
        assert_eq!(resolve_mode("dry_run"), ReconciliationMode::DryRun);
        assert_eq!(resolve_mode("repair"), ReconciliationMode::Repair);
    }

    /// An unparseable mode must degrade to read-only, never to `repair`.
    #[test]
    fn resolve_mode_falls_back_to_dry_run_not_repair() {
        for bad in ["report", "", "REPAIR", "dry-run", "repiar"] {
            assert_eq!(
                resolve_mode(bad),
                ReconciliationMode::DryRun,
                "mode {bad:?} must fall back to dry_run"
            );
        }
    }

    /// `interval_seconds = 0` must not survive into the loop — it would hot
    /// spin against PostgreSQL and Qdrant.
    #[test]
    fn resolve_interval_clamps_zero_and_small_values_to_floor() {
        assert_eq!(resolve_interval(0), MIN_INTERVAL);
        assert_eq!(resolve_interval(1), MIN_INTERVAL);
        assert_eq!(resolve_interval(MIN_INTERVAL.as_secs() - 1), MIN_INTERVAL);
    }

    #[test]
    fn resolve_interval_preserves_values_at_or_above_floor() {
        assert_eq!(resolve_interval(MIN_INTERVAL.as_secs()), MIN_INTERVAL);
        assert_eq!(resolve_interval(3600), Duration::from_secs(3600));
    }

    #[test]
    fn config_default_interval_survives_clamping() {
        let default_cfg = crate::config::ReconciliationConfig::default();
        assert_eq!(
            resolve_interval(default_cfg.interval_seconds).as_secs(),
            default_cfg.interval_seconds,
            "default interval must not be silently rewritten by the floor"
        );
    }

    /// The scanner is enabled by default: shipping it disabled would preserve
    /// the original defect (drift never detected) while looking implemented.
    #[test]
    fn config_default_is_enabled() {
        assert!(crate::config::ReconciliationConfig::default().enabled);
    }

    // --- Staleness signal ------------------------------------------------- //

    /// The staleness gauge must advance to roughly now, not stay at 0.
    ///
    /// A `0` reading makes `time() - gauge` ~56 years, so the staleness alert
    /// would fire on every boot and get muted — which would then hide a real
    /// stalled scanner.
    #[test]
    fn stamping_scan_time_sets_a_plausible_unix_timestamp() {
        let exporter = get_exporter();
        exporter.stamp_reconciliation_scan_time();

        let metrics = exporter.registry().gather();
        let stamped = metrics
            .iter()
            .find(|m| m.get_name() == "reconciliation_last_scan_timestamp_seconds")
            .and_then(|m| m.get_metric().first())
            .map(|m| m.get_gauge().get_value())
            .expect("reconciliation_last_scan_timestamp_seconds must be registered");

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_secs_f64();

        assert!(
            stamped > 0.0,
            "gauge must not stay at 0 — time() - 0 would fire the staleness \
             alert on every boot"
        );
        assert!(
            (now - stamped).abs() < 60.0,
            "stamped {stamped} should be within 60s of now {now}"
        );
    }

    /// The staleness threshold in monitoring/alerts/aetheris-alerts.yml is
    /// 7800s, derived as 2x the default scan interval plus slack. If the default
    /// interval changes, that alert silently stops being a meaningful bound —
    /// this pins the relationship the comment there claims.
    #[test]
    fn alert_staleness_threshold_still_exceeds_two_scan_intervals() {
        const ALERT_THRESHOLD_SECONDS: u64 = 7800;
        let default_interval = crate::config::ReconciliationConfig::default().interval_seconds;
        assert!(
            ALERT_THRESHOLD_SECONDS > 2 * default_interval,
            "VectorReconciliationScannerStalled uses {ALERT_THRESHOLD_SECONDS}s but the \
             default interval is now {default_interval}s; update the alert in \
             monitoring/alerts/aetheris-alerts.yml"
        );
    }
}
