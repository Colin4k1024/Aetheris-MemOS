//! Sandbox proxy for MCP Plane B (untrusted extension) tool execution.
//!
//! This is the Plane B executor from
//! `docs/adr/ADR-0004-mcp-sandbox-execution-model.md`. Registered extension
//! tools are WebAssembly modules; execution is delegated to [`WasmSandbox`],
//! which enforces the [`CapabilityPolicy`] (deny-by-default) and wasmtime
//! resource limits. The proxy adds a structured audit record ([`ToolExecutionLog`])
//! around each execution.
//!
//! Scope note (honest status): the production registry is **empty**. No
//! untrusted/third-party tools are onboarded yet (see ADR-0004 — the decision
//! to open a Plane B tool surface, its signing, and its capability-grant model
//! are deferred pending a tech-lead scope ruling). The conduit here is real and
//! tested; it simply has no tools to run in production, so every non-first-party
//! tool is rejected upstream in `routers/mcp.rs::call_tool`.

use crate::mcp::sandbox::{CapabilityPolicy, SandboxError, WasmSandbox};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tracing::{info, warn};

/// Error types for sandbox proxy operations.
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("sandbox execution failed: {0}")]
    ExecutionFailed(String),

    #[error("capability denied: {0}")]
    RbacDenied(String),
}

/// Represents a tool execution audit log entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolExecutionLog {
    /// Unique identifier for the tool execution.
    pub execution_id: String,
    /// Name of the tool being executed.
    pub tool_name: String,
    /// Capabilities granted to this execution.
    ///
    /// Derived from the [`CapabilityPolicy`] the caller was granted. This is the
    /// *authorized* capability set — per-host-function usage tracking is not yet
    /// implemented (there are no host functions until real extension tools land).
    pub capabilities_used: Vec<String>,
    /// RBAC decision (`allowed` or `denied`).
    pub rbac_decision: String,
    /// Timestamp of the execution.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ToolExecutionLog {
    /// Creates a new execution log entry.
    pub fn new(
        execution_id: String,
        tool_name: String,
        capabilities_used: Vec<String>,
        rbac_decision: String,
    ) -> Self {
        Self {
            execution_id,
            tool_name,
            capabilities_used,
            rbac_decision,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// A registered Plane B extension tool: an untrusted WebAssembly module.
struct ExtensionTool {
    /// Raw wasm module bytes (or wasm text, which wasmtime also accepts).
    wasm_bytes: Vec<u8>,
}

/// SandboxProxy is the Plane B executor: it holds a wasmtime sandbox and a
/// registry of untrusted extension tools, running each inside the sandbox
/// under a capability policy.
pub struct SandboxProxy {
    sandbox: WasmSandbox,
    /// Registered extension tools mapped by name. Empty in production today.
    tools: HashMap<String, ExtensionTool>,
}

impl Default for SandboxProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl SandboxProxy {
    /// Creates a new SandboxProxy with an empty tool registry.
    pub fn new() -> Self {
        Self {
            sandbox: WasmSandbox::new(),
            tools: HashMap::new(),
        }
    }

    /// Registers an untrusted extension tool by name with its wasm module bytes.
    pub fn register_tool(&mut self, name: impl Into<String>, wasm_bytes: Vec<u8>) {
        self.tools.insert(name.into(), ExtensionTool { wasm_bytes });
    }

    /// Returns whether `tool_name` is a registered Plane B extension tool.
    ///
    /// Used by the plane classifier in `routers/mcp.rs` to distinguish
    /// extension tools from unknown tools.
    pub fn is_registered(&self, tool_name: &str) -> bool {
        self.tools.contains_key(tool_name)
    }

    /// Executes a registered extension tool inside the wasmtime sandbox.
    ///
    /// The `policy` is genuinely enforced: [`WasmSandbox::execute_wasm`] rejects
    /// the call (before running the module) when a required capability is not
    /// permitted. Returns the tool output together with a [`ToolExecutionLog`]
    /// the caller can persist to the audit trail.
    pub fn execute_tool(
        &self,
        tool_name: &str,
        input: JsonValue,
        policy: &CapabilityPolicy,
    ) -> Result<(JsonValue, ToolExecutionLog), ProxyError> {
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| ProxyError::ToolNotFound(tool_name.to_string()))?;

        let execution_id = ulid::Ulid::new().to_string();
        // The capability set the policy authorizes for this execution.
        let capabilities_used: Vec<String> = policy.allowed.iter().map(|c| c.to_string()).collect();

        match self.sandbox.execute_wasm(&tool.wasm_bytes, input, policy) {
            Ok(output) => {
                info!(
                    execution_id = %execution_id,
                    tool_name = %tool_name,
                    rbac_decision = "allowed",
                    "Plane B tool execution completed"
                );
                let log = ToolExecutionLog::new(
                    execution_id,
                    tool_name.to_string(),
                    capabilities_used,
                    "allowed".to_string(),
                );
                Ok((output, log))
            }
            Err(err) => {
                warn!(
                    execution_id = %execution_id,
                    tool_name = %tool_name,
                    rbac_decision = "denied",
                    error = %err,
                    "Plane B tool execution denied or failed"
                );
                // A capability denial is an authorization outcome, not an
                // internal failure — surface it distinctly.
                match err {
                    SandboxError::CapabilityDenied(cap) => {
                        Err(ProxyError::RbacDenied(cap.to_string()))
                    }
                    other => Err(ProxyError::ExecutionFailed(other.to_string())),
                }
            }
        }
    }

    /// Returns a list of all registered tool names.
    pub fn registered_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::sandbox::Capability;
    use serde_json::json;

    /// Minimal valid module: exports `memory` / `alloc` / `execute`;
    /// `execute` returns a constant, null-terminated JSON string.
    const ECHO_MODULE_WAT: &str = r#"
        (module
          (memory (export "memory") 1)
          (global $bump (mut i32) (i32.const 1024))
          (func (export "alloc") (param $size i32) (result i32)
            (local $p i32)
            (local.set $p (global.get $bump))
            (global.set $bump (i32.add (global.get $bump) (local.get $size)))
            (local.get $p))
          (data (i32.const 64) "{\"echoed\":true}\00")
          (func (export "execute") (param $ptr i32) (param $len i32) (result i32)
            (i32.const 64)))
    "#;

    fn allow_all() -> CapabilityPolicy {
        CapabilityPolicy::allow([
            Capability::NetworkAccess,
            Capability::FilesystemRead,
            Capability::FilesystemWrite,
            Capability::EnvVars,
        ])
    }

    #[test]
    fn is_registered_reflects_registry() {
        let mut proxy = SandboxProxy::new();
        assert!(!proxy.is_registered("ext_tool"));
        proxy.register_tool("ext_tool", ECHO_MODULE_WAT.as_bytes().to_vec());
        assert!(proxy.is_registered("ext_tool"));
        assert!(!proxy.is_registered("other"));
    }

    #[test]
    fn execute_tool_not_found() {
        let proxy = SandboxProxy::new();
        let result = proxy.execute_tool("nonexistent", json!({}), &allow_all());
        assert!(matches!(result, Err(ProxyError::ToolNotFound(_))));
    }

    #[test]
    fn execute_tool_runs_registered_wasm_through_sandbox() {
        // Proves Plane B routing is real: a registered module executes inside
        // wasmtime and the audit log records the granted capabilities.
        let mut proxy = SandboxProxy::new();
        proxy.register_tool("ext_echo", ECHO_MODULE_WAT.as_bytes().to_vec());

        let (output, log) = proxy
            .execute_tool("ext_echo", json!({ "x": 1 }), &allow_all())
            .expect("registered module should execute");

        assert_eq!(output, json!({ "echoed": true }));
        assert_eq!(log.tool_name, "ext_echo");
        assert_eq!(log.rbac_decision, "allowed");
        assert!(!log.capabilities_used.is_empty());
    }

    #[test]
    fn execute_tool_denied_by_empty_policy() {
        // Proves the policy is genuinely consumed: deny-by-default blocks a
        // registered tool when no capability is granted.
        let mut proxy = SandboxProxy::new();
        proxy.register_tool("ext_echo", ECHO_MODULE_WAT.as_bytes().to_vec());

        let result = proxy.execute_tool("ext_echo", json!({}), &CapabilityPolicy::new());
        assert!(matches!(result, Err(ProxyError::RbacDenied(_))));
    }

    #[test]
    fn registered_tools_lists_names() {
        let mut proxy = SandboxProxy::new();
        proxy.register_tool("tool1", ECHO_MODULE_WAT.as_bytes().to_vec());
        proxy.register_tool("tool2", ECHO_MODULE_WAT.as_bytes().to_vec());

        let mut tools = proxy.registered_tools();
        tools.sort();
        assert_eq!(tools, vec!["tool1".to_string(), "tool2".to_string()]);
    }
}
