//! Enterprise Hooks
//!
//! Static injection hooks for enterprise features like governance, billing, audit, and RBAC.
//! This module provides a pluggable interface for enterprise-specific functionality that can be
//! compiled in via static injection at build time.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// License tier levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicenseTier {
    Free,
    Starter,
    Pro,
    Enterprise,
}

impl Default for LicenseTier {
    fn default() -> Self {
        LicenseTier::Free
    }
}

/// Resource types for quota checking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    ApiCalls,
    StorageMb,
    CognitiveUnits,
    MemoryOperations,
    VectorQueries,
}

/// Quota check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaResult {
    pub allowed: bool,
    pub current: u64,
    pub limit: u64,
    pub overage: i64,
}

/// Audit event for tracking actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub action: String,
    pub resource: String,
    pub result: AuditResult,
    pub timestamp: i64,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

impl AuditEvent {
    pub fn new(
        tenant_id: String,
        action: String,
        resource: String,
        result: AuditResult,
    ) -> Self {
        Self {
            tenant_id,
            user_id: None,
            action,
            resource,
            result,
            timestamp: chrono::Utc::now().timestamp(),
            metadata: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditResult {
    Success,
    Failure,
    Denied,
}

// ============================================================================
// Hook Types for GovernanceHook pre/post operations
// ============================================================================

/// Operation types for hooks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Store,
    Search,
    Update,
    Delete,
}

impl Operation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Operation::Store => "store",
            Operation::Search => "search",
            Operation::Update => "update",
            Operation::Delete => "delete",
        }
    }
}

/// Hook context - carries request information through the hook chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    /// Tenant identifier
    pub tenant_id: String,
    /// User identifier (if authenticated)
    pub user_id: Option<String>,
    /// Operation being performed
    pub operation: Operation,
    /// Resource being accessed
    pub resource: String,
    /// Additional parameters
    pub params: std::collections::HashMap<String, String>,
}

impl HookContext {
    pub fn new(tenant_id: String, operation: Operation, resource: String) -> Self {
        Self {
            tenant_id,
            user_id: None,
            operation,
            resource,
            params: std::collections::HashMap::new(),
        }
    }

    pub fn with_user(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }
}

/// Decision returned by pre-hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "reason")]
pub enum HookDecision {
    /// Allow the operation to proceed
    Allow,
    /// Deny the operation with a reason
    Deny(String),
    /// Skip this hook (for hook chains)
    Skip,
}

impl HookDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, HookDecision::Allow)
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, HookDecision::Deny(_))
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            HookDecision::Deny(reason) => Some(reason),
            _ => None,
        }
    }
}

impl Default for HookDecision {
    fn default() -> Self {
        HookDecision::Allow
    }
}

/// Result from post-hook execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    /// Whether the operation succeeded
    pub success: bool,
    /// Optional result data
    pub data: Option<serde_json::Value>,
    /// Error message if failed
    pub error: Option<String>,
}

impl HookResult {
    pub fn success() -> Self {
        Self {
            success: true,
            data: None,
            error: None,
        }
    }

    pub fn success_with_data(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

/// Error from hook execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookError {
    /// Error code
    pub code: String,
    /// Error message
    pub message: String,
}

impl HookError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<std::io::Error> for HookError {
    fn from(err: std::io::Error) -> Self {
        Self {
            code: "IO_ERROR".into(),
            message: err.to_string(),
        }
    }
}

/// Governance hook trait for enterprise features
/// Implement this trait to provide custom governance, licensing, and quota management
pub trait GovernanceHook: Send + Sync {
    // =========================================================================
    // License & Feature Checks
    // =========================================================================

    /// Check if a license tier is valid and active
    fn check_license(&self, tenant_id: &str, tier: LicenseTier) -> bool;

    /// Check if a feature is enabled for a tenant
    fn check_feature(&self, tenant_id: &str, feature: &str) -> bool;

    /// Check quota for a specific resource
    fn check_quota(&self, tenant_id: &str, resource: Resource) -> QuotaResult;

    /// Record an audit event
    fn record_audit(&self, event: AuditEvent);

    /// Get current usage for a tenant
    fn get_usage(&self, tenant_id: &str) -> Option<UsageSnapshot>;

    // =========================================================================
    // Pre-hooks (return decision to allow/deny/skip)
    // =========================================================================

    /// Pre-hook for store operation
    fn pre_store(&self, _ctx: &HookContext) -> HookDecision {
        HookDecision::Allow
    }

    /// Pre-hook for search operation
    fn pre_search(&self, _ctx: &HookContext) -> HookDecision {
        HookDecision::Allow
    }

    /// Pre-hook for update operation
    fn pre_update(&self, _ctx: &HookContext) -> HookDecision {
        HookDecision::Allow
    }

    /// Pre-hook for delete operation
    fn pre_delete(&self, _ctx: &HookContext) -> HookDecision {
        HookDecision::Allow
    }

    // =========================================================================
    // Post-hooks (called after operation completes)
    // =========================================================================

    /// Post-hook for store operation
    fn post_store(&self, _ctx: &HookContext, _result: &HookResult) {}

    /// Post-hook for search operation
    fn post_search(&self, _ctx: &HookContext, _result: &HookResult) {}

    /// Post-hook for update operation
    fn post_update(&self, _ctx: &HookContext, _result: &HookResult) {}

    /// Post-hook for delete operation
    fn post_delete(&self, _ctx: &HookContext, _result: &HookResult) {}

    // =========================================================================
    // Error handling & lifecycle
    // =========================================================================

    /// Called when an operation fails with an error
    fn on_error(&self, _ctx: &HookContext, _error: &HookError) {}

    /// Called when the hook is being disconnected/unloaded
    fn on_disconnect(&self) {}

    // =========================================================================
    // Metrics (optional, for sinks that want to collect)
    // =========================================================================

    /// Record metrics event (optional implementation)
    /// Default: no-op
    fn record_metrics(&self, _op_type: &str, _latency_ms: u64, _outcome: &str) {}
}

/// Usage snapshot for a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub tenant_id: String,
    pub tier: LicenseTier,
    pub api_calls: u64,
    pub storage_mb: u64,
    pub cognitive_units: u64,
    pub memory_operations: u64,
    pub vector_queries: u64,
}

// ============================================================================
// HookSet - Multiple hook registration with short-circuit logic
// ============================================================================

/// Short-circuit strategy for hook execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShortCircuit {
    /// All hooks must allow (AND logic)
    #[default]
    All,
    /// Any hook denying stops (OR logic)
    Any,
}

/// Hook execution policy
#[derive(Debug, Clone)]
pub struct HookExecutionPolicy {
    /// Timeout in milliseconds for each hook
    pub timeout_ms: u64,
    /// If true, one failure stops all subsequent hooks
    pub fail_fast: bool,
    /// Short-circuit strategy
    pub short_circuit: ShortCircuit,
}

impl Default for HookExecutionPolicy {
    fn default() -> Self {
        Self {
            timeout_ms: 5000, // 5 seconds default
            fail_fast: true,
            short_circuit: ShortCircuit::All,
        }
    }
}

/// HookSet - manages multiple governance hooks with execution policy
#[derive(Clone)]
pub struct HookSet {
    hooks: Vec<Arc<dyn GovernanceHook>>,
    policy: HookExecutionPolicy,
}

impl Default for HookSet {
    fn default() -> Self {
        Self::new()
    }
}

impl HookSet {
    /// Create a new empty hook set with default policy
    pub fn new() -> Self {
        Self {
            hooks: Vec::new(),
            policy: HookExecutionPolicy::default(),
        }
    }

    /// Create with custom policy
    pub fn with_policy(policy: HookExecutionPolicy) -> Self {
        Self {
            hooks: Vec::new(),
            policy,
        }
    }

    /// Register a new hook
    pub fn register(&mut self, hook: Arc<dyn GovernanceHook>) {
        self.hooks.push(hook);
    }

    /// Register a hook (builder pattern)
    #[must_use]
    pub fn with_hook(mut self, hook: Arc<dyn GovernanceHook>) -> Self {
        self.hooks.push(hook);
        self
    }

    /// Set execution policy
    #[must_use]
    pub fn with_policy_mut(mut self, policy: HookExecutionPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Get number of registered hooks
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }

    /// Execute pre-hook for store operation
    pub fn pre_store(&self, ctx: &HookContext) -> HookDecision {
        self.execute_pre_hook(|hook| hook.pre_store(ctx))
    }

    /// Execute pre-hook for search operation
    pub fn pre_search(&self, ctx: &HookContext) -> HookDecision {
        self.execute_pre_hook(|hook| hook.pre_search(ctx))
    }

    /// Execute pre-hook for update operation
    pub fn pre_update(&self, ctx: &HookContext) -> HookDecision {
        self.execute_pre_hook(|hook| hook.pre_update(ctx))
    }

    /// Execute pre-hook for delete operation
    pub fn pre_delete(&self, ctx: &HookContext) -> HookDecision {
        self.execute_pre_hook(|hook| hook.pre_delete(ctx))
    }

    /// Execute post-hook for store operation
    pub fn post_store(&self, ctx: &HookContext, result: &HookResult) {
        self.execute_post_hook(|hook| hook.post_store(ctx, result));
    }

    /// Execute post-hook for search operation
    pub fn post_search(&self, ctx: &HookContext, result: &HookResult) {
        self.execute_post_hook(|hook| hook.post_search(ctx, result));
    }

    /// Execute post-hook for update operation
    pub fn post_update(&self, ctx: &HookContext, result: &HookResult) {
        self.execute_post_hook(|hook| hook.post_update(ctx, result));
    }

    /// Execute post-hook for delete operation
    pub fn post_delete(&self, ctx: &HookContext, result: &HookResult) {
        self.execute_post_hook(|hook| hook.post_delete(ctx, result));
    }

    /// Execute error handler
    pub fn on_error(&self, ctx: &HookContext, error: &HookError) {
        for hook in &self.hooks {
            hook.on_error(ctx, error);
        }
    }

    fn execute_pre_hook<F>(&self, mut f: F) -> HookDecision
    where
        F: FnMut(&dyn GovernanceHook) -> HookDecision,
    {
        if self.hooks.is_empty() {
            return HookDecision::Allow;
        }

        match self.policy.short_circuit {
            ShortCircuit::All => {
                // All must allow
                for hook in &self.hooks {
                    let decision = f(hook.as_ref());
                    if !decision.is_allowed() {
                        return decision;
                    }
                }
                HookDecision::Allow
            }
            ShortCircuit::Any => {
                // Any deny stops
                for hook in &self.hooks {
                    let decision = f(hook.as_ref());
                    if decision.is_denied() {
                        return decision;
                    }
                }
                HookDecision::Allow
            }
        }
    }

    fn execute_post_hook<F>(&self, mut f: F)
    where
        F: FnMut(&dyn GovernanceHook) -> (),
    {
        for hook in &self.hooks {
            f(hook.as_ref());
        }
    }
}

/// No-op implementation of GovernanceHook
pub struct NoopHookSet;

impl GovernanceHook for NoopHookSet {
    fn check_license(&self, _tenant_id: &str, _tier: LicenseTier) -> bool {
        true
    }

    fn check_feature(&self, _tenant_id: &str, _feature: &str) -> bool {
        true
    }

    fn check_quota(&self, _tenant_id: &str, _resource: Resource) -> QuotaResult {
        QuotaResult {
            allowed: true,
            current: 0,
            limit: u64::MAX,
            overage: 0,
        }
    }

    fn record_audit(&self, _event: AuditEvent) {}

    fn get_usage(&self, _tenant_id: &str) -> Option<UsageSnapshot> {
        None
    }
}

impl Default for NoopHookSet {
    fn default() -> Self {
        Self
    }
}

/// Authentication hook trait
pub trait AuthHook: Send + Sync {
    /// Validate API key
    fn validate_api_key(&self, api_key: &str) -> Option<String>;

    /// Get tenant ID from token
    fn get_tenant_from_token(&self, token: &str) -> Option<String>;
}

/// RBAC hook for permission checking
pub trait RbacHook: Send + Sync {
    /// Check if a user has permission for an action
    fn check_permission(
        &self,
        tenant_id: &str,
        user_id: &str,
        resource: &str,
        action: &str,
    ) -> bool;

    /// Get user roles
    fn get_roles(&self, tenant_id: &str, user_id: &str) -> Vec<String>;
}

/// Enterprise hook set - collection of all enterprise hooks
#[derive(Clone)]
pub struct EnterpriseHookSet {
    governance: Option<Arc<dyn GovernanceHook>>,
    auth: Option<Arc<dyn AuthHook>>,
    rbac: Option<Arc<dyn RbacHook>>,
}

impl Default for EnterpriseHookSet {
    fn default() -> Self {
        Self::new()
    }
}

impl EnterpriseHookSet {
    /// Create a new empty hook set
    pub fn new() -> Self {
        Self {
            governance: None,
            auth: None,
            rbac: None,
        }
    }

    /// Set governance hook
    #[must_use]
    pub fn with_governance<H: GovernanceHook + 'static>(mut self, hook: H) -> Self {
        self.governance = Some(Arc::new(hook));
        self
    }

    /// Set governance hook from Arc
    #[must_use]
    pub fn with_governance_arc(mut self, hook: Arc<dyn GovernanceHook>) -> Self {
        self.governance = Some(hook);
        self
    }

    /// Set authentication hook
    #[must_use]
    pub fn with_auth<H: AuthHook + 'static>(mut self, hook: H) -> Self {
        self.auth = Some(Arc::new(hook));
        self
    }

    /// Set authentication hook from Arc
    #[must_use]
    pub fn with_auth_arc(mut self, hook: Arc<dyn AuthHook>) -> Self {
        self.auth = Some(hook);
        self
    }

    /// Set RBAC hook
    #[must_use]
    pub fn with_rbac<H: RbacHook + 'static>(mut self, hook: H) -> Self {
        self.rbac = Some(Arc::new(hook));
        self
    }

    /// Set RBAC hook from Arc
    #[must_use]
    pub fn with_rbac_arc(mut self, hook: Arc<dyn RbacHook>) -> Self {
        self.rbac = Some(hook);
        self
    }

    /// Check if governance hook is available
    pub fn has_governance(&self) -> bool {
        self.governance.is_some()
    }

    /// Get governance hook reference
    pub fn governance(&self) -> Option<&Arc<dyn GovernanceHook>> {
        self.governance.as_ref()
    }

    /// Check if auth hook is available
    pub fn has_auth(&self) -> bool {
        self.auth.is_some()
    }

    /// Get auth hook reference
    pub fn auth(&self) -> Option<&Arc<dyn AuthHook>> {
        self.auth.as_ref()
    }

    /// Check if RBAC hook is available
    pub fn has_rbac(&self) -> bool {
        self.rbac.is_some()
    }

    /// Get RBAC hook reference
    pub fn rbac(&self) -> Option<&Arc<dyn RbacHook>> {
        self.rbac.as_ref()
    }

    /// Check license (no-op if no governance hook)
    pub fn check_license(&self, tenant_id: &str, tier: LicenseTier) -> bool {
        self.governance
            .as_ref()
            .map(|h| h.check_license(tenant_id, tier))
            .unwrap_or(true) // Default to true if no hook configured
    }

    /// Check feature (no-op if no governance hook)
    pub fn check_feature(&self, tenant_id: &str, feature: &str) -> bool {
        self.governance
            .as_ref()
            .map(|h| h.check_feature(tenant_id, feature))
            .unwrap_or(true)
    }

    /// Check quota (returns unlimited if no governance hook)
    pub fn check_quota(&self, tenant_id: &str, resource: Resource) -> QuotaResult {
        self.governance
            .as_ref()
            .map(|h| h.check_quota(tenant_id, resource))
            .unwrap_or(QuotaResult {
                allowed: true,
                current: 0,
                limit: u64::MAX,
                overage: 0,
            })
    }

    /// Record audit event (no-op if no governance hook)
    pub fn record_audit(&self, event: AuditEvent) {
        if let Some(h) = &self.governance {
            h.record_audit(event);
        }
    }

    /// Get usage (returns None if no governance hook)
    pub fn get_usage(&self, tenant_id: &str) -> Option<UsageSnapshot> {
        self.governance.as_ref()?.get_usage(tenant_id)
    }
}

/// Global enterprise hook set
static ENTERPRISE_HOOKS: std::sync::OnceLock<EnterpriseHookSet> = std::sync::OnceLock::new();

/// Initialize global enterprise hooks
pub fn init_enterprise_hooks(hooks: EnterpriseHookSet) {
    let _ = ENTERPRISE_HOOKS.set(hooks);
}

/// Get global enterprise hooks
pub fn get_enterprise_hooks() -> &'static EnterpriseHookSet {
    ENTERPRISE_HOOKS.get_or_init(EnterpriseHookSet::new)
}

/// Server builder extension for enterprise hooks
pub struct ServerBuilder {
    enterprise_hooks: EnterpriseHookSet,
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerBuilder {
    pub fn new() -> Self {
        Self {
            enterprise_hooks: EnterpriseHookSet::new(),
        }
    }

    /// Add enterprise hooks to the server
    #[must_use]
    pub fn with_enterprise_hooks(mut self, hooks: EnterpriseHookSet) -> Self {
        self.enterprise_hooks = hooks;
        self
    }

    /// Add governance hook
    #[must_use]
    pub fn with_governance<H: GovernanceHook + 'static>(mut self, hook: H) -> Self {
        self.enterprise_hooks = self.enterprise_hooks.with_governance(hook);
        self
    }

    /// Add auth hook
    #[must_use]
    pub fn with_auth<H: AuthHook + 'static>(mut self, hook: H) -> Self {
        self.enterprise_hooks = self.enterprise_hooks.with_auth(hook);
        self
    }

    /// Add RBAC hook
    #[must_use]
    pub fn with_rbac<H: RbacHook + 'static>(mut self, hook: H) -> Self {
        self.enterprise_hooks = self.enterprise_hooks.with_rbac(hook);
        self
    }

    /// Build and initialize the server with enterprise hooks
    pub fn build(self) {
        init_enterprise_hooks(self.enterprise_hooks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_tier_default() {
        assert_eq!(LicenseTier::default(), LicenseTier::Free);
    }

    #[test]
    fn test_enterprise_hook_set_empty() {
        let hooks = EnterpriseHookSet::new();
        assert!(!hooks.has_governance());
        assert!(!hooks.has_auth());
        assert!(!hooks.has_rbac());
    }

    #[test]
    fn test_enterprise_hook_set_with_governance() {
        struct MockGovernance;

        impl GovernanceHook for MockGovernance {
            fn check_license(&self, _tenant_id: &str, tier: LicenseTier) -> bool {
                tier == LicenseTier::Pro || tier == LicenseTier::Enterprise
            }

            fn check_feature(&self, _tenant_id: &str, _feature: &str) -> bool {
                true
            }

            fn check_quota(&self, _tenant_id: &str, _resource: Resource) -> QuotaResult {
                QuotaResult {
                    allowed: true,
                    current: 100,
                    limit: 1000,
                    overage: 0,
                }
            }

            fn record_audit(&self, _event: AuditEvent) {}

            fn get_usage(&self, _tenant_id: &str) -> Option<UsageSnapshot> {
                None
            }
        }

        let hooks = EnterpriseHookSet::new().with_governance(MockGovernance);
        assert!(hooks.has_governance());
        assert!(hooks.check_license("tenant1", LicenseTier::Free));
        assert!(hooks.check_license("tenant1", LicenseTier::Pro));
    }

    #[test]
    fn test_quota_result_defaults() {
        let hooks = EnterpriseHookSet::new();
        let result = hooks.check_quota("tenant1", Resource::ApiCalls);
        assert!(result.allowed);
        assert_eq!(result.limit, u64::MAX);
    }

    #[test]
    fn test_server_builder() {
        let builder = ServerBuilder::new();
        // Just verify it compiles and can be built
        builder.build();
    }
}
