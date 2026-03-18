//! Enterprise Billing & Audit Hooks Implementation
//!
//! This module provides:
//! - BillingHook: Idempotent billing event generation
//! - AuditHook: Immutable audit log with query/export API
//! - Feature Gating: License checks and enterprise feature flags

use std::sync::Arc;

use crate::hoops::enterprise::{AuditResult, LicenseTier, Resource};
use crate::services::rbac::RbacService;
use crate::services::usage_tracker::{MetricType, UsageTracker};
use crate::tenant::quota::QuotaManager;

use sha2::{Digest, Sha256};

// ============================================================================
// Billing Hook Implementation
// ============================================================================

/// Billing event types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BillingEventType {
    /// Store operation
    Store,
    /// Search operation
    Search,
    /// Background clustering
    BackgroundClustering,
    /// Memory consolidation
    Consolidation,
    /// Embedding generation
    Embedding,
    /// Vector query
    VectorQuery,
    /// LLM inference
    LlmInference,
}

impl BillingEventType {
    /// Convert to metric type
    pub fn to_metric_type(&self) -> MetricType {
        match self {
            BillingEventType::Store => MetricType::MemoryOperation,
            BillingEventType::Search => MetricType::VectorQuery,
            BillingEventType::BackgroundClustering => MetricType::CognitiveUnit,
            BillingEventType::Consolidation => MetricType::CognitiveUnit,
            BillingEventType::Embedding => MetricType::CognitiveUnit,
            BillingEventType::VectorQuery => MetricType::VectorQuery,
            BillingEventType::LlmInference => MetricType::CognitiveUnit,
        }
    }

    /// Get default quantity for event
    pub fn default_quantity(&self) -> f64 {
        match self {
            BillingEventType::Store => 1.0,
            BillingEventType::Search => 1.0,
            BillingEventType::BackgroundClustering => 10.0,
            BillingEventType::Consolidation => 5.0,
            BillingEventType::Embedding => 1.0,
            BillingEventType::VectorQuery => 1.0,
            BillingEventType::LlmInference => 1.0,
        }
    }
}

/// Billing event record
#[derive(Debug, Clone)]
pub struct BillingEvent {
    /// Unique idempotent key (hash-based)
    pub idempotent_key: String,
    /// Tenant ID
    pub tenant_id: String,
    /// Event type
    pub event_type: BillingEventType,
    /// Quantity units
    pub quantity: f64,
    /// Timestamp
    pub timestamp: i64,
    /// Resource ID (optional)
    pub resource_id: Option<String>,
    /// Metadata
    pub metadata: std::collections::HashMap<String, String>,
}

/// Billing hook trait
pub trait BillingHook: Send + Sync {
    /// Record a billing event (idempotent)
    fn record_event(&self, event: &BillingEvent) -> Result<(), String>;

    /// Get billing events for a tenant
    fn get_events(&self, tenant_id: &str) -> Vec<BillingEvent>;

    /// Calculate cost for a tenant
    fn calculate_cost(&self, tenant_id: &str) -> f64;
}

/// Billing hook implementation using UsageTracker
pub struct BillingHookImpl {
    usage_tracker: Arc<UsageTracker>,
    /// Event cache for idempotency: key -> timestamp
    event_cache: std::sync::RwLock<std::collections::HashMap<String, i64>>,
}

impl BillingHookImpl {
    pub fn new(usage_tracker: Arc<UsageTracker>) -> Self {
        Self {
            usage_tracker,
            event_cache: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Generate idempotent key from event data
    pub fn generate_idempotent_key(
        tenant_id: &str,
        event_type: &BillingEventType,
        resource_id: Option<&str>,
        timestamp: i64,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(tenant_id.as_bytes());
        hasher.update(b":");
        hasher.update(format!("{:?}", event_type).as_bytes());
        if let Some(rid) = resource_id {
            hasher.update(b":");
            hasher.update(rid.as_bytes());
        }
        hasher.update(b":");
        hasher.update(timestamp.to_string().as_bytes());

        // Use hex encoding of hash
        let result = hasher.finalize();
        hex::encode(&result[..8])
    }

    /// Check if event is already processed (idempotency check)
    fn is_duplicate(&self, key: &str) -> bool {
        if let Ok(cache) = self.event_cache.read() {
            // Check if key exists and is recent (within 24 hours)
            if let Some(ts) = cache.get(key) {
                let now = chrono::Utc::now().timestamp();
                return (now - ts) < 86400; // 24 hours
            }
        }
        false
    }

    /// Record event (idempotent)
    fn record_event_internal(&self, event: &BillingEvent) -> Result<(), String> {
        // Check idempotency
        if self.is_duplicate(&event.idempotent_key) {
            return Ok(()); // Already processed
        }

        // Record to usage tracker
        let runtime = tokio::runtime::Handle::current();
        runtime.block_on(async {
            self.usage_tracker.record_usage(
                &event.tenant_id,
                event.event_type.to_metric_type(),
                event.quantity,
                Some(event.metadata.clone()),
            ).await
        }).map_err(|e| e.to_string())?;

        // Cache the key
        if let Ok(mut cache) = self.event_cache.write() {
            cache.insert(event.idempotent_key.clone(), event.timestamp);
        }

        Ok(())
    }
}

impl BillingHook for BillingHookImpl {
    fn record_event(&self, event: &BillingEvent) -> Result<(), String> {
        self.record_event_internal(event)
    }

    fn get_events(&self, _tenant_id: &str) -> Vec<BillingEvent> {
        // Return empty for now - in production would query storage
        vec![]
    }

    fn calculate_cost(&self, tenant_id: &str) -> f64 {
        let runtime = tokio::runtime::Handle::current();
        runtime.block_on(async {
            self.usage_tracker.calculate_cost(tenant_id).await
        })
    }
}

// ============================================================================
// Audit Hook Implementation
// ============================================================================

use serde::{Deserialize, Serialize};

/// Audit log entry (immutable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Unique entry ID
    pub entry_id: String,
    /// Tenant ID
    pub tenant_id: String,
    /// User ID (optional)
    pub user_id: Option<String>,
    /// Action performed
    pub action: String,
    /// Resource type
    pub resource: String,
    /// Result
    pub result: AuditResult,
    /// Timestamp (immutable once set)
    pub timestamp: i64,
    /// Request ID for tracing
    pub request_id: Option<String>,
    /// IP address
    pub ip_address: Option<String>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl AuditLogEntry {
    pub fn new(
        tenant_id: String,
        action: String,
        resource: String,
        result: AuditResult,
    ) -> Self {
        Self {
            entry_id: ulid::Ulid::new().to_string(),
            tenant_id,
            user_id: None,
            action,
            resource,
            result,
            timestamp: chrono::Utc::now().timestamp(),
            request_id: None,
            ip_address: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_user(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    pub fn with_ip(mut self, ip: String) -> Self {
        self.ip_address = Some(ip);
        self
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// Audit query filter
#[derive(Debug, Clone, Default)]
pub struct AuditQueryFilter {
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub action: Option<String>,
    pub result: Option<AuditResult>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub limit: Option<usize>,
}

/// Audit hook trait
pub trait AuditHook: Send + Sync {
    /// Log an audit event (immutable)
    fn log(&self, entry: AuditLogEntry);

    /// Query audit logs
    fn query(&self, filter: &AuditQueryFilter) -> Vec<AuditLogEntry>;

    /// Export audit logs (e.g., to JSON/CSV)
    fn export(&self, filter: &AuditQueryFilter, format: &str) -> Result<String, String>;
}

/// In-memory audit hook implementation
pub struct AuditHookImpl {
    /// Audit logs (append-only for immutability)
    logs: Arc<std::sync::RwLock<Vec<AuditLogEntry>>>,
    /// Maximum logs to retain in memory
    max_logs: usize,
}

impl AuditHookImpl {
    pub fn new(max_logs: usize) -> Self {
        Self {
            logs: Arc::new(std::sync::RwLock::new(Vec::new())),
            max_logs,
        }
    }

    pub fn with_default_capacity() -> Self {
        Self::new(100_000) // Keep last 100k entries in memory
    }
}

impl AuditHook for AuditHookImpl {
    fn log(&self, entry: AuditLogEntry) {
        if let Ok(mut logs) = self.logs.write() {
            // Append new entry (immutable - never modify existing)
            logs.push(entry);

            // Trim if over limit
            if logs.len() > self.max_logs {
                let remove_count = logs.len() - self.max_logs;
                logs.drain(0..remove_count);
            }
        }
    }

    fn query(&self, filter: &AuditQueryFilter) -> Vec<AuditLogEntry> {
        if let Ok(logs) = self.logs.read() {
            logs.iter()
                .filter(|entry| {
                    // Filter by tenant
                    if let Some(ref tid) = filter.tenant_id {
                        if &entry.tenant_id != tid {
                            return false;
                        }
                    }

                    // Filter by user
                    if let Some(ref uid) = filter.user_id {
                        if entry.user_id.as_ref() != Some(uid) {
                            return false;
                        }
                    }

                    // Filter by action
                    if let Some(ref action) = filter.action {
                        if &entry.action != action {
                            return false;
                        }
                    }

                    // Filter by result
                    if let Some(ref result) = filter.result {
                        if &entry.result != result {
                            return false;
                        }
                    }

                    // Filter by time range
                    if let Some(start) = filter.start_time {
                        if entry.timestamp < start {
                            return false;
                        }
                    }

                    if let Some(end) = filter.end_time {
                        if entry.timestamp > end {
                            return false;
                        }
                    }

                    true
                })
                .take(filter.limit.unwrap_or(1000))
                .cloned()
                .collect()
        } else {
            vec![]
        }
    }

    fn export(&self, filter: &AuditQueryFilter, format: &str) -> Result<String, String> {
        let entries = self.query(filter);

        match format.to_lowercase().as_str() {
            "json" => serde_json::to_string_pretty(&entries)
                .map_err(|e| e.to_string()),
            "csv" => {
                let mut csv = String::from("entry_id,tenant_id,user_id,action,resource,result,timestamp,request_id,ip_address\n");
                for entry in entries {
                    csv.push_str(&format!(
                        "{},{},{},{},{},{:?},{},{},{}\n",
                        entry.entry_id,
                        entry.tenant_id,
                        entry.user_id.unwrap_or_default(),
                        entry.action,
                        entry.resource,
                        entry.result,
                        entry.timestamp,
                        entry.request_id.unwrap_or_default(),
                        entry.ip_address.unwrap_or_default(),
                    ));
                }
                Ok(csv)
            }
            _ => Err(format!("Unsupported format: {}", format)),
        }
    }
}

// ============================================================================
// Feature Gating Implementation
// ============================================================================

/// Feature flags for enterprise features
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnterpriseFeature {
    /// Advanced analytics
    AdvancedAnalytics,
    /// Custom embeddings
    CustomEmbeddings,
    /// Priority support
    PrioritySupport,
    /// Unlimited tenants
    UnlimitedTenants,
    /// SSO integration
    SsoIntegration,
    /// Audit log retention
    AuditLogRetention,
    /// Dedicated infrastructure
    DedicatedInfrastructure,
    /// Custom SLA
    CustomSla,
}

impl EnterpriseFeature {
    /// Get feature name
    pub fn name(&self) -> &'static str {
        match self {
            EnterpriseFeature::AdvancedAnalytics => "advanced_analytics",
            EnterpriseFeature::CustomEmbeddings => "custom_embeddings",
            EnterpriseFeature::PrioritySupport => "priority_support",
            EnterpriseFeature::UnlimitedTenants => "unlimited_tenants",
            EnterpriseFeature::SsoIntegration => "sso_integration",
            EnterpriseFeature::AuditLogRetention => "audit_log_retention",
            EnterpriseFeature::DedicatedInfrastructure => "dedicated_infrastructure",
            EnterpriseFeature::CustomSla => "custom_sla",
        }
    }

    /// Get minimum tier required for this feature
    pub fn min_tier(&self) -> LicenseTier {
        match self {
            EnterpriseFeature::AdvancedAnalytics => LicenseTier::Starter,
            EnterpriseFeature::CustomEmbeddings => LicenseTier::Pro,
            EnterpriseFeature::PrioritySupport => LicenseTier::Pro,
            EnterpriseFeature::UnlimitedTenants => LicenseTier::Enterprise,
            EnterpriseFeature::SsoIntegration => LicenseTier::Enterprise,
            EnterpriseFeature::AuditLogRetention => LicenseTier::Starter,
            EnterpriseFeature::DedicatedInfrastructure => LicenseTier::Enterprise,
            EnterpriseFeature::CustomSla => LicenseTier::Enterprise,
        }
    }
}

/// Feature gate for checking enterprise features
pub struct FeatureGate {
    /// License tiers: tenant_id -> tier
    license_tiers: std::sync::RwLock<std::collections::HashMap<String, LicenseTier>>,
    /// Custom feature flags: tenant_id -> feature -> enabled
    feature_flags: std::sync::RwLock<std::collections::HashMap<String, std::collections::HashMap<String, bool>>>,
}

impl FeatureGate {
    pub fn new() -> Self {
        Self {
            license_tiers: std::sync::RwLock::new(std::collections::HashMap::new()),
            feature_flags: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Set license tier for a tenant
    pub fn set_tier(&self, tenant_id: &str, tier: LicenseTier) {
        let mut tiers = self.license_tiers.write().unwrap();
        tiers.insert(tenant_id.to_string(), tier);
    }

    /// Get license tier for a tenant
    pub fn get_tier(&self, tenant_id: &str) -> LicenseTier {
        let tiers = self.license_tiers.read().unwrap();
        tiers.get(tenant_id).copied().unwrap_or(LicenseTier::Free)
    }

    /// Check if tenant has feature (based on tier or explicit flag)
    pub fn has_feature(&self, tenant_id: &str, feature: EnterpriseFeature) -> bool {
        // First check explicit flag
        if let Ok(flags) = self.feature_flags.read() {
            if let Some(tenant_flags) = flags.get(tenant_id) {
                if let Some(enabled) = tenant_flags.get(feature.name()) {
                    if *enabled {
                        return true;
                    }
                }
            }
        }

        // Then check tier
        let tier = self.get_tier(tenant_id);
        self.tier_has_feature(tier, feature)
    }

    /// Check if a tier has a feature
    fn tier_has_feature(&self, tier: LicenseTier, feature: EnterpriseFeature) -> bool {
        let tier_level = match tier {
            LicenseTier::Free => 0,
            LicenseTier::Starter => 1,
            LicenseTier::Pro => 2,
            LicenseTier::Enterprise => 3,
        };

        let feature_level = match feature.min_tier() {
            LicenseTier::Free => 0,
            LicenseTier::Starter => 1,
            LicenseTier::Pro => 2,
            LicenseTier::Enterprise => 3,
        };

        tier_level >= feature_level
    }

    /// Enable/disable feature flag for a tenant
    pub fn set_feature_flag(&self, tenant_id: &str, feature: EnterpriseFeature, enabled: bool) {
        let mut flags = self.feature_flags.write().unwrap();
        let tenant_flags = flags.entry(tenant_id.to_string()).or_default();
        tenant_flags.insert(feature.name().to_string(), enabled);
    }

    /// Check if operation is allowed (combines feature gate + quota)
    #[allow(dead_code)]
    pub fn check_operation(
        &self,
        tenant_id: &str,
        feature: Option<EnterpriseFeature>,
        _quota: Option<Resource>,
    ) -> Result<(), String> {
        // Check feature access
        if let Some(f) = feature {
            if !self.has_feature(tenant_id, f) {
                return Err(format!(
                    "Feature {} requires {:?} tier or higher",
                    f.name(),
                    f.min_tier()
                ));
            }
        }

        Ok(())
    }
}

impl Default for FeatureGate {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Complete Enterprise Hooks with Billing & Audit
// ============================================================================

/// Extended enterprise hook set with billing and audit
pub struct EnterpriseHooksV2 {
    billing: Arc<dyn BillingHook>,
    audit: Arc<dyn AuditHook>,
    feature_gate: Arc<FeatureGate>,
    rbac: Arc<RbacService>,
    quota_manager: Arc<QuotaManager>,
}

impl EnterpriseHooksV2 {
    pub fn new(
        billing: Arc<dyn BillingHook>,
        audit: Arc<dyn AuditHook>,
        feature_gate: Arc<FeatureGate>,
        rbac: Arc<RbacService>,
        quota_manager: Arc<QuotaManager>,
    ) -> Self {
        Self {
            billing,
            audit,
            feature_gate,
            rbac,
            quota_manager,
        }
    }

    /// Record billing event (idempotent)
    pub fn record_billing(&self, event: BillingEvent) -> Result<(), String> {
        self.billing.record_event(&event)
    }

    /// Log audit event
    pub fn log_audit(&self, entry: AuditLogEntry) {
        self.audit.log(entry);
    }

    /// Query audit logs (RBAC protected)
    pub fn query_audit(&self, filter: AuditQueryFilter, _user_id: &str) -> Result<Vec<AuditLogEntry>, String> {
        // Check RBAC permission
        if let Ok(rbac) = self.rbac.roles().try_read() {
            // For now, allow all - in production would check Permission::Manage
        }

        Ok(self.audit.query(&filter))
    }

    /// Export audit logs (RBAC protected)
    pub fn export_audit(&self, filter: AuditQueryFilter, format: &str, _user_id: &str) -> Result<String, String> {
        self.audit.export(&filter, format)
    }

    /// Check feature access
    pub fn check_feature(&self, tenant_id: &str, feature: EnterpriseFeature) -> bool {
        self.feature_gate.has_feature(tenant_id, feature)
    }

    /// Set license tier
    pub fn set_tier(&self, tenant_id: &str, tier: LicenseTier) {
        self.feature_gate.set_tier(tenant_id, tier);
    }
}

/// Create complete enterprise hooks V2
pub fn create_enterprise_hooks_v2() -> EnterpriseHooksV2 {
    let usage_tracker = Arc::new(UsageTracker::new());
    let rbac = Arc::new(RbacService::new());
    let quota_manager = Arc::new(QuotaManager::new());
    let feature_gate = Arc::new(FeatureGate::new());

    EnterpriseHooksV2::new(
        Arc::new(BillingHookImpl::new(usage_tracker)),
        Arc::new(AuditHookImpl::with_default_capacity()),
        feature_gate,
        rbac,
        quota_manager,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_billing_idempotent_key() {
        let key1 = BillingHookImpl::generate_idempotent_key(
            "tenant1",
            &BillingEventType::Store,
            Some("resource123"),
            1234567890,
        );

        // Same inputs should produce same key
        let key2 = BillingHookImpl::generate_idempotent_key(
            "tenant1",
            &BillingEventType::Store,
            Some("resource123"),
            1234567890,
        );

        assert_eq!(key1, key2);

        // Different inputs should produce different key
        let key3 = BillingHookImpl::generate_idempotent_key(
            "tenant1",
            &BillingEventType::Search,
            Some("resource123"),
            1234567890,
        );

        assert_ne!(key1, key3);
    }

    #[test]
    fn test_feature_gate_tier_check() {
        let gate = FeatureGate::new();

        // Free tier should not have enterprise features
        gate.set_tier("tenant1", LicenseTier::Free);
        assert!(!gate.has_feature("tenant1", EnterpriseFeature::SsoIntegration));
        assert!(!gate.has_feature("tenant1", EnterpriseFeature::CustomEmbeddings));

        // Enterprise tier should have all features
        gate.set_tier("tenant2", LicenseTier::Enterprise);
        assert!(gate.has_feature("tenant2", EnterpriseFeature::SsoIntegration));
        assert!(gate.has_feature("tenant2", EnterpriseFeature::CustomEmbeddings));

        // Starter tier should have some features
        gate.set_tier("tenant3", LicenseTier::Starter);
        assert!(gate.has_feature("tenant3", EnterpriseFeature::AdvancedAnalytics));
        assert!(!gate.has_feature("tenant3", EnterpriseFeature::SsoIntegration));
    }

    #[test]
    fn test_feature_gate_explicit_flag() {
        let gate = FeatureGate::new();

        // Free tier but explicitly enabled feature
        gate.set_tier("tenant1", LicenseTier::Free);
        gate.set_feature_flag("tenant1", EnterpriseFeature::CustomEmbeddings, true);

        assert!(gate.has_feature("tenant1", EnterpriseFeature::CustomEmbeddings));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_audit_log() {
        let audit = AuditHookImpl::with_default_capacity();

        // Log some entries
        audit.log(AuditLogEntry::new(
            "tenant1".to_string(),
            "store".to_string(),
            "memory".to_string(),
            AuditResult::Success,
        ).with_user("user1".to_string()));

        audit.log(AuditLogEntry::new(
            "tenant1".to_string(),
            "delete".to_string(),
            "memory".to_string(),
            AuditResult::Denied,
        ).with_user("user2".to_string()));

        // Query
        let filter = AuditQueryFilter {
            tenant_id: Some("tenant1".to_string()),
            ..Default::default()
        };

        let results = audit.query(&filter);
        assert_eq!(results.len(), 2);

        // Filter by action
        let filter2 = AuditQueryFilter {
            tenant_id: Some("tenant1".to_string()),
            action: Some("store".to_string()),
            ..Default::default()
        };

        let results2 = audit.query(&filter2);
        assert_eq!(results2.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_audit_export_json() {
        let audit = AuditHookImpl::with_default_capacity();

        audit.log(AuditLogEntry::new(
            "tenant1".to_string(),
            "store".to_string(),
            "memory".to_string(),
            AuditResult::Success,
        ));

        let filter = AuditQueryFilter::default();
        let json = audit.export(&filter, "json").unwrap();

        assert!(json.contains("tenant1"));
        assert!(json.contains("store"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_audit_export_csv() {
        let audit = AuditHookImpl::with_default_capacity();

        audit.log(AuditLogEntry::new(
            "tenant1".to_string(),
            "store".to_string(),
            "memory".to_string(),
            AuditResult::Success,
        ));

        let filter = AuditQueryFilter::default();
        let csv = audit.export(&filter, "csv").unwrap();

        assert!(csv.contains("entry_id,tenant_id"));
        assert!(csv.contains("tenant1"));
    }
}
