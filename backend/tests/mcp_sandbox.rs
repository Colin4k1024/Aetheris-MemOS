//! Integration tests for MCP sandbox isolation, exercised through the public
//! crate API (`backend::mcp::*`) so that a visibility regression is caught in
//! addition to a behavioural one.
//!
//! # Why there is no "register a native tool and run it" test here
//!
//! An earlier version of this file defined a `MockTool` implementing a
//! `SandboxedTool` trait and asserted that `SandboxProxy::execute_tool` ran it
//! successfully. That trait modelled **native** execution, which is the opposite
//! of what a sandbox is for — and `SandboxProxy::execute_tool` did in fact
//! ignore the capability policy and call the tool natively. So the test was
//! asserting that the fake sandbox worked.
//!
//! Backlog A-2 removed the trait and rewired `execute_tool` to route through
//! `WasmSandbox::execute_wasm`, so `register_tool` now takes wasm bytes rather
//! than a trait object. Per-tool execution behaviour is covered by unit tests in
//! `src/mcp/sandbox_proxy.rs`; what remains valuable at the integration level is
//! the capability-policy semantics and the registry surface, both below.

use backend::mcp::sandbox::{Capability, CapabilityPolicy};
use backend::mcp::sandbox_proxy::{ProxyError, SandboxProxy};
use serde_json::json;

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
    // A capability present in both sets must be denied: deny-by-default only
    // holds if an explicit deny cannot be overridden by an explicit allow.
    let allowed_set: std::collections::HashSet<Capability> =
        [Capability::NetworkAccess].into_iter().collect();
    let denied_set: std::collections::HashSet<Capability> =
        [Capability::NetworkAccess].into_iter().collect();

    let policy = CapabilityPolicy {
        allowed: allowed_set,
        denied: denied_set,
    };

    assert!(!policy.is_permitted(Capability::NetworkAccess));
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
fn test_sandbox_proxy_rejects_unknown_tool() {
    let proxy = SandboxProxy::new();
    let policy = CapabilityPolicy::allow([Capability::FilesystemRead]);
    let input = json!({ "key": "value" });

    let result = proxy.execute_tool("unknown_tool", input, &policy);
    assert!(matches!(result, Err(ProxyError::ToolNotFound(_))));
}

/// The registry starts empty in production — this is the property that makes
/// "the Plane B conduit is wired" different from "Plane B can run extension
/// tools". See `docs/memory/decisions.md`.
#[test]
fn test_sandbox_proxy_registry_starts_empty() {
    let proxy = SandboxProxy::new();

    assert!(proxy.registered_tools().is_empty());
    assert!(!proxy.is_registered("anything"));
}

#[test]
fn test_registered_tool_is_visible_in_registry() {
    let mut proxy = SandboxProxy::new();
    // `register_tool` now takes wasm bytes, not a trait object. These bytes are
    // deliberately not a valid module: this test asserts only registry
    // bookkeeping, and execution of real modules is covered by the unit tests
    // in `src/mcp/sandbox_proxy.rs`.
    proxy.register_tool("test_tool", vec![0x00, 0x61, 0x73, 0x6d]);

    assert!(proxy.is_registered("test_tool"));
    assert_eq!(proxy.registered_tools(), vec!["test_tool".to_string()]);
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
