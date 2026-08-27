//! Prometheus Metrics Exporter
//!
//! Exports internal metrics in Prometheus text format for scraping.

use std::time::Duration;

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::services::metrics;

/// Prometheus metrics exporter state
pub struct PrometheusExporter {
    /// Active STM sessions count
    stm_sessions_active: prometheus::Gauge,
    /// Total LTM entries count
    ltm_entries_total: prometheus::Gauge,
    /// Weight adjustment operations counter
    weight_adjustments_total: prometheus::Counter,
    /// STM to LTM transfer operations counter
    transfer_operations_total: prometheus::Counter,
    /// Search duration histogram (in seconds)
    search_duration_seconds: prometheus::Histogram,
    /// Request duration histogram (in seconds)
    request_duration_seconds: prometheus::Histogram,
    /// Requests counter by endpoint and status
    requests_total: prometheus::CounterVec,
    /// Authentication failures counter, labelled by a bounded `reason`
    /// (missing_token / invalid_token / expired / query_param_token).
    auth_failures_total: prometheus::CounterVec,
    /// LTM writes stored with the LLM summary degraded (deferred), labelled by a
    /// bounded `reason` (unavailable / upstream_error). A climbing value means
    /// the LLM backend is failing so summaries are being deferred for later
    /// backfill — writes are NOT blocked, but retrieval on those entries is
    /// temporarily weaker until backfilled.
    ltm_summary_degraded_total: prometheus::CounterVec,
    /// Tenant quota usage ratio
    tenant_quota_usage_ratio: prometheus::GaugeVec,
    /// Outbox pending events count
    outbox_pending_total: prometheus::Gauge,
    /// Outbox dead letter events count
    outbox_dead_letter_total: prometheus::Counter,
    /// Outbox processing duration histogram (in seconds)
    outbox_processing_duration_seconds: prometheus::Histogram,
    /// Outbox Qdrant upsert success count
    outbox_qdrant_upsert_success_total: prometheus::Counter,
    /// Outbox Qdrant upsert failure count
    outbox_qdrant_upsert_failure_total: prometheus::Counter,
    /// Reconciliation missing count (DB entry with no Qdrant point)
    reconciliation_missing_entries: prometheus::Gauge,
    /// Reconciliation orphan count (Qdrant point with no DB entry)
    reconciliation_orphan_points: prometheus::Gauge,
    /// Reconciliation tenant mismatch count
    reconciliation_tenant_mismatch_entries: prometheus::Gauge,
    /// Reconciliation content hash mismatch count
    reconciliation_content_hash_mismatch_entries: prometheus::Gauge,
    /// Reconciliation total scanned entries count
    reconciliation_scanned_entries: prometheus::Gauge,
    /// Unix timestamp of the last completed reconciliation scan.
    ///
    /// Exists because the four drift gauges **hold their last value between
    /// scans**, so a wedged scan loop inside a live process is invisible to
    /// them — the backstop would be dead while its metrics looked calm. Alert on
    /// `time() - this > 2 * scan interval` to detect that.
    reconciliation_last_scan_timestamp_seconds: prometheus::Gauge,
    /// Audit events spilled to the local disk buffer (queue full or INSERT failed).
    /// Not lost — pending replay on the next startup.
    audit_spilled_total: prometheus::Counter,
    /// Audit events replayed from the spill buffer into the database at startup.
    audit_replayed_total: prometheus::Counter,
    /// Audit events dropped as a **last resort** (spill write failed / cap exceeded /
    /// no writer). Non-zero means real audit loss — the counter to alert on.
    audit_dropped_total: prometheus::Counter,
    /// Spill lines skipped during replay because they could not be parsed
    /// (truncated tail from a crash mid-write, or corruption).
    audit_truncated_skipped_total: prometheus::Counter,
    /// Active WebSocket connections (#86)
    ws_connections_active: prometheus::Gauge,
    /// WebSocket events dropped because the client lagged behind the broadcast (#86)
    ws_lagged_drops_total: prometheus::Counter,
    consolidation_run_duration_seconds: prometheus::Histogram,
    consolidation_runs_total: prometheus::Counter,
    consolidation_conflicts_total: prometheus::Counter,
    consolidation_stale_marked_total: prometheus::Counter,
    consolidation_promises_expired_total: prometheus::Counter,
    consolidation_reconciliation_diffs_total: prometheus::Counter,
    consolidation_failures_total: prometheus::Counter,
    belief_active: prometheus::Gauge,
    recall_requests_total: prometheus::Counter,
    recall_items_total: prometheus::Counter,
    /// WebSocket frame send duration histogram (#86) — slow-client / backpressure signal.
    ws_send_duration_seconds: prometheus::Histogram,
    /// WebSocket broadcast channel queue depth (#86) — high values = slow consumer.
    ws_broadcast_queue_depth: prometheus::Gauge,
    /// Prometheus registry for metric collection
    registry: prometheus::Registry,
}

impl Default for PrometheusExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl PrometheusExporter {
    /// Create a new Prometheus exporter
    pub fn new() -> Self {
        let registry = prometheus::Registry::new();

        let stm_sessions_active = prometheus::Gauge::new(
            "memory_stm_sessions_active",
            "Number of active STM sessions",
        )
        .expect("gauge creation failed");

        let ltm_entries_total =
            prometheus::Gauge::new("memory_ltm_entries_total", "Total number of LTM entries")
                .expect("gauge creation failed");

        let weight_adjustments_total = prometheus::Counter::new(
            "memory_weight_adjustments_total",
            "Total number of weight adjustment operations",
        )
        .expect("counter creation failed");

        let transfer_operations_total = prometheus::Counter::new(
            "memory_transfer_operations_total",
            "Total number of STM to LTM transfer operations",
        )
        .expect("counter creation failed");

        let search_duration_seconds = prometheus::Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "memory_search_duration_seconds",
                "Search operation duration in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
        )
        .expect("histogram creation failed");

        let request_duration_seconds = prometheus::Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "memory_request_duration_seconds",
                "HTTP request duration in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
        )
        .expect("histogram creation failed");

        let requests_total = prometheus::CounterVec::new(
            prometheus::Opts::new(
                "memory_requests_total",
                "Total requests by endpoint and status",
            ),
            &["endpoint", "status"],
        )
        .expect("countervec creation failed");

        // Label is a bounded `reason` only. Deliberately NOT labelled by user,
        // tenant, or token content: those are unbounded (cardinality blow-up) and
        // sensitive (secret leakage into scraped metrics).
        let auth_failures_total = prometheus::CounterVec::new(
            prometheus::Opts::new(
                "auth_failures_total",
                "Total authentication failures by reason",
            ),
            &["reason"],
        )
        .expect("countervec creation failed");

        // Label is a bounded `reason` only (unavailable / upstream_error), for the
        // same cardinality/leakage reasons as auth_failures_total above.
        let ltm_summary_degraded_total = prometheus::CounterVec::new(
            prometheus::Opts::new(
                "ltm_summary_degraded_total",
                "LTM writes stored with a deferred (empty) LLM summary, by reason",
            ),
            &["reason"],
        )
        .expect("countervec creation failed");

        let tenant_quota_usage_ratio = prometheus::GaugeVec::new(
            prometheus::Opts::new(
                "tenant_quota_usage_ratio",
                "Quota usage ratio per tenant (0.0 to 1.0)",
            ),
            &["tenant"],
        )
        .expect("gauagevec creation failed");

        let outbox_pending_total =
            prometheus::Gauge::new("outbox_pending_total", "Number of pending outbox events")
                .expect("gauge creation failed");

        let outbox_dead_letter_total = prometheus::Counter::new(
            "outbox_dead_letter_total",
            "Total number of dead letter outbox events",
        )
        .expect("counter creation failed");

        let outbox_processing_duration_seconds = prometheus::Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "outbox_processing_duration_seconds",
                "Outbox batch processing duration in seconds",
            )
            .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0]),
        )
        .expect("histogram creation failed");

        let outbox_qdrant_upsert_success_total = prometheus::Counter::new(
            "outbox_qdrant_upsert_success_total",
            "Total number of successful Qdrant upsert operations",
        )
        .expect("counter creation failed");

        let outbox_qdrant_upsert_failure_total = prometheus::Counter::new(
            "outbox_qdrant_upsert_failure_total",
            "Total number of failed Qdrant upsert operations",
        )
        .expect("counter creation failed");

        let reconciliation_missing_entries = prometheus::Gauge::new(
            "reconciliation_missing_entries",
            "Number of DB entries missing from Qdrant",
        )
        .expect("gauge creation failed");

        let reconciliation_orphan_points = prometheus::Gauge::new(
            "reconciliation_orphan_points",
            "Number of Qdrant points with no matching DB entry",
        )
        .expect("gauge creation failed");

        let reconciliation_tenant_mismatch_entries = prometheus::Gauge::new(
            "reconciliation_tenant_mismatch_entries",
            "Number of entries with tenant ID mismatch between DB and Qdrant",
        )
        .expect("gauge creation failed");

        let reconciliation_content_hash_mismatch_entries = prometheus::Gauge::new(
            "reconciliation_content_hash_mismatch_entries",
            "Number of entries with content hash mismatch between DB and Qdrant",
        )
        .expect("gauge creation failed");

        let reconciliation_scanned_entries = prometheus::Gauge::new(
            "reconciliation_scanned_entries",
            "Total number of entries scanned in last reconciliation run",
        )
        .expect("gauge creation failed");

        let reconciliation_last_scan_timestamp_seconds = prometheus::Gauge::new(
            "reconciliation_last_scan_timestamp_seconds",
            "Unix timestamp of the last completed reconciliation scan (staleness signal)",
        )
        .expect("gauge creation failed");

        let audit_spilled_total = prometheus::Counter::new(
            "audit_spilled_total",
            "Total audit events spilled to the local disk buffer (pending replay)",
        )
        .expect("counter creation failed");

        let audit_replayed_total = prometheus::Counter::new(
            "audit_replayed_total",
            "Total audit events replayed from the spill buffer into the database",
        )
        .expect("counter creation failed");

        let audit_dropped_total = prometheus::Counter::new(
            "audit_dropped_total",
            "Total audit events dropped as a last resort (spill failed / cap exceeded / no writer)",
        )
        .expect("counter creation failed");

        let audit_truncated_skipped_total = prometheus::Counter::new(
            "audit_truncated_skipped_total",
            "Total spill lines skipped during replay (truncated tail or corruption)",
        )
        .expect("counter creation failed");

        let ws_connections_active = prometheus::Gauge::new(
            "ws_connections_active",
            "Number of active WebSocket connections",
        )
        .expect("gauge creation failed");

        let consolidation_run_duration_seconds = prometheus::Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "consolidation_run_duration_seconds",
                "Belief consolidation run duration in seconds (per tenant)",
            )
            .buckets(vec![0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
        )
        .expect("histogram creation failed");
        let consolidation_runs_total = prometheus::Counter::new(
            "consolidation_runs_total",
            "Belief consolidation runs started",
        )
        .expect("counter creation failed");
        let consolidation_conflicts_total = prometheus::Counter::new(
            "consolidation_conflicts_total",
            "Single-valued multi-active groups repaired into the conflict/confirm flow",
        )
        .expect("counter creation failed");
        let consolidation_stale_marked_total = prometheus::Counter::new(
            "consolidation_stale_marked_total",
            "Beliefs marked stale (or routed to the confirmation queue) by the stale scan",
        )
        .expect("counter creation failed");
        let consolidation_promises_expired_total = prometheus::Counter::new(
            "consolidation_promises_expired_total",
            "Time-bounded promise beliefs retired from the current set",
        )
        .expect("counter creation failed");
        let consolidation_reconciliation_diffs_total = prometheus::Counter::new(
            "consolidation_reconciliation_diffs_total",
            "SoR reconciliation differences found (closed, opened, or refreshed)",
        )
        .expect("counter creation failed");
        let consolidation_failures_total = prometheus::Counter::new(
            "consolidation_failures_total",
            "Belief consolidation operation failures",
        )
        .expect("counter creation failed");
        let belief_active = prometheus::Gauge::new(
            "memory_belief_active",
            "Current-truth (active, open-window) belief edges (#130 observability)",
        )
        .expect("gauge creation failed");
        let recall_requests_total = prometheus::Counter::new(
            "recall_requests_total",
            "Belief recall-core requests served (#130 observability)",
        )
        .expect("counter creation failed");
        let recall_items_total = prometheus::Counter::new(
            "recall_items_total",
            "Belief items returned into Working Memory (#130 observability)",
        )
        .expect("counter creation failed");
        let ws_lagged_drops_total = prometheus::Counter::new(
            "ws_lagged_drops_total",
            "WebSocket events dropped because the client lagged behind the broadcast",
        )
        .expect("counter creation failed");

        let ws_send_duration_seconds = prometheus::Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "ws_send_duration_seconds",
                "WebSocket frame send duration in seconds (slow-client / backpressure signal)",
            )
            .buckets(vec![
                0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0,
            ]),
        )
        .expect("histogram creation failed");

        let ws_broadcast_queue_depth = prometheus::Gauge::new(
            "ws_broadcast_queue_depth",
            "WebSocket broadcast channel queue depth — high values indicate slow consumers",
        )
        .expect("gauge creation failed");

        // Register all metrics with the registry
        registry
            .register(Box::new(stm_sessions_active.clone()))
            .expect("failed to register stm_sessions_active");
        registry
            .register(Box::new(ltm_entries_total.clone()))
            .expect("failed to register ltm_entries_total");
        registry
            .register(Box::new(weight_adjustments_total.clone()))
            .expect("failed to register weight_adjustments_total");
        registry
            .register(Box::new(transfer_operations_total.clone()))
            .expect("failed to register transfer_operations_total");
        registry
            .register(Box::new(search_duration_seconds.clone()))
            .expect("failed to register search_duration_seconds");
        registry
            .register(Box::new(request_duration_seconds.clone()))
            .expect("failed to register request_duration_seconds");
        registry
            .register(Box::new(requests_total.clone()))
            .expect("failed to register requests_total");
        registry
            .register(Box::new(auth_failures_total.clone()))
            .expect("failed to register auth_failures_total");
        registry
            .register(Box::new(ltm_summary_degraded_total.clone()))
            .expect("failed to register ltm_summary_degraded_total");
        registry
            .register(Box::new(tenant_quota_usage_ratio.clone()))
            .expect("failed to register tenant_quota_usage_ratio");
        registry
            .register(Box::new(outbox_pending_total.clone()))
            .expect("failed to register outbox_pending_total");
        registry
            .register(Box::new(outbox_dead_letter_total.clone()))
            .expect("failed to register outbox_dead_letter_total");
        registry
            .register(Box::new(outbox_processing_duration_seconds.clone()))
            .expect("failed to register outbox_processing_duration_seconds");
        registry
            .register(Box::new(outbox_qdrant_upsert_success_total.clone()))
            .expect("failed to register outbox_qdrant_upsert_success_total");
        registry
            .register(Box::new(outbox_qdrant_upsert_failure_total.clone()))
            .expect("failed to register outbox_qdrant_upsert_failure_total");
        registry
            .register(Box::new(reconciliation_missing_entries.clone()))
            .expect("failed to register reconciliation_missing_entries");
        registry
            .register(Box::new(reconciliation_orphan_points.clone()))
            .expect("failed to register reconciliation_orphan_points");
        registry
            .register(Box::new(reconciliation_tenant_mismatch_entries.clone()))
            .expect("failed to register reconciliation_tenant_mismatch_entries");
        registry
            .register(Box::new(
                reconciliation_content_hash_mismatch_entries.clone(),
            ))
            .expect("failed to register reconciliation_content_hash_mismatch_entries");
        registry
            .register(Box::new(reconciliation_scanned_entries.clone()))
            .expect("failed to register reconciliation_scanned_entries");
        registry
            .register(Box::new(reconciliation_last_scan_timestamp_seconds.clone()))
            .expect("failed to register reconciliation_last_scan_timestamp_seconds");
        registry
            .register(Box::new(audit_spilled_total.clone()))
            .expect("failed to register audit_spilled_total");
        registry
            .register(Box::new(audit_replayed_total.clone()))
            .expect("failed to register audit_replayed_total");
        registry
            .register(Box::new(audit_dropped_total.clone()))
            .expect("failed to register audit_dropped_total");
        registry
            .register(Box::new(audit_truncated_skipped_total.clone()))
            .expect("failed to register audit_truncated_skipped_total");
        registry
            .register(Box::new(ws_connections_active.clone()))
            .expect("failed to register ws_connections_active");
        registry
            .register(Box::new(ws_lagged_drops_total.clone()))
            .expect("failed to register ws_lagged_drops_total");
        registry
            .register(Box::new(consolidation_run_duration_seconds.clone()))
            .expect("failed to register consolidation_run_duration_seconds");
        registry
            .register(Box::new(consolidation_runs_total.clone()))
            .expect("failed to register consolidation_runs_total");
        registry
            .register(Box::new(consolidation_conflicts_total.clone()))
            .expect("failed to register consolidation_conflicts_total");
        registry
            .register(Box::new(consolidation_stale_marked_total.clone()))
            .expect("failed to register consolidation_stale_marked_total");
        registry
            .register(Box::new(consolidation_promises_expired_total.clone()))
            .expect("failed to register consolidation_promises_expired_total");
        registry
            .register(Box::new(consolidation_reconciliation_diffs_total.clone()))
            .expect("failed to register consolidation_reconciliation_diffs_total");
        registry
            .register(Box::new(consolidation_failures_total.clone()))
            .expect("failed to register consolidation_failures_total");
        registry
            .register(Box::new(belief_active.clone()))
            .expect("failed to register memory_belief_active");
        registry
            .register(Box::new(recall_requests_total.clone()))
            .expect("failed to register recall_requests_total");
        registry
            .register(Box::new(recall_items_total.clone()))
            .expect("failed to register recall_items_total");
        registry
            .register(Box::new(ws_send_duration_seconds.clone()))
            .expect("failed to register ws_send_duration_seconds");
        registry
            .register(Box::new(ws_broadcast_queue_depth.clone()))
            .expect("failed to register ws_broadcast_queue_depth");

        Self {
            stm_sessions_active,
            ltm_entries_total,
            weight_adjustments_total,
            transfer_operations_total,
            search_duration_seconds,
            request_duration_seconds,
            requests_total,
            auth_failures_total,
            ltm_summary_degraded_total,
            tenant_quota_usage_ratio,
            outbox_pending_total,
            outbox_dead_letter_total,
            outbox_processing_duration_seconds,
            outbox_qdrant_upsert_success_total,
            outbox_qdrant_upsert_failure_total,
            reconciliation_missing_entries,
            reconciliation_orphan_points,
            reconciliation_tenant_mismatch_entries,
            reconciliation_content_hash_mismatch_entries,
            reconciliation_scanned_entries,
            reconciliation_last_scan_timestamp_seconds,
            audit_spilled_total,
            audit_replayed_total,
            audit_dropped_total,
            audit_truncated_skipped_total,
            ws_connections_active,
            ws_lagged_drops_total,
            consolidation_run_duration_seconds,
            consolidation_runs_total,
            consolidation_conflicts_total,
            consolidation_stale_marked_total,
            consolidation_promises_expired_total,
            consolidation_reconciliation_diffs_total,
            consolidation_failures_total,
            belief_active,
            recall_requests_total,
            recall_items_total,
            ws_send_duration_seconds,
            ws_broadcast_queue_depth,
            registry,
        }
    }

    /// Record a request completion
    pub fn record_request(&self, endpoint: &str, status: u16, duration: Duration) {
        self.requests_total
            .with_label_values(&[endpoint, &status.to_string()])
            .inc();
        self.request_duration_seconds
            .observe(duration.as_secs_f64());
    }

    /// Record search operation duration
    pub fn record_search_duration(&self, duration_secs: f64) {
        self.search_duration_seconds.observe(duration_secs);
    }

    /// Increment the authentication-failure counter for a bounded `reason`.
    ///
    /// `reason` MUST be a caller-supplied constant (e.g. `missing_token`,
    /// `invalid_token`, `expired`, `query_param_token`) — never user id, tenant
    /// id, or token content, which would explode label cardinality and leak
    /// secrets into scraped metrics.
    pub fn inc_auth_failure(&self, reason: &str) {
        self.auth_failures_total.with_label_values(&[reason]).inc();
    }

    /// Increment the LTM summary-degraded counter for a bounded `reason`.
    ///
    /// `reason` MUST be a caller-supplied constant (`unavailable` /
    /// `upstream_error`) — never error text, ids, or tenant, which would
    /// explode label cardinality and leak detail into scraped metrics.
    pub fn inc_ltm_summary_degraded(&self, reason: &str) {
        self.ltm_summary_degraded_total
            .with_label_values(&[reason])
            .inc();
    }

    /// Set active STM sessions count
    pub fn set_stm_sessions_active(&self, count: f64) {
        self.stm_sessions_active.set(count);
    }

    /// Set total LTM entries count
    pub fn set_ltm_entries_total(&self, count: f64) {
        self.ltm_entries_total.set(count);
    }

    /// Increment weight adjustment counter
    pub fn increment_weight_adjustments(&self) {
        self.weight_adjustments_total.inc();
    }

    /// Increment transfer operations counter
    pub fn increment_transfer_operations(&self) {
        self.transfer_operations_total.inc();
    }

    /// Set tenant quota usage ratio
    pub fn set_tenant_quota_usage(&self, tenant: &str, ratio: f64) {
        self.tenant_quota_usage_ratio
            .with_label_values(&[tenant])
            .set(ratio);
    }

    /// Set outbox pending events count
    pub fn set_outbox_pending(&self, count: f64) {
        self.outbox_pending_total.set(count);
    }

    /// Increment outbox dead letter counter
    pub fn inc_outbox_dead_letter(&self) {
        self.outbox_dead_letter_total.inc();
    }

    /// Set the active WebSocket connection count (#86).
    pub fn set_ws_connections_active(&self, count: f64) {
        self.ws_connections_active.set(count);
    }

    /// Increment when a WS client lagged behind the broadcast and events were
    /// dropped (#86) — a slow-consumer / backpressure signal.
    /// #129 belief consolidation metrics.
    pub fn record_consolidation_run_duration(&self, duration_secs: f64) {
        self.consolidation_run_duration_seconds
            .observe(duration_secs);
    }

    pub fn inc_consolidation_runs(&self) {
        self.consolidation_runs_total.inc();
    }

    pub fn inc_consolidation_conflicts(&self) {
        self.consolidation_conflicts_total.inc();
    }

    pub fn inc_consolidation_stale(&self) {
        self.consolidation_stale_marked_total.inc();
    }

    pub fn inc_consolidation_promises_expired(&self) {
        self.consolidation_promises_expired_total.inc();
    }

    pub fn inc_consolidation_reconciliation_diffs(&self) {
        self.consolidation_reconciliation_diffs_total.inc();
    }

    /// #130 governance/recall observability.
    pub fn set_belief_active(&self, count: f64) {
        self.belief_active.set(count);
    }

    pub fn inc_recall_request(&self, items: usize) {
        self.recall_requests_total.inc();
        self.recall_items_total.inc_by(items as f64);
    }

    pub fn inc_consolidation_failures(&self) {
        self.consolidation_failures_total.inc();
    }

    pub fn inc_ws_lagged_drops(&self) {
        self.ws_lagged_drops_total.inc();
    }

    /// Record a WebSocket frame send duration (#86) — a climbing p99 means the
    /// client is slow to drain its socket (backpressure / head-of-line blocking).
    pub fn record_ws_send_duration(&self, duration_secs: f64) {
        self.ws_send_duration_seconds.observe(duration_secs);
    }

    /// Set the WebSocket broadcast channel queue depth (#86) — a climbing value
    /// means consumers are slow to drain the broadcast channel.
    pub fn set_ws_broadcast_queue_depth(&self, depth: f64) {
        self.ws_broadcast_queue_depth.set(depth);
    }

    /// Record outbox processing duration
    pub fn record_outbox_processing_duration(&self, duration_secs: f64) {
        self.outbox_processing_duration_seconds
            .observe(duration_secs);
    }

    /// Increment outbox Qdrant upsert success counter
    pub fn inc_outbox_qdrant_upsert_success(&self) {
        self.outbox_qdrant_upsert_success_total.inc();
    }

    /// Increment outbox Qdrant upsert failure counter
    pub fn inc_outbox_qdrant_upsert_failure(&self) {
        self.outbox_qdrant_upsert_failure_total.inc();
    }

    /// Set reconciliation missing count
    pub fn set_reconciliation_missing(&self, count: f64) {
        self.reconciliation_missing_entries.set(count);
    }

    /// Set reconciliation orphan count
    pub fn set_reconciliation_orphan(&self, count: f64) {
        self.reconciliation_orphan_points.set(count);
    }

    /// Set reconciliation tenant mismatch count
    pub fn set_reconciliation_tenant_mismatch(&self, count: f64) {
        self.reconciliation_tenant_mismatch_entries.set(count);
    }

    /// Set reconciliation content hash mismatch count
    pub fn set_reconciliation_content_hash_mismatch(&self, count: f64) {
        self.reconciliation_content_hash_mismatch_entries.set(count);
    }

    /// Set reconciliation scanned total
    pub fn set_reconciliation_scanned(&self, count: f64) {
        self.reconciliation_scanned_entries.set(count);
    }

    /// Borrow the registry so callers can `gather()` current metric families.
    ///
    /// Used by tests to assert a metric was actually written rather than merely
    /// registered — the distinction that made most metrics in this file read a
    /// frozen 0 in production.
    pub fn registry(&self) -> &prometheus::Registry {
        &self.registry
    }

    /// Stamp the reconciliation staleness gauge with the current wall-clock time.
    ///
    /// Call this when a scan cycle completes **and** once when the scanner
    /// starts. Seeding at startup matters: an unset gauge reads `0`, so
    /// `time() - 0` is ~56 years and the staleness alert would fire on every
    /// boot. Seeding also keeps the "scanner started but no scan ever
    /// succeeded" case detectable, which a `> 0` guard in the alert expression
    /// would have hidden.
    pub fn stamp_reconciliation_scan_time(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        self.reconciliation_last_scan_timestamp_seconds.set(now);
    }

    /// Increment the audit-spilled counter by one (event written to the disk buffer).
    pub fn inc_audit_spilled(&self) {
        self.audit_spilled_total.inc();
    }

    /// Increment the audit-replayed counter by `n` (events drained from spill to DB).
    pub fn inc_audit_replayed_by(&self, n: f64) {
        self.audit_replayed_total.inc_by(n);
    }

    /// Increment the audit last-resort-drop counter by one (spill itself failed).
    pub fn inc_audit_dropped(&self) {
        self.audit_dropped_total.inc();
    }

    /// Increment the audit truncated-skip counter by `n` (unparseable spill lines).
    pub fn inc_audit_truncated_skipped_by(&self, n: f64) {
        self.audit_truncated_skipped_total.inc_by(n);
    }

    /// Convert internal metrics to Prometheus format
    pub fn export_internal_metrics(&self) -> String {
        let mut output = String::new();

        // Export internal metrics from the metrics service
        let internal_metrics = metrics::get_metrics().get_metrics();

        for event in internal_metrics {
            let metric_name = format!("memory_internal_{}_total", event.op_type.as_str());
            let help_text = format!("Internal metric for {} operations", event.op_type.as_str());

            output.push_str(&format!("# HELP {} {}\n", metric_name, help_text));
            output.push_str(&format!("# TYPE {} counter\n", metric_name));

            let total = event.failure.total();
            output.push_str(&format!(
                "{}{{bucket=\"{}\"}} {}\n",
                metric_name, event.bucket, total
            ));
        }

        output
    }

    /// Generate Prometheus text format output
    pub fn generate_prometheus_output(&self) -> String {
        let mut output = String::new();

        // Gather metrics from the registry
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        if let Ok(encoded) = encoder.encode_to_string(&metric_families) {
            output.push_str(&encoded);
        }

        // Export internal metrics from metrics service
        output.push_str(&self.export_internal_metrics());

        output
    }
}

/// Global Prometheus exporter instance
static PROMETHEUS_EXPORTER: std::sync::OnceLock<PrometheusExporter> = std::sync::OnceLock::new();

/// Get the global Prometheus exporter instance
pub fn get_exporter() -> &'static PrometheusExporter {
    PROMETHEUS_EXPORTER.get_or_init(PrometheusExporter::new)
}

/// Initialize the Prometheus exporter with custom configuration
pub fn init_exporter() -> &'static PrometheusExporter {
    get_exporter()
}

/// Metrics endpoint handler returning Prometheus text format
pub async fn metrics_handler() -> Response {
    let exporter = get_exporter();
    let output = exporter.generate_prometheus_output();

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "text/plain; version=0.0.4".parse().unwrap(),
    );

    (StatusCode::OK, headers, output).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prometheus_exporter_creation() {
        let exporter = PrometheusExporter::new();
        assert_eq!(exporter.stm_sessions_active.get(), 0.0);
        assert_eq!(exporter.ltm_entries_total.get(), 0.0);
    }

    #[test]
    fn test_set_stm_sessions() {
        let exporter = PrometheusExporter::new();
        exporter.set_stm_sessions_active(5.0);
        assert_eq!(exporter.stm_sessions_active.get(), 5.0);
    }

    #[test]
    fn test_set_ltm_entries() {
        let exporter = PrometheusExporter::new();
        exporter.set_ltm_entries_total(100.0);
        assert_eq!(exporter.ltm_entries_total.get(), 100.0);
    }

    #[test]
    fn test_increment_counters() {
        let exporter = PrometheusExporter::new();
        exporter.increment_weight_adjustments();
        exporter.increment_weight_adjustments();
        exporter.increment_transfer_operations();
        // No panic means success
    }

    #[test]
    fn test_tenant_quota_usage() {
        let exporter = PrometheusExporter::new();
        exporter.set_tenant_quota_usage("tenant1", 0.75);
        exporter.set_tenant_quota_usage("tenant2", 0.50);
        // No panic means success
    }

    #[test]
    fn test_generate_prometheus_output() {
        let exporter = PrometheusExporter::new();
        exporter.set_stm_sessions_active(10.0);
        exporter.set_ltm_entries_total(200.0);

        let output = exporter.generate_prometheus_output();

        assert!(output.contains("memory_stm_sessions_active"));
        assert!(output.contains("memory_ltm_entries_total"));
    }

    #[test]
    fn test_ws_metrics_registered() {
        // #86: confirm the WS gauge + counter + histogram + queue depth are
        // registered. Live setter callers exist in handle_ws_connection
        // (create/remove set the gauge, Lagged incs the counter, queue depth
        // set on every recv) — see aetheris-metrics-mostly-dead.
        let exporter = PrometheusExporter::new();
        exporter.set_ws_connections_active(3.0);
        exporter.inc_ws_lagged_drops();
        exporter.record_ws_send_duration(0.005);
        exporter.set_ws_broadcast_queue_depth(7.0);

        let output = exporter.generate_prometheus_output();
        assert!(
            output.contains("ws_connections_active"),
            "missing gauge: {output}"
        );
        assert!(
            output.contains("ws_lagged_drops_total"),
            "missing counter: {output}"
        );
        assert!(
            output.contains("ws_send_duration_seconds"),
            "missing histogram: {output}"
        );
        assert!(
            output.contains("ws_broadcast_queue_depth"),
            "missing gauge: {output}"
        );
    }

    #[test]
    fn test_record_search_duration() {
        let exporter = PrometheusExporter::new();
        exporter.record_search_duration(0.05);
        exporter.record_search_duration(0.12);
        // No panic means success
    }

    #[test]
    fn test_record_request() {
        let exporter = PrometheusExporter::new();
        exporter.record_request("/api/memory", 200, Duration::from_millis(50));
        exporter.record_request("/api/memory", 500, Duration::from_millis(200));
        // No panic means success
    }

    #[test]
    fn test_global_exporter_singleton() {
        let exporter1 = get_exporter();
        let exporter2 = get_exporter();
        assert!(std::ptr::eq(exporter1, exporter2));
    }

    // --- Registry read-back helpers -------------------------------------- //
    //
    // These tests use a FRESH `PrometheusExporter::new()` rather than the global
    // `get_exporter()` singleton, so every metric starts at a deterministic 0 and
    // absolute assertions are stable. On the shared singleton, other tests would
    // perturb the counters and force fragile delta arithmetic.

    /// Value of a labelled counter series, or 0.0 if the series is absent.
    /// A CounterVec emits NO series for a label value until it is first used, so
    /// "absent" and "0" are indistinguishable here — the meaningful assertions
    /// below are the post-increment values.
    fn counter_with_reason(reg: &prometheus::Registry, name: &str, reason: &str) -> f64 {
        for mf in reg.gather() {
            if mf.get_name() != name {
                continue;
            }
            for m in mf.get_metric() {
                if m.get_label()
                    .iter()
                    .any(|l| l.get_name() == "reason" && l.get_value() == reason)
                {
                    return m.get_counter().get_value();
                }
            }
        }
        0.0
    }

    /// Value of an unlabelled counter, or 0.0 if absent.
    fn counter_scalar(reg: &prometheus::Registry, name: &str) -> f64 {
        for mf in reg.gather() {
            if mf.get_name() == name {
                if let Some(m) = mf.get_metric().first() {
                    return m.get_counter().get_value();
                }
            }
        }
        0.0
    }

    /// Value of an unlabelled gauge, or 0.0 if absent.
    fn gauge_scalar(reg: &prometheus::Registry, name: &str) -> f64 {
        for mf in reg.gather() {
            if mf.get_name() == name {
                if let Some(m) = mf.get_metric().first() {
                    return m.get_gauge().get_value();
                }
            }
        }
        0.0
    }

    #[test]
    fn inc_auth_failure_emits_a_series_per_reason() {
        let exporter = PrometheusExporter::new();
        exporter.inc_auth_failure("missing_token");
        exporter.inc_auth_failure("missing_token");
        exporter.inc_auth_failure("expired");

        assert_eq!(
            counter_with_reason(exporter.registry(), "auth_failures_total", "missing_token"),
            2.0,
            "two missing_token failures must be counted under that reason"
        );
        assert_eq!(
            counter_with_reason(exporter.registry(), "auth_failures_total", "expired"),
            1.0
        );
        // A reason never incremented produces no series (reads as 0.0 here).
        assert_eq!(
            counter_with_reason(exporter.registry(), "auth_failures_total", "invalid_token"),
            0.0
        );
    }

    #[test]
    fn inc_ltm_summary_degraded_emits_a_series_per_reason() {
        let exporter = PrometheusExporter::new();
        exporter.inc_ltm_summary_degraded("unavailable");
        exporter.inc_ltm_summary_degraded("unavailable");
        exporter.inc_ltm_summary_degraded("upstream_error");

        assert_eq!(
            counter_with_reason(
                exporter.registry(),
                "ltm_summary_degraded_total",
                "unavailable"
            ),
            2.0,
            "two unavailable degradations must be counted under that reason"
        );
        assert_eq!(
            counter_with_reason(
                exporter.registry(),
                "ltm_summary_degraded_total",
                "upstream_error"
            ),
            1.0
        );
        // A reason never incremented produces no series (reads as 0.0 here).
        assert_eq!(
            counter_with_reason(
                exporter.registry(),
                "ltm_summary_degraded_total",
                "malformed"
            ),
            0.0
        );
    }

    #[test]
    fn outbox_counters_move_in_the_registry() {
        let exporter = PrometheusExporter::new();
        exporter.inc_outbox_dead_letter();
        exporter.inc_outbox_qdrant_upsert_success();
        exporter.inc_outbox_qdrant_upsert_success();
        exporter.inc_outbox_qdrant_upsert_failure();

        assert_eq!(
            counter_scalar(exporter.registry(), "outbox_dead_letter_total"),
            1.0
        );
        assert_eq!(
            counter_scalar(exporter.registry(), "outbox_qdrant_upsert_success_total"),
            2.0
        );
        assert_eq!(
            counter_scalar(exporter.registry(), "outbox_qdrant_upsert_failure_total"),
            1.0
        );
    }

    #[test]
    fn set_outbox_pending_reflected_in_registry() {
        let exporter = PrometheusExporter::new();
        exporter.set_outbox_pending(42.0);
        assert_eq!(
            gauge_scalar(exporter.registry(), "outbox_pending_total"),
            42.0
        );
        // A gauge tracks the latest value, not a running sum.
        exporter.set_outbox_pending(7.0);
        assert_eq!(
            gauge_scalar(exporter.registry(), "outbox_pending_total"),
            7.0
        );
    }

    #[test]
    fn audit_durability_counters_move_in_the_registry() {
        let exporter = PrometheusExporter::new();
        exporter.inc_audit_spilled();
        exporter.inc_audit_spilled();
        exporter.inc_audit_replayed_by(3.0);
        exporter.inc_audit_dropped();
        exporter.inc_audit_truncated_skipped_by(2.0);

        assert_eq!(
            counter_scalar(exporter.registry(), "audit_spilled_total"),
            2.0,
            "two spilled events must be counted"
        );
        assert_eq!(
            counter_scalar(exporter.registry(), "audit_replayed_total"),
            3.0
        );
        assert_eq!(
            counter_scalar(exporter.registry(), "audit_dropped_total"),
            1.0,
            "last-resort drops are the compliance-alert signal"
        );
        assert_eq!(
            counter_scalar(exporter.registry(), "audit_truncated_skipped_total"),
            2.0
        );
    }

    // --- B-5b: the four previously zero-call metrics ---------------------- //

    /// Sample count of an unlabelled histogram, or 0 if the series is absent.
    fn histogram_count(reg: &prometheus::Registry, name: &str) -> u64 {
        for mf in reg.gather() {
            if mf.get_name() == name {
                if let Some(m) = mf.get_metric().first() {
                    return m.get_histogram().get_sample_count();
                }
            }
        }
        0
    }

    /// Value of a labelled counter series (e.g. `memory_requests_total`) matching
    /// ALL of `labels`, or 0.0 if no such series exists.
    fn labelled_counter(reg: &prometheus::Registry, name: &str, labels: &[(&str, &str)]) -> f64 {
        for mf in reg.gather() {
            if mf.get_name() != name {
                continue;
            }
            for m in mf.get_metric() {
                let series = m.get_label();
                let all_match = labels.iter().all(|(k, v)| {
                    series
                        .iter()
                        .any(|l| l.get_name() == *k && l.get_value() == *v)
                });
                if all_match {
                    return m.get_counter().get_value();
                }
            }
        }
        0.0
    }

    #[test]
    fn set_inventory_gauges_reflected_in_registry() {
        let exporter = PrometheusExporter::new();
        exporter.set_ltm_entries_total(1234.0);
        exporter.set_stm_sessions_active(56.0);

        assert_eq!(
            gauge_scalar(exporter.registry(), "memory_ltm_entries_total"),
            1234.0,
            "LTM inventory gauge must carry the value it was set to"
        );
        assert_eq!(
            gauge_scalar(exporter.registry(), "memory_stm_sessions_active"),
            56.0
        );
        // A gauge tracks the latest observation, not a running sum.
        exporter.set_ltm_entries_total(1000.0);
        assert_eq!(
            gauge_scalar(exporter.registry(), "memory_ltm_entries_total"),
            1000.0
        );
    }

    #[test]
    fn record_search_duration_lands_in_the_histogram() {
        let exporter = PrometheusExporter::new();
        assert_eq!(
            histogram_count(exporter.registry(), "memory_search_duration_seconds"),
            0,
            "histogram starts empty"
        );
        exporter.record_search_duration(0.05);
        exporter.record_search_duration(0.20);
        assert_eq!(
            histogram_count(exporter.registry(), "memory_search_duration_seconds"),
            2,
            "each search observation must be counted in the histogram"
        );
    }

    #[test]
    fn record_request_moves_counter_and_latency_histogram() {
        let exporter = PrometheusExporter::new();
        exporter.record_request("/api/x", 200, Duration::from_millis(5));
        exporter.record_request("/api/x", 200, Duration::from_millis(7));
        exporter.record_request("/api/x", 500, Duration::from_millis(9));

        assert_eq!(
            labelled_counter(
                exporter.registry(),
                "memory_requests_total",
                &[("endpoint", "/api/x"), ("status", "200")]
            ),
            2.0,
            "two 200s under the same endpoint must accumulate on one series"
        );
        assert_eq!(
            labelled_counter(
                exporter.registry(),
                "memory_requests_total",
                &[("endpoint", "/api/x"), ("status", "500")]
            ),
            1.0,
            "a different status is a distinct series"
        );
        // record_request also observes the shared (unlabelled) latency histogram,
        // once per call regardless of status.
        assert_eq!(
            histogram_count(exporter.registry(), "memory_request_duration_seconds"),
            3
        );
    }
}
