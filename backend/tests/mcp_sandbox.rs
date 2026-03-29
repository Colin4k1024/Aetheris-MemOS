//! Integration tests for MCP sandbox isolation.

use backend::mcp::sandbox::{Capability, CapabilityPolicy, SandboxedTool, SandboxError};
use backend::mcp::sandbox_proxy::{ProxyError, SandboxProxy};
use serde_json::json;

/// A mock tool for testing.
struct MockTool;

impl SandboxedTool for MockTool {
    fn execute(&self, input: serde_json::Value) -> Result<serde_json::Value, SandboxError> {
        Ok(json!({ "output": input }))
    }
}

#[test]
fn test_capability_policy_denies_forbidden() {
    let policy = CapabilityPolicy::deny([
        Capability::NetworkAccess,
        Capability::FilesystemWrite,
        Capability::EnvVars,
    ]);

    // Denied capabilities should not be permitted
    assert!(!policy.is_permitted(Capability::NetworkAccess));
    assert!(!policy.is_permitted(Capability::FilesystemWrite));
    assert!(!policy.is_permitted(Capability::EnvVars));
}

#[test]
fn test_capability_policy_allows_permitted() {
    let policy = CapabilityPolicy::allow([Capability::FilesystemRead, Capability::FilesystemWrite]);

    // Explicitly allowed capabilities should be permitted
    assert!(policy.is_permitted(Capability::FilesystemRead));
    assert!(policy.is_permitted(Capability::FilesystemWrite));

    // Other capabilities should not be permitted
    assert!(!policy.is_permitted(Capability::NetworkAccess));
    assert!(!policy.is_permitted(Capability::EnvVars));
}

#[test]
fn test_deny_takes_precedence_over_allowed() {
    let mut policy = CapabilityPolicy::new();
    // First add NetworkAccess to allowed
    let allowed_set: std::collections::HashSet<Capability> =
        [Capability::NetworkAccess].into_iter().collect();
    let denied_set: std::collections::HashSet<Capability> =
        [Capability::NetworkAccess].into_iter().collect();

    policy = CapabilityPolicy {
        allowed: allowed_set,
        denied: denied_set,
    };

    // Denied should take precedence
    assert!(!policy.is_permitted(Capability::NetworkAccess));
}

#[test]
fn test_sandbox_proxy_executes_registered_tool() {
    let mut proxy = SandboxProxy::new();
    proxy.register_tool("test_tool".to_string(), MockTool);

    let policy = CapabilityPolicy::allow([Capability::FilesystemRead]);
    let input = json!({ "key": "value" });

    let result = proxy.execute_tool("test_tool", input, &policy);
    assert!(result.is_ok());
}

#[test]
fn test_sandbox_proxy_rejects_unknown_tool() {
    let proxy = SandboxProxy::new();
    let policy = CapabilityPolicy::allow([Capability::FilesystemRead]);
    let input = json!({ "key": "value" });

    let result = proxy.execute_tool("unknown_tool", input, &policy);
    assert!(matches!(result, Err(ProxyError::ToolNotFound(_))));
}

#[test]
fn test_empty_policy_denies_all() {
    let policy = CapabilityPolicy::new();

    assert!(!policy.is_permitted(Capability::NetworkAccess));
    assert!(!policy.is_permitted(Capability::FilesystemRead));
    assert!(!policy.is_permitted(Capability::FilesystemWrite));
    assert!(!policy.is_permitted(Capability::EnvVars));
}

#[test]
fn test_tool_execution_log_creation() {
    use backend::mcp::sandbox_proxy::ToolExecutionLog;

    let log = ToolExecutionLog::new(
        "exec123".to_string(),
        "test_tool".to_string(),
        vec!["NetworkAccess".to_string()],
        "allowed".to_string(),
    );

    assert_eq!(log.execution_id, "exec123");
    assert_eq!(log.tool_name, "test_tool");
    assert_eq!(log.capabilities_used, vec!["NetworkAccess".to_string()]);
    assert_eq!(log.rbac_decision, "allowed");
}
