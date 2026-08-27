//! Memory Consolidation Service
//!
//! Implements sleep-like memory consolidation for transforming short-term memories
//! into structured long-term knowledge.

use crate::kernel::error::MemoryResult;
use crate::kernel::types::*;

/// Consolidation trigger type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerType {
    /// Time-based trigger (e.g., daily during idle period)
    Scheduled,
    /// Threshold-based trigger (e.g., STM backlog exceeds limit)
    Threshold,
    /// Manual trigger
    Manual,
}

/// Consolidation result
#[derive(Debug, Clone)]
pub struct ConsolidationResult {
    /// Number of memories consolidated
    pub consolidated_count: usize,
    /// Number of memories compressed
    pub compressed_count: usize,
    /// Number of conflicts resolved
    pub conflicts_resolved: usize,
    /// Summary of operations
    pub summary: Vec<String>,
}

impl Default for ConsolidationResult {
    fn default() -> Self {
        Self {
            consolidated_count: 0,
            compressed_count: 0,
            conflicts_resolved: 0,
            summary: vec![],
        }
    }
}

/// Consolidation configuration
#[derive(Debug, Clone)]
pub struct ConsolidationConfig {
    /// Maximum STM entries before threshold trigger
    pub stm_threshold: usize,
    /// Minimum time between scheduled consolidations (seconds)
    pub schedule_interval_seconds: i64,
    /// Maximum entries to process per cycle
    pub max_entries_per_cycle: usize,
    /// Enable memory compression
    pub enable_compression: bool,
    /// Enable conflict detection
    pub enable_conflict_resolution: bool,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            stm_threshold: 1000,
            schedule_interval_seconds: 86400, // Daily
            max_entries_per_cycle: 100,
            enable_compression: true,
            enable_conflict_resolution: true,
        }
    }
}

/// Memory consolidation service
pub struct ConsolidationService {
    config: ConsolidationConfig,
    last_consolidation: i64,
}

impl Default for ConsolidationService {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsolidationService {
    pub fn new() -> Self {
        Self {
            config: ConsolidationConfig::default(),
            last_consolidation: 0,
        }
    }

    pub fn with_config(config: ConsolidationConfig) -> Self {
        Self {
            config,
            last_consolidation: 0,
        }
    }

    /// Check if consolidation should be triggered
    pub fn should_consolidate(&self, stm_count: usize) -> bool {
        let now = chrono::Utc::now().timestamp();

        // Check threshold trigger
        if stm_count >= self.config.stm_threshold {
            return true;
        }

        // Check scheduled trigger
        if now - self.last_consolidation >= self.config.schedule_interval_seconds {
            return true;
        }

        false
    }

    /// Run memory consolidation cycle
    pub async fn consolidate(
        &mut self,
        stm_entries: &[MemoryEntry],
    ) -> MemoryResult<ConsolidationResult> {
        use crate::services::memory_storage::MemoryStorageService;

        let now = chrono::Utc::now().timestamp();
        let mut result = ConsolidationResult::default();

        // Limit entries to process
        let entries_to_process = stm_entries
            .iter()
            .take(self.config.max_entries_per_cycle)
            .collect::<Vec<_>>();

        for entry in entries_to_process {
            // Check if entry should be consolidated based on age and importance
            let age_hours = (now - entry.created_at) as f64 / 3600.0;
            let importance = entry.metadata.importance;

            // Consolidate important old memories to LTM
            if age_hours > 24.0 && importance > 0.5 {
                // Get session_id from metadata
                if let Some(session_id) = &entry.metadata.session_id {
                    // Extract content string from MemoryContent enum
                    let content = match &entry.content {
                        MemoryContent::Text(s) => s.clone(),
                        MemoryContent::Json(v) => v.to_string(),
                        MemoryContent::Binary(_) => String::from("[binary data]"),
                        MemoryContent::Graph(_) => String::from("[graph data]"),
                    };
                    // Truncate content for LTM storage
                    let truncated = content.chars().take(100).collect::<String>();

                    // Transfer to LTM via storage service
                    match MemoryStorageService::store_ltm(
                        session_id,
                        "consolidated",
                        &format!("Consolidated from STM: {}", truncated),
                        None,
                    )
                    .await
                    {
                        Ok(_) => {
                            result.consolidated_count += 1;
                        }
                        Err(e) => {
                            result.summary.push(format!(
                                "Transfer failed for {}: {}",
                                entry.id.as_str(),
                                e
                            ));
                        }
                    }
                }
            } else if self.config.enable_compression && age_hours > 48.0 {
                // Compress old low-importance memories
                result.compressed_count += 1;
                result
                    .summary
                    .push(format!("Compressed entry: {}", entry.id.as_str()));
            }
        }

        // Conflict resolution
        if self.config.enable_conflict_resolution {
            let conflicts = self.detect_conflicts(stm_entries).await;
            result.conflicts_resolved = conflicts.len();
            for conflict in conflicts {
                result
                    .summary
                    .push(format!("Resolved conflict: {}", conflict));
            }
        }

        // Update last consolidation time
        self.last_consolidation = now;

        if result.consolidated_count == 0 && result.compressed_count == 0 {
            result
                .summary
                .push("No memories met consolidation criteria".to_string());
        }

        Ok(result)
    }

    /// Detect conflicts between memories
    async fn detect_conflicts(&self, _entries: &[MemoryEntry]) -> Vec<String> {
        // Placeholder for conflict detection logic
        // In production, this would compare semantic embeddings or content
        vec![]
    }

    /// Get consolidation statistics
    pub fn get_stats(&self) -> ConsolidationStats {
        ConsolidationStats {
            last_consolidation: self.last_consolidation,
            config: self.config.clone(),
        }
    }
}

/// Consolidation statistics
#[derive(Debug, Clone)]
pub struct ConsolidationStats {
    pub last_consolidation: i64,
    pub config: ConsolidationConfig,
}

/// Background task for periodic consolidation
pub struct ConsolidationScheduler {
    service: ConsolidationService,
}

impl Default for ConsolidationScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsolidationScheduler {
    pub fn new() -> Self {
        Self {
            service: ConsolidationService::new(),
        }
    }

    pub fn with_config(config: ConsolidationConfig) -> Self {
        Self {
            service: ConsolidationService::with_config(config),
        }
    }

    /// Run consolidation if triggered
    pub async fn run_cycle(
        &mut self,
        stm_entries: &[MemoryEntry],
    ) -> MemoryResult<ConsolidationResult> {
        if self.service.should_consolidate(stm_entries.len()) {
            self.service.consolidate(stm_entries).await
        } else {
            Ok(ConsolidationResult {
                summary: vec!["Consolidation not triggered".to_string()],
                ..Default::default()
            })
        }
    }

    /// Force immediate consolidation
    pub async fn force_consolidate(
        &mut self,
        stm_entries: &[MemoryEntry],
    ) -> MemoryResult<ConsolidationResult> {
        self.service.consolidate(stm_entries).await
    }

    /// Get service statistics
    pub fn get_stats(&self) -> ConsolidationStats {
        self.service.get_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consolidation_config_defaults() {
        let config = ConsolidationConfig::default();
        assert_eq!(config.stm_threshold, 1000);
        assert_eq!(config.schedule_interval_seconds, 86400);
    }

    #[test]
    fn test_should_consolidate_threshold() {
        let service = ConsolidationService::with_config(ConsolidationConfig {
            stm_threshold: 100,
            schedule_interval_seconds: i64::MAX, // Disable scheduled trigger
            ..Default::default()
        });

        assert!(service.should_consolidate(100));
        assert!(service.should_consolidate(150));
        assert!(!service.should_consolidate(50));
    }

    #[test]
    fn test_consolidation_result_default() {
        let result = ConsolidationResult::default();
        assert_eq!(result.consolidated_count, 0);
        assert_eq!(result.compressed_count, 0);
        assert_eq!(result.conflicts_resolved, 0);
    }

    #[test]
    fn test_consolidation_scheduler() {
        let scheduler = ConsolidationScheduler::new();
        let stats = scheduler.get_stats();
        assert_eq!(stats.config.stm_threshold, 1000);
    }
}

// ============================================================================
// #129: Belief consolidation — offline expiry detection, decay, SoR reconcile
//
// Everything above this line is the legacy in-memory prototype; everything
// below operates on the governed belief store (#127) and is the runnable,
// observable, idempotent consolidation loop #129 prescribes.
// ============================================================================

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;

use crate::db::belief::BeliefRepository;
use crate::db::memory_event::MemoryEventRepository;
use crate::error::AppError;
use crate::models::belief::BeliefSource;
use crate::models::belief_record::{BeliefClaim, GateOutcome};
use crate::models::memory_event::{AppendMemoryEventRequest, MemoryEventType};
use crate::services::belief::BeliefGateService;
use crate::services::prometheus_exporter::get_exporter;
use crate::tenant::TenantId;

/// One authoritative fact change pushed by a system of record.
#[derive(Debug, Clone)]
pub struct SorUpdate {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// Originating system tag (e.g. "mock-crm", "hr-sync").
    pub system: String,
    /// Belief owner when the SoR knows it; resolved from the subject when not.
    pub principal_id: Option<String>,
}

impl SorUpdate {
    /// Attach the belief-owning principal (when the SoR knows it).
    pub fn principal(mut self, id: impl Into<String>) -> Self {
        self.principal_id = Some(id.into());
        self
    }

    pub fn new(
        subject: impl Into<String>,
        predicate: impl Into<String>,
        object: impl Into<String>,
        system: impl Into<String>,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            system: system.into(),
            principal_id: None,
        }
    }
}

/// Authority-system adapter (#129: one system, interface first). Fetch the
/// tenant's authoritative updates since the last cycle; reconciliation turns
/// each into supersede/reconfirm — never an in-place history edit.
pub trait SorAdapter: Send + Sync {
    fn fetch_updates(
        &self,
        tenant_id: &TenantId,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<SorUpdate>, AppError>> + Send + '_>,
    >;
}

/// Static/configured adapter: fixed updates per tenant. Serves as the minimal
/// CRM/HR mock #129 asks for and as the test double.
#[derive(Default)]
pub struct StaticSorAdapter {
    updates: std::sync::RwLock<HashMap<String, Vec<SorUpdate>>>,
}

impl StaticSorAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure the updates the adapter will serve for one tenant.
    pub fn set_updates(&self, tenant_id: &TenantId, updates: Vec<SorUpdate>) {
        self.updates
            .write()
            .expect("sor adapter lock")
            .insert(tenant_id.as_str().to_string(), updates);
    }
}

impl SorAdapter for StaticSorAdapter {
    fn fetch_updates(
        &self,
        tenant_id: &TenantId,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<SorUpdate>, AppError>> + Send + '_>,
    > {
        let updates = self
            .updates
            .read()
            .expect("sor adapter lock")
            .get(tenant_id.as_str())
            .cloned()
            .unwrap_or_default();
        Box::pin(std::future::ready(Ok(updates)))
    }
}

/// Tuning knobs for the consolidation loop.
#[derive(Debug, Clone)]
pub struct BeliefConsolidationConfig {
    /// Per-scan row cap PER TENANT PER CYCLE — the fairness guarantee (#129:
    /// one tenant's backlog must not starve the others).
    pub per_tenant_batch: i64,
    /// Web observations older than this decay to the first trust plateau.
    pub web_decay_hours: i32,
    /// Trust plateaus (fixed steps keep re-runs idempotent).
    pub web_trust_plateau_1: f32,
    pub web_trust_plateau_2: f32,
    /// Stale edges with at least this many feedback signals go to the human
    /// confirmation queue instead of plain stale (高召回 stale → 待确认).
    pub confirm_queue_feedback_min: i64,
}

impl Default for BeliefConsolidationConfig {
    fn default() -> Self {
        Self {
            per_tenant_batch: 100,
            web_decay_hours: 48,
            web_trust_plateau_1: 0.15,
            web_trust_plateau_2: 0.05,
            confirm_queue_feedback_min: 2,
        }
    }
}

/// What one tenant's consolidation cycle did.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ConsolidationReport {
    pub multi_active_repaired: usize,
    pub edges_closed_in_repair: usize,
    pub conflicts_parked: usize,
    pub stale_marked: usize,
    pub confirm_queued: usize,
    pub promises_expired: usize,
    pub web_decayed: usize,
    pub sor_diffs: usize,
    pub sor_opened: usize,
    pub sor_closed: usize,
    pub sor_refreshed: usize,
    pub errors: Vec<String>,
}

pub struct BeliefConsolidationService {
    repo: BeliefRepository,
    events: MemoryEventRepository,
    gate: BeliefGateService,
    adapter: Arc<dyn SorAdapter>,
    config: BeliefConsolidationConfig,
}

impl BeliefConsolidationService {
    pub fn new(
        pool: PgPool,
        adapter: Arc<dyn SorAdapter>,
        config: BeliefConsolidationConfig,
    ) -> Self {
        Self {
            repo: BeliefRepository::new(pool.clone()),
            events: MemoryEventRepository::new(pool.clone()),
            gate: BeliefGateService::new(pool),
            adapter,
            config,
        }
    }

    /// Run every scan for one tenant. Each scan is individually idempotent and
    /// individually failure-isolated: an error in one lands in the report
    /// (and the failure metric) without aborting the others.
    pub async fn run_for_tenant(&self, tenant_id: &TenantId) -> ConsolidationReport {
        let start = std::time::Instant::now();
        get_exporter().inc_consolidation_runs();

        let mut report = ConsolidationReport::default();
        self.scan_multi_active(tenant_id, &mut report).await;
        self.scan_stale(tenant_id, &mut report).await;
        self.scan_expired_promises(tenant_id, &mut report).await;
        self.scan_web_decay(tenant_id, &mut report).await;
        self.reconcile_sor(tenant_id, &mut report).await;

        get_exporter().record_consolidation_run_duration(start.elapsed().as_secs_f64());
        report
    }

    /// One fairness round: every listed tenant gets the same per-scan budget;
    /// a tenant with a huge backlog consumes its batch and yields.
    pub async fn process_round(
        &self,
        tenants: &[TenantId],
    ) -> HashMap<String, ConsolidationReport> {
        let mut out = HashMap::new();
        for tenant in tenants {
            let report = self.run_for_tenant(tenant).await;
            out.insert(tenant.as_str().to_string(), report);
        }
        out
    }

    async fn scan_multi_active(&self, tenant_id: &TenantId, report: &mut ConsolidationReport) {
        let groups = match self
            .repo
            .multi_active_groups(tenant_id, self.config.per_tenant_batch)
            .await
        {
            Ok(g) => g,
            Err(e) => return self.fail("multi_active", e, report),
        };
        for (subject, predicate, _n) in groups {
            match self
                .repo
                .repair_multi_active_group(tenant_id, &subject, &predicate)
                .await
            {
                Ok((_winner, closed, parked)) => {
                    report.multi_active_repaired += 1;
                    report.edges_closed_in_repair += closed;
                    if parked {
                        report.conflicts_parked += 1;
                    }
                    get_exporter().inc_consolidation_conflicts();
                }
                Err(e) => self.fail("multi_active_repair", e, report),
            }
        }
    }

    async fn scan_stale(&self, tenant_id: &TenantId, report: &mut ConsolidationReport) {
        let candidates = match self
            .repo
            .stale_candidates(tenant_id, self.config.per_tenant_batch)
            .await
        {
            Ok(c) => c,
            Err(e) => return self.fail("stale", e, report),
        };
        if candidates.is_empty() {
            return;
        }
        // Engagement signal decides the destination: high-recall stale edges
        // queue for a human; the rest simply age out of the active set.
        let ids: Vec<String> = candidates.iter().map(|b| b.id.clone()).collect();
        let engagement = match self.repo.feedback_counts(tenant_id, &ids).await {
            Ok(m) => m,
            Err(e) => return self.fail("stale_feedback", e, report),
        };
        for edge in candidates {
            let hot = engagement.get(&edge.id).copied().unwrap_or(0)
                >= self.config.confirm_queue_feedback_min;
            match self.repo.mark_stale(tenant_id, &edge.id, hot).await {
                Ok(true) => {
                    if hot {
                        report.confirm_queued += 1;
                    } else {
                        report.stale_marked += 1;
                    }
                    get_exporter().inc_consolidation_stale();
                }
                Ok(false) => {} // already non-active: idempotent replay
                Err(e) => self.fail("mark_stale", e, report),
            }
        }
    }

    async fn scan_expired_promises(&self, tenant_id: &TenantId, report: &mut ConsolidationReport) {
        let expired = match self
            .repo
            .expired_promises(tenant_id, self.config.per_tenant_batch)
            .await
        {
            Ok(c) => c,
            Err(e) => return self.fail("promises", e, report),
        };
        for edge in expired {
            match self.repo.retire_promise(tenant_id, &edge.id).await {
                Ok(true) => {
                    report.promises_expired += 1;
                    get_exporter().inc_consolidation_promises_expired();
                }
                Ok(false) => {}
                Err(e) => self.fail("retire_promise", e, report),
            }
        }
    }

    async fn scan_web_decay(&self, tenant_id: &TenantId, report: &mut ConsolidationReport) {
        let observations = match self
            .repo
            .web_observations(
                tenant_id,
                self.config.web_decay_hours,
                self.config.per_tenant_batch,
            )
            .await
        {
            Ok(c) => c,
            Err(e) => return self.fail("web_decay", e, report),
        };
        for edge in observations {
            // Fixed plateaus keyed on age buckets keep the operation
            // idempotent: re-running sets the same target again.
            let age_hours = (chrono::Utc::now()
                - chrono::DateTime::parse_from_rfc3339(&edge.recorded_at)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()))
            .num_hours()
            .max(0);
            let plateau = if age_hours >= 2 * self.config.web_decay_hours as i64 {
                self.config.web_trust_plateau_2
            } else {
                self.config.web_trust_plateau_1
            };
            match self
                .repo
                .decay_web_trust(tenant_id, &edge.id, plateau)
                .await
            {
                Ok(true) => report.web_decayed += 1,
                Ok(false) => {}
                Err(e) => self.fail("web_decay_apply", e, report),
            }
        }
    }

    async fn reconcile_sor(&self, tenant_id: &TenantId, report: &mut ConsolidationReport) {
        let updates = match self.adapter.fetch_updates(tenant_id).await {
            Ok(u) => u,
            Err(e) => return self.fail("sor_fetch", e, report),
        };
        for update in updates {
            if let Err(e) = self.apply_sor_update(tenant_id, &update, report).await {
                self.fail("sor_apply", e, report);
            }
        }
    }

    async fn apply_sor_update(
        &self,
        tenant_id: &TenantId,
        update: &SorUpdate,
        report: &mut ConsolidationReport,
    ) -> Result<(), AppError> {
        // Unknown predicates cannot enter the governed store; they count as
        // reconciliation differences (surfaced) without silently broadening
        // the allowlist.
        if crate::models::belief::find_predicate(&update.predicate).is_none() {
            report.sor_diffs += 1;
            get_exporter().inc_consolidation_reconciliation_diffs();
            return Ok(());
        }

        // Reconciliation sees STALE edges too — an authority re-vouching for
        // (or replacing) an aged-out fact is the canonical SoR flow.
        let open = self
            .repo
            .reconcile_target(tenant_id, &update.subject, &update.predicate)
            .await?;
        let open_principal = open.as_ref().map(|e| e.principal_id.clone());
        let stale_edge_id = open
            .as_ref()
            .filter(|e| e.status == "stale")
            .map(|e| e.id.clone());
        if let Some(edge) = open.filter(|e| e.object == update.object) {
            // Authority re-vouched for the current truth: refresh the aging
            // clock (and revive a stale edge). No new version, no history edit.
            if self.repo.reconfirm_from_sor(tenant_id, &edge.id).await? {
                report.sor_refreshed += 1;
                report.sor_diffs += 1;
                get_exporter().inc_consolidation_reconciliation_diffs();
            }
            return Ok(());
        }

        // A stale target with a DIFFERENT object is replaced: close it first
        // so the gate's ADD leaves exactly one open edge (the write gate's
        // open-edge view intentionally does not include stale).
        if let Some(stale_id) = stale_edge_id {
            let _ = self.repo.close_stale_edge(tenant_id, &stale_id).await?;
        }

        // Changed (or brand new): through the WRITE GATE as system_of_record.
        // Evidence = immutable external_record event, idempotent per update.
        let principal_id = update
            .principal_id
            .clone()
            .or(open_principal)
            .unwrap_or_else(|| "__sor__".to_string());
        let event = self
            .events
            .append(
                tenant_id,
                AppendMemoryEventRequest::new(
                    principal_id.clone(),
                    MemoryEventType::ExternalRecord,
                )
                .actor(&update.system)
                .payload(serde_json::json!({
                    "system": update.system,
                    "subject": update.subject,
                    "predicate": update.predicate,
                    "object": update.object,
                }))
                .idempotency_key(format!(
                    "sor|{}|{}|{}|{}",
                    update.system, update.subject, update.predicate, update.object
                )),
            )
            .await?;

        let mut claim = BeliefClaim::new(
            principal_id,
            &update.subject,
            &update.predicate,
            &update.object,
            BeliefSource::SystemOfRecord,
        )
        .evidence(vec![event.id().to_string()])
        .payload(serde_json::json!({ "sor_system": update.system }));
        claim.idempotency_key = Some(format!(
            "sor|{}|{}|{}|{}",
            update.system, update.subject, update.predicate, update.object
        ));

        match self.gate.submit(tenant_id, claim).await {
            Ok(GateOutcome::Committed { .. }) => {
                report.sor_opened += 1;
                report.sor_diffs += 1;
                get_exporter().inc_consolidation_reconciliation_diffs();
            }
            Ok(GateOutcome::Superseded { .. }) => {
                report.sor_closed += 1;
                report.sor_diffs += 1;
                get_exporter().inc_consolidation_reconciliation_diffs();
            }
            Ok(GateOutcome::Noop { .. }) => {}
            Ok(other) => {
                report.errors.push(format!(
                    "sor update {:?} resolved unexpectedly: {other:?}",
                    update.predicate
                ));
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    fn fail(&self, stage: &str, e: AppError, report: &mut ConsolidationReport) {
        get_exporter().inc_consolidation_failures();
        report.errors.push(format!("{stage}: {e}"));
        tracing::warn!(stage, error = %e, "consolidation stage failed");
    }
}

// ============================================================================
// Background worker (#129): periodic, tenant-fair consolidation rounds.
// ============================================================================

use std::sync::atomic::{AtomicBool, Ordering};

/// Start the belief-consolidation loop. Idempotent; no-op on non-Postgres
/// backends (the belief store is PG-only). Non-critical by design: a failed
/// start logs and returns — the API surface must not depend on the worker.
pub async fn init_consolidation_worker() -> Result<(), AppError> {
    if !crate::db::is_postgres() {
        tracing::info!("consolidation worker skipped: belief store requires PostgreSQL");
        return Ok(());
    }
    let (enabled, interval_seconds, batch) = {
        let c = &crate::config::get().consolidation_worker;
        (c.enabled, c.interval_seconds, c.per_tenant_batch)
    };
    if !enabled {
        tracing::info!("consolidation worker disabled by config");
        return Ok(());
    }

    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let interval = std::time::Duration::from_secs(interval_seconds);
    tokio::spawn(async move {
        // Static adapter until a real CRM/HR integration lands (#129 scope:
        // interface + one minimal adapter); pushes nothing by default.
        let adapter: Arc<dyn SorAdapter> = Arc::new(StaticSorAdapter::new());
        let service = BeliefConsolidationService::new(
            crate::db::pool().clone(),
            adapter,
            BeliefConsolidationConfig {
                per_tenant_batch: batch,
                ..Default::default()
            },
        );
        loop {
            let tenants = crate::services::multi_tenant::list_scheduled_tenants();
            if !tenants.is_empty() {
                let reports = service.process_round(&tenants).await;
                let total_errors: usize = reports.values().map(|r| r.errors.len()).sum();
                if total_errors > 0 {
                    tracing::warn!(
                        tenants = tenants.len(),
                        total_errors,
                        "consolidation round had errors"
                    );
                } else {
                    tracing::info!(tenants = tenants.len(), "consolidation round complete");
                }
            }
            tokio::time::sleep(interval).await;
        }
    });
    Ok(())
}
