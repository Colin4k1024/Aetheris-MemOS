//! Enterprise Hooks Implementation
//!
//! This module provides concrete implementations of enterprise hooks:
//! - JwtAuthHook: JWT + API Key authentication
//! - RbacHookImpl: Role-based access control
//! - TenantQuotaHook: Tenant quota management
//! - GovernanceHookImpl: Comprehensive governance with audit

use std::sync::Arc;

use crate::hoops::enterprise::{
    AuditEvent, AuditResult, AuthHook, GovernanceHook, HookContext, HookDecision, HookError,
    HookResult, LicenseTier, QuotaResult, RbacHook, Resource, ServerBuilder, UsageSnapshot,
};
use crate::services::rbac::{get_rbac_service, Permission, RbacService, Role};
use crate::tenant::context::QuotaResource;
use crate::tenant::quota::QuotaManager;

/// Auth context - carries authentication information
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub roles: Vec<Role>,
    pub license_tier: LicenseTier,
}

impl AuthContext {
    pub fn new(tenant_id: String) -> Self {
        Self {
            tenant_id,
            user_id: None,
            roles: vec![],
            license_tier: LicenseTier::Free,
        }
    }

    pub fn with_user(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_role(mut self, role: Role) -> Self {
        self.roles.push(role);
        self
    }

    pub fn with_tier(mut self, tier: LicenseTier) -> Self {
        self.license_tier = tier;
        self
    }
}

// ============================================================================
// JWT Auth Hook Implementation
// ============================================================================

/// JWT + API Key authentication hook implementation
pub struct JwtAuthHookImpl {
    #[allow(dead_code)]
    rbac: Arc<RbacService>,
    /// API keys: key -> tenant_id
    api_keys: std::sync::RwLock<std::collections::HashMap<String, String>>,
    /// License tiers: tenant_id -> tier
    license_tiers: std::sync::RwLock<std::collections::HashMap<String, LicenseTier>>,
}

impl JwtAuthHookImpl {
    pub fn new(rbac: Arc<RbacService>) -> Self {
        Self {
            rbac,
            api_keys: std::sync::RwLock::new(std::collections::HashMap::new()),
            license_tiers: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Register an API key for a tenant
    pub fn register_api_key(&self, api_key: String, tenant_id: String) {
        let mut keys = self.api_keys.write().unwrap();
        keys.insert(api_key, tenant_id);
    }

    /// Set license tier for a tenant
    pub fn set_license_tier(&self, tenant_id: &str, tier: LicenseTier) {
        let mut tiers = self.license_tiers.write().unwrap();
        tiers.insert(tenant_id.to_string(), tier);
    }

    /// Get license tier for a tenant
    pub fn get_license_tier(&self, tenant_id: &str) -> LicenseTier {
        let tiers = self.license_tiers.read().unwrap();
        tiers.get(tenant_id).copied().unwrap_or(LicenseTier::Free)
    }

    /// Validate JWT token and extract tenant/user info.
    ///
    /// Reads the `org` claim for the tenant id (falling back to `uid` for tokens
    /// issued before the claim existed). The old `_`-split convention was a
    /// divergent derivation path that disagreed with the primary auth middleware —
    /// replaced in C-3 / PR-3.
    fn validate_jwt(&self, token: &str) -> Option<AuthContext> {
        let claims = crate::hoops::jwt::decode_token_claims(token)?;

        let tenant_id = claims.org.unwrap_or_else(|| claims.uid.clone());
        let user_id = claims.uid;

        let tier = self.get_license_tier(&tenant_id);

        Some(
            AuthContext::new(tenant_id)
                .with_user(user_id)
                .with_tier(tier),
        )
    }
}

impl AuthHook for JwtAuthHookImpl {
    fn validate_api_key(&self, api_key: &str) -> Option<String> {
        let keys = self.api_keys.read().unwrap();
        keys.get(api_key).cloned()
    }

    fn get_tenant_from_token(&self, token: &str) -> Option<String> {
        // Try JWT first
        if let Some(ctx) = self.validate_jwt(token) {
            return Some(ctx.tenant_id);
        }

        // Try API key
        let keys = self.api_keys.read().unwrap();
        keys.get(token).cloned()
    }
}

// ============================================================================
// RBAC Hook Implementation
// ============================================================================

/// RBAC hook implementation using RbacService
pub struct RbacHookImpl {
    rbac: Arc<RbacService>,
    /// Audit callback for denied actions
    audit_callback: Option<Arc<dyn Fn(AuditEvent) + Send + Sync>>,
}

impl RbacHookImpl {
    pub fn new(rbac: Arc<RbacService>) -> Self {
        Self {
            rbac,
            audit_callback: None,
        }
    }

    /// Set audit callback for denied actions
    #[allow(dead_code)]
    pub fn with_audit_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(AuditEvent) + Send + Sync + 'static,
    {
        self.audit_callback = Some(Arc::new(callback));
        self
    }

    /// Log audit event for denied action
    fn log_denial(&self, tenant_id: &str, _user_id: &str, resource: &str, action: &str) {
        if let Some(callback) = &self.audit_callback {
            let event = AuditEvent::new(
                tenant_id.to_string(),
                action.to_string(),
                resource.to_string(),
                AuditResult::Denied,
            );
            callback(event);
        }
    }

    /// **Deny-only stub.** This hook no longer performs authorization.
    ///
    /// The RBAC decision moved to `hoops::governance::governance_middleware`
    /// (C-3 / PR-2b): roles live in `tenant_members`, so the lookup is an async
    /// database read and this trait is synchronous. There is no correct answer
    /// this method can compute, so it returns `false` and says why in a log
    /// rather than guessing.
    ///
    /// `false` — not `true` — because a stub on an authorization path must fail
    /// closed. Nothing in production reaches it today (`GovernanceHookImpl` no
    /// longer calls it), so the denial is inert; if some future caller does wire
    /// it up, it will get an obviously-broken deny with an explanation instead of
    /// a silent blanket allow.
    ///
    /// The old string→Permission mapping it used to apply defaulted unknown
    /// actions to `Read`, which is exactly the kind of quiet under-classification
    /// the exhaustive `rbac::operation_to_permission` now prevents.
    fn deny_with_reason(
        &self,
        tenant_id: &str,
        user_id: &str,
        resource: &str,
        action: &str,
    ) -> bool {
        tracing::warn!(
            tenant = %tenant_id,
            user = %user_id,
            resource = %resource,
            action = %action,
            "RbacHookImpl::check_permission is a deny-only stub since C-3; \
             authorization belongs to governance_middleware. Denying."
        );
        false
    }
}

impl RbacHook for RbacHookImpl {
    fn check_permission(
        &self,
        tenant_id: &str,
        user_id: &str,
        resource: &str,
        action: &str,
    ) -> bool {
        let result = self.deny_with_reason(tenant_id, user_id, resource, action);
        if !result {
            self.log_denial(tenant_id, user_id, resource, action);
        }
        result
    }

    fn get_roles(&self, _tenant_id: &str, _user_id: &str) -> Vec<String> {
        // Would need async - return empty for sync implementation
        vec![]
    }
}

// ============================================================================
// Tenant Quota Hook Implementation
// ============================================================================

/// Tenant quota hook implementation - partial GovernanceHook for quota management
pub struct TenantQuotaHookImpl {
    quota_manager: Arc<QuotaManager>,
    /// Soft limit threshold (percentage, 0-100)
    soft_limit_threshold: u8,
    /// Audit callback
    audit_callback: Option<Arc<dyn Fn(AuditEvent) + Send + Sync>>,
}

impl TenantQuotaHookImpl {
    pub fn new(quota_manager: Arc<QuotaManager>) -> Self {
        Self {
            quota_manager,
            soft_limit_threshold: 80, // 80% is soft limit
            audit_callback: None,
        }
    }

    /// Set soft limit threshold
    #[allow(dead_code)]
    pub fn with_soft_limit(mut self, threshold: u8) -> Self {
        self.soft_limit_threshold = threshold.min(100);
        self
    }

    /// Set audit callback
    #[allow(dead_code)]
    pub fn with_audit_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(AuditEvent) + Send + Sync + 'static,
    {
        self.audit_callback = Some(Arc::new(callback));
        self
    }

    /// Check if resource is over soft limit (warning threshold)
    fn is_soft_limit_exceeded(&self, result: &QuotaResult) -> bool {
        if result.limit == 0 {
            return false;
        }
        let percentage = (result.current as f64 / result.limit as f64 * 100.0) as u8;
        percentage >= self.soft_limit_threshold
    }

    /// Log quota warning
    fn log_quota_warning(&self, tenant_id: &str, resource: &Resource, _result: &QuotaResult) {
        if let Some(callback) = &self.audit_callback {
            let event = AuditEvent::new(
                tenant_id.to_string(),
                "quota_warning".to_string(),
                format!("{:?}", resource),
                AuditResult::Failure,
            );
            callback(event);
        }
    }

    /// Access the underlying quota manager (e.g. for usage increments in post-hooks).
    pub fn quota_manager(&self) -> &Arc<QuotaManager> {
        &self.quota_manager
    }

    /// Check quota - public method
    pub fn check_quota(&self, tenant_id: &str, resource: Resource) -> QuotaResult {
        // Map enterprise Resource to QuotaResource
        let quota_resource = match resource {
            Resource::ApiCalls => QuotaResource::ApiCallsPerDay,
            Resource::StorageMb => QuotaResource::StorageMB,
            Resource::CognitiveUnits => QuotaResource::ApiCallsPerDay, // Map to API calls
            Resource::MemoryOperations => QuotaResource::MemoryEntries,
            Resource::VectorQueries => QuotaResource::ApiCallsPerDay, // Map to API calls
        };

        // Lazy self-healing: provision a default quota record for any tenant
        // that does not yet have one, so that usage counting has a record to
        // count against.  The quota map is in-memory, so it must self-heal
        // after every restart — this mirrors the Owner auto-grant in
        // `services/rbac.rs`.
        let quota = self.quota_manager.ensure_quota(tenant_id);

        let allowed = quota.check(&quota_resource);
        let remaining = quota.remaining(&quota_resource);
        let limit = match quota_resource {
            QuotaResource::StorageMB => quota.storage_mb,
            QuotaResource::ApiCallsPerDay => quota.api_calls_per_day,
            QuotaResource::ConcurrentSessions => quota.concurrent_sessions as u64,
            QuotaResource::MemoryEntries => quota.memory_entries,
        };
        let current = limit.saturating_sub(remaining);
        let overage = current.saturating_sub(limit) as i64;

        let result = QuotaResult {
            allowed,
            current,
            limit,
            overage,
        };

        // Log warning if over soft limit
        if self.is_soft_limit_exceeded(&result) {
            self.log_quota_warning(tenant_id, &resource, &result);
        }

        result
    }

    /// Get usage - public method
    pub fn get_usage(&self, tenant_id: &str) -> Option<UsageSnapshot> {
        let quota = self.quota_manager.get_quota(tenant_id)?;

        Some(UsageSnapshot {
            tenant_id: tenant_id.to_string(),
            tier: LicenseTier::Free, // Would need to get from license service
            api_calls: quota.used.api_calls_today,
            storage_mb: quota.used.storage_mb,
            cognitive_units: 0,
            memory_operations: quota.used.memory_entries,
            vector_queries: 0,
        })
    }
}

// ============================================================================
// Governance Hook Implementation (Combined)
// ============================================================================

/// Combined governance hook implementation
pub struct GovernanceHookImpl {
    rbac: Arc<RbacHookImpl>,
    quota: Arc<TenantQuotaHookImpl>,
    license_tiers: std::sync::RwLock<std::collections::HashMap<String, LicenseTier>>,
    features: std::sync::RwLock<std::collections::HashMap<String, Vec<String>>>,
    audit_callback: Arc<dyn Fn(AuditEvent) + Send + Sync>,
}

impl GovernanceHookImpl {
    pub fn new(
        rbac: Arc<RbacHookImpl>,
        quota: Arc<TenantQuotaHookImpl>,
        audit_callback: Arc<dyn Fn(AuditEvent) + Send + Sync>,
    ) -> Self {
        Self {
            rbac,
            quota,
            license_tiers: std::sync::RwLock::new(std::collections::HashMap::new()),
            features: std::sync::RwLock::new(std::collections::HashMap::new()),
            audit_callback,
        }
    }

    /// Set license tier for a tenant
    pub fn set_license_tier(&self, tenant_id: &str, tier: LicenseTier) {
        let mut tiers = self.license_tiers.write().unwrap();
        tiers.insert(tenant_id.to_string(), tier);
    }

    /// Enable a feature for a tenant
    pub fn enable_feature(&self, tenant_id: &str, feature: &str) {
        let mut features = self.features.write().unwrap();
        let tenant_features = features.entry(tenant_id.to_string()).or_default();
        if !tenant_features.contains(&feature.to_string()) {
            tenant_features.push(feature.to_string());
        }
    }

    /// Record audit event
    fn record_audit(&self, event: AuditEvent) {
        (self.audit_callback)(event);
    }
}

impl GovernanceHook for GovernanceHookImpl {
    fn check_license(&self, tenant_id: &str, tier: LicenseTier) -> bool {
        let tiers = self.license_tiers.read().unwrap();
        let current_tier = tiers.get(tenant_id).copied().unwrap_or(LicenseTier::Free);

        // Check if current tier is sufficient for requested tier
        let current_level = match current_tier {
            LicenseTier::Free => 0,
            LicenseTier::Starter => 1,
            LicenseTier::Pro => 2,
            LicenseTier::Enterprise => 3,
        };
        let required_level = match tier {
            LicenseTier::Free => 0,
            LicenseTier::Starter => 1,
            LicenseTier::Pro => 2,
            LicenseTier::Enterprise => 3,
        };

        current_level >= required_level
    }

    fn check_feature(&self, tenant_id: &str, feature: &str) -> bool {
        let features = self.features.read().unwrap();
        features
            .get(tenant_id)
            .map(|f| f.contains(&feature.to_string()))
            .unwrap_or(false)
    }

    fn check_quota(&self, tenant_id: &str, resource: Resource) -> QuotaResult {
        self.quota.check_quota(tenant_id, resource)
    }

    fn record_audit(&self, event: AuditEvent) {
        self.record_audit(event);
    }

    fn get_usage(&self, tenant_id: &str) -> Option<UsageSnapshot> {
        self.quota.get_usage(tenant_id)
    }

    fn pre_store(&self, ctx: &HookContext) -> HookDecision {
        // Check quota for store operations
        let quota_result = self.check_quota(&ctx.tenant_id, Resource::MemoryOperations);
        if !quota_result.allowed {
            self.record_audit(AuditEvent::new(
                ctx.tenant_id.clone(),
                "pre_store".to_string(),
                "quota_exceeded".to_string(),
                AuditResult::Denied,
            ));
            return HookDecision::Deny(format!(
                "Quota exceeded: {}/{}",
                quota_result.current, quota_result.limit
            ));
        }

        // RBAC is NOT checked here. It moved to `hoops::governance::governance_middleware`
        // in C-3 / PR-2b, which runs it before this hook — see that call site for
        // the reasoning. In short: roles now live in `tenant_members`, so the
        // lookup is a database read, and this trait is synchronous.
        //
        // Leaving a second, weaker check here would be worse than removing it: it
        // mapped a string action to a Permission with an "unknown → Read" default,
        // so the two planes could disagree about the same request. This hook's job
        // is quota.

        HookDecision::Allow
    }

    fn pre_search(&self, ctx: &HookContext) -> HookDecision {
        // Check quota for search operations
        let quota_result = self.check_quota(&ctx.tenant_id, Resource::VectorQueries);
        if !quota_result.allowed {
            self.record_audit(AuditEvent::new(
                ctx.tenant_id.clone(),
                "pre_search".to_string(),
                "quota_exceeded".to_string(),
                AuditResult::Denied,
            ));
            return HookDecision::Deny("Search quota exceeded".to_string());
        }

        // RBAC is NOT checked here — see the note in `pre_store`. Authorization
        // runs in `governance_middleware` before this hook; this one does quota.

        HookDecision::Allow
    }

    fn on_error(&self, ctx: &HookContext, _error: &HookError) {
        self.record_audit(AuditEvent::new(
            ctx.tenant_id.clone(),
            "error".to_string(),
            ctx.operation.as_str().to_string(),
            AuditResult::Failure,
        ));
    }

    fn post_store(&self, ctx: &HookContext, result: &HookResult) {
        if result.success {
            let quota_manager = self.quota.quota_manager();
            // Lazy self-healing: provision default quota if none exists for this tenant.
            // The quota map is in-memory, so it must self-heal after every restart.
            let mut quota = quota_manager.ensure_quota(&ctx.tenant_id);
            quota.used.memory_entries += 1;
            let new_used = quota.used.memory_entries;
            let limit = quota.memory_entries;
            quota_manager.set_quota(&ctx.tenant_id, quota);

            // Publish the usage ratio here rather than from `QuotaManager::set_quota`:
            // this is the only place that holds tenant + current + limit together,
            // and putting a Prometheus side effect inside a generic map setter would
            // also fire it from unrelated unit tests.
            //
            // CARDINALITY: `tenant` is an unbounded label — one series per tenant that
            // ever stores a memory. Acceptable for self-hosted and small tenant counts;
            // a large multi-tenant deployment must bound it (emit only above a
            // threshold, or pre-aggregate server-side) before enabling the staged
            // TenantQuotaNearLimit rule. Do not add more labels here.
            //
            // A zero limit would mean "unlimited" or "unconfigured" depending on the
            // caller; either way a ratio is undefined, so skip rather than divide.
            if limit > 0 {
                let ratio = new_used as f64 / limit as f64;
                crate::services::prometheus_exporter::get_exporter()
                    .set_tenant_quota_usage(&ctx.tenant_id, ratio);
            }

            tracing::info!(
                tenant_id = %ctx.tenant_id,
                new_used,
                limit,
                "quota: incremented memory_entries after successful store"
            );
        } else {
            self.record_audit(AuditEvent::new(
                ctx.tenant_id.clone(),
                "post_store_failure".to_string(),
                ctx.resource.clone(),
                AuditResult::Failure,
            ));
        }
    }

    fn post_search(&self, ctx: &HookContext, result: &HookResult) {
        if !result.success {
            self.record_audit(AuditEvent::new(
                ctx.tenant_id.clone(),
                "post_search_failure".to_string(),
                ctx.resource.clone(),
                AuditResult::Failure,
            ));
        }
    }
}

// ============================================================================
// Factory Functions
// ============================================================================

/// Create a complete enterprise hook set with all enterprise features
pub fn create_enterprise_hook_set() -> crate::hoops::enterprise::EnterpriseHookSet {
    let rbac_service = get_rbac_service();
    let quota_manager = Arc::new(QuotaManager::new());

    // Create audit callback: map the in-memory enterprise audit event onto the
    // persisted `memory_audit_events` schema and enqueue it on the async, best-effort
    // audit writer (non-blocking; drops + counts if the writer is not running, e.g. on
    // a SQLite dev backend). A `tracing` side-channel is kept for local debugging.
    let audit_callback: Arc<dyn Fn(AuditEvent) + Send + Sync> = Arc::new(|event| {
        tracing::debug!(?event, "governance audit event");

        let result = match event.result {
            AuditResult::Success => "success",
            AuditResult::Failure => "failure",
            AuditResult::Denied => "denied",
        };
        let action = event.action.clone();
        let resource = event.resource.clone();
        let metadata = serde_json::json!({
            "source": "governance_hook",
            "result": result,
            "action": action,
            "resource": resource,
            "occurred_at": event.timestamp,
            "extra": event.metadata,
        });

        let mut db_event =
            crate::db::audit::AuditEvent::new(format!("governance.{action}"), "governance")
                .tenant(event.tenant_id)
                .with_metadata(&metadata);
        if let Some(user_id) = event.user_id {
            db_event = db_event.actor(user_id);
        }
        if !resource.is_empty() {
            db_event = db_event.resource_id(resource);
        }
        crate::services::audit_writer::record_audit(db_event);
    });

    // Create auth hook
    let auth_hook = JwtAuthHookImpl::new(rbac_service.clone());
    let auth_hook = Arc::new(auth_hook);

    // Create RBAC hook with audit
    let rbac_hook = RbacHookImpl::new(rbac_service.clone()).with_audit_callback({
        let callback = audit_callback.clone();
        move |event| callback(event)
    });

    // Create quota hook
    let quota_hook = TenantQuotaHookImpl::new(quota_manager)
        .with_soft_limit(80)
        .with_audit_callback({
            let callback = audit_callback.clone();
            move |event| callback(event)
        });

    // Create governance hook
    let governance_hook =
        GovernanceHookImpl::new(Arc::new(rbac_hook), Arc::new(quota_hook), audit_callback);

    // Build and return enterprise hook set
    crate::hoops::enterprise::EnterpriseHookSet::new()
        .with_governance(governance_hook)
        .with_auth_arc(auth_hook)
        .with_rbac(RbacHookImpl::new(rbac_service))
}

/// Build server with enterprise hooks (for static injection)
pub fn build_server_with_enterprise_hooks() {
    ServerBuilder::new()
        .with_enterprise_hooks(create_enterprise_hook_set())
        .build();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hoops::enterprise::Operation;
    use crate::tenant::quota::ResourceQuota;

    #[test]
    fn test_auth_hook_validate_api_key() {
        let rbac = Arc::new(RbacService::new());
        let auth = JwtAuthHookImpl::new(rbac);

        auth.register_api_key("test_key_123".to_string(), "tenant1".to_string());

        assert_eq!(
            auth.validate_api_key("test_key_123"),
            Some("tenant1".to_string())
        );
        assert_eq!(auth.validate_api_key("invalid_key"), None);
    }

    #[test]
    fn test_license_tier_check() {
        let rbac = Arc::new(RbacService::new());
        let auth = JwtAuthHookImpl::new(rbac);

        auth.set_license_tier("tenant1", LicenseTier::Pro);

        assert!(auth.get_license_tier("tenant1") == LicenseTier::Pro);
        assert!(auth.get_license_tier("unknown") == LicenseTier::Free);
    }

    #[test]
    fn test_quota_check() {
        let quota_manager = Arc::new(QuotaManager::new());

        // Set a quota
        let mut quota = ResourceQuota::default();
        quota.api_calls_per_day = 100;
        quota_manager.set_quota("tenant1", quota);

        let hook = TenantQuotaHookImpl::new(quota_manager);

        // Should be allowed initially
        let result = hook.check_quota("tenant1", Resource::ApiCalls);
        assert!(result.allowed);
    }

    #[test]
    fn test_governance_hook_set_license_tier() {
        let rbac_service = Arc::new(RbacService::new());
        let quota_manager = Arc::new(QuotaManager::new());

        let audit_callback: Arc<dyn Fn(AuditEvent) + Send + Sync> = Arc::new(|_| {});

        let governance = GovernanceHookImpl::new(
            Arc::new(RbacHookImpl::new(rbac_service)),
            Arc::new(TenantQuotaHookImpl::new(quota_manager)),
            audit_callback,
        );

        governance.set_license_tier("tenant1", LicenseTier::Enterprise);

        assert!(governance.check_license("tenant1", LicenseTier::Enterprise));
        assert!(governance.check_license("tenant1", LicenseTier::Pro));
        // Enterprise tier includes Free tier (higher tier includes lower)
        assert!(governance.check_license("tenant1", LicenseTier::Free));

        // Test that Free tier tenant cannot access Enterprise features
        governance.set_license_tier("tenant2", LicenseTier::Free);
        assert!(!governance.check_license("tenant2", LicenseTier::Enterprise));
    }

    #[test]
    fn test_create_enterprise_hook_set() {
        let hooks = create_enterprise_hook_set();

        assert!(hooks.has_governance());
        assert!(hooks.has_auth());
        assert!(hooks.has_rbac());
    }

    // ------------------------------------------------------------------
    // Quota counting tests — verify that post_store actually increments
    // usage, that failures do NOT increment, and that Search is a no-op.
    // ------------------------------------------------------------------

    fn make_test_governance() -> (Arc<QuotaManager>, GovernanceHookImpl) {
        let rbac = Arc::new(RbacService::new());
        let quota_manager = Arc::new(QuotaManager::new());
        let quota_hook = Arc::new(TenantQuotaHookImpl::new(quota_manager.clone()));
        let audit: Arc<dyn Fn(AuditEvent) + Send + Sync> = Arc::new(|_| {});
        let governance =
            GovernanceHookImpl::new(Arc::new(RbacHookImpl::new(rbac)), quota_hook, audit);
        (quota_manager, governance)
    }

    #[test]
    fn post_store_success_increments_memory_entries_by_one() {
        let (quota_manager, governance) = make_test_governance();
        let ctx = HookContext::new(
            "tenant-a".to_string(),
            Operation::Store,
            "/test".to_string(),
        );

        governance.post_store(&ctx, &HookResult::success());

        let quota = quota_manager.ensure_quota("tenant-a");
        assert_eq!(
            quota.used.memory_entries, 1,
            "a successful Store must increment used.memory_entries by exactly 1"
        );
    }

    /// The usage-ratio gauge must actually move — `tenant_quota_usage_ratio` was
    /// registered but never written for the whole life of the project (backlog
    /// B-5), so it read a frozen 0 in `/metrics` while looking instrumented.
    #[test]
    fn post_store_publishes_the_usage_ratio_gauge() {
        let (quota_manager, governance) = make_test_governance();
        let tenant = "tenant-ratio";
        let ctx = HookContext::new(tenant.to_string(), Operation::Store, "/test".to_string());

        // Small limit so one store produces an exactly-predictable ratio.
        let mut seed = ResourceQuota::default();
        seed.memory_entries = 4;
        seed.used.memory_entries = 0;
        quota_manager.set_quota(tenant, seed);

        governance.post_store(&ctx, &HookResult::success());

        let families = crate::services::prometheus_exporter::get_exporter()
            .registry()
            .gather();
        let ratio = families
            .iter()
            .find(|f| f.get_name() == "tenant_quota_usage_ratio")
            .expect("tenant_quota_usage_ratio must be registered")
            .get_metric()
            .iter()
            .find(|m| {
                m.get_label()
                    .iter()
                    .any(|l| l.get_name() == "tenant" && l.get_value() == tenant)
            })
            .map(|m| m.get_gauge().get_value())
            .expect("post_store must emit a series for this tenant");

        assert!(
            (ratio - 0.25).abs() < f64::EPSILON,
            "1 used of limit 4 must publish 0.25, got {ratio}"
        );
    }

    /// A zero limit means "unlimited" or "unconfigured" depending on the caller, so
    /// a ratio is undefined. Emitting one would either divide by zero (inf) or
    /// invent a number an alert would then act on.
    #[test]
    fn post_store_emits_no_ratio_when_limit_is_zero() {
        let (quota_manager, governance) = make_test_governance();
        let tenant = "tenant-zero-limit";
        let ctx = HookContext::new(tenant.to_string(), Operation::Store, "/test".to_string());

        let mut seed = ResourceQuota::default();
        seed.memory_entries = 0;
        quota_manager.set_quota(tenant, seed);

        governance.post_store(&ctx, &HookResult::success());

        let families = crate::services::prometheus_exporter::get_exporter()
            .registry()
            .gather();
        let emitted = families
            .iter()
            .find(|f| f.get_name() == "tenant_quota_usage_ratio")
            .map(|f| {
                f.get_metric().iter().any(|m| {
                    m.get_label()
                        .iter()
                        .any(|l| l.get_name() == "tenant" && l.get_value() == tenant)
                })
            })
            .unwrap_or(false);

        assert!(
            !emitted,
            "a zero limit must not publish a ratio — inf or a fabricated value would \
             be acted on by the quota alert"
        );

        // The counter itself must still advance; only the derived ratio is skipped.
        assert_eq!(quota_manager.ensure_quota(tenant).used.memory_entries, 1);
    }

    #[test]
    fn post_store_failure_does_not_increment() {
        let (quota_manager, governance) = make_test_governance();
        let ctx = HookContext::new(
            "tenant-b".to_string(),
            Operation::Store,
            "/test".to_string(),
        );

        // Seed a quota so we can observe that it stays unchanged.
        let mut seed = ResourceQuota::default();
        seed.used.memory_entries = 5;
        quota_manager.set_quota("tenant-b", seed);

        governance.post_store(&ctx, &HookResult::failure("internal error"));

        let quota = quota_manager.ensure_quota("tenant-b");
        assert_eq!(
            quota.used.memory_entries, 5,
            "a failed Store must NOT increment used.memory_entries"
        );
    }

    #[test]
    fn search_does_not_increment_memory_entries() {
        let (quota_manager, governance) = make_test_governance();
        let ctx = HookContext::new(
            "tenant-c".to_string(),
            Operation::Search,
            "/test".to_string(),
        );

        governance.post_search(&ctx, &HookResult::success());

        // Search path never calls ensure_quota, so the tenant may not even have
        // a quota record.  If it does (from a prior ensure_quota call), the count
        // must still be zero.
        let used = quota_manager
            .get_quota("tenant-c")
            .map(|q| q.used.memory_entries)
            .unwrap_or(0);
        assert_eq!(
            used, 0,
            "a Search operation must NOT increment used.memory_entries"
        );
    }
}
