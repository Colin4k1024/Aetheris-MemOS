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
use crate::services::rbac::{Permission, RbacService, Role};
use crate::tenant::context::QuotaResource;
use crate::tenant::quota::{QuotaManager, ResourceQuota};

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

    /// Validate JWT token and extract tenant/user info
    fn validate_jwt(&self, token: &str) -> Option<AuthContext> {
        // Use existing JWT validation from hoops::jwt
        let claims = crate::hoops::jwt::decode_token_claims(token)?;

        // For now, use the uid as user_id and derive tenant_id
        // In production, you'd decode the full JWT with tenant claims
        let user_id = claims.uid.clone();

        // Try to get tenant from user_id format (e.g., "tenantId_userId")
        let (tenant_id, actual_user_id) = if user_id.contains('_') {
            let parts: Vec<&str> = user_id.splitn(2, '_').collect();
            if parts.len() == 2 {
                (parts[0].to_string(), Some(parts[1].to_string()))
            } else {
                (user_id.clone(), Some(user_id))
            }
        } else {
            (user_id.clone(), Some(user_id))
        };

        let tier = self.get_license_tier(&tenant_id);

        Some(
            AuthContext::new(tenant_id)
                .with_user(actual_user_id.unwrap_or_default())
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

    /// Synchronous permission check (for use in sync contexts)
    pub fn blocking_has_permission(&self, tenant_id: &str, user_id: &str, permission: Permission) -> bool {
        // Try to acquire read lock and check
        if let Ok(roles) = self.rbac.roles().try_read() {
            if let Some(tenant_roles) = roles.get(tenant_id) {
                if let Some(role) = tenant_roles.get(user_id) {
                    return role.has_permission(&permission);
                }
            }
        }
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
        // Map action to permission
        let permission = match action.to_lowercase().as_str() {
            "read" | "get" | "list" => Permission::Read,
            "write" | "create" | "store" | "update" => Permission::Write,
            "delete" | "remove" => Permission::Delete,
            "manage" => Permission::Manage,
            "manage_memory" => Permission::ManageMemory,
            "manage_agents" => Permission::ManageAgents,
            "manage_tenant" => Permission::ManageTenant,
            "manage_billing" => Permission::ManageBilling,
            "delete_tenant" => Permission::DeleteTenant,
            _ => Permission::Read, // Default to read for unknown actions
        };

        let result = self.blocking_has_permission(tenant_id, user_id, permission);

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

        let allowed = self.quota_manager.can_perform(tenant_id, &quota_resource);

        // Get quota info
        let quota = self.quota_manager.get_quota(tenant_id);

        if let Some(q) = quota {
            let remaining = q.remaining(&quota_resource);
            let limit = match quota_resource {
                QuotaResource::StorageMB => q.storage_mb,
                QuotaResource::ApiCallsPerDay => q.api_calls_per_day,
                QuotaResource::ConcurrentSessions => q.concurrent_sessions as u64,
                QuotaResource::MemoryEntries => q.memory_entries,
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
        } else {
            // No quota configured - allow
            QuotaResult {
                allowed: true,
                current: 0,
                limit: u64::MAX,
                overage: 0,
            }
        }
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
        if !result.success {
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
    let rbac_service = Arc::new(RbacService::new());
    let quota_manager = Arc::new(QuotaManager::new());

    // Create audit callback
    let audit_callback: Arc<dyn Fn(AuditEvent) + Send + Sync> = Arc::new(|event| {
        tracing::info!("Audit event: {:?}", event);
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
    let governance_hook = GovernanceHookImpl::new(
        Arc::new(rbac_hook),
        Arc::new(quota_hook),
        audit_callback,
    );

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
}
