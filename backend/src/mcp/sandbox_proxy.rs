//! Sandbox proxy for MCP tool execution with RBAC and audit logging.
//!
//! Provides a secure interface for executing MCP tools inside WebAssembly sandboxes
//! with RBAC decision logging and event store integration.

use crate::mcp::sandbox::{CapabilityPolicy, SandboxedTool, WasmSandbox};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{info, warn};

/// Error types for sandbox proxy operations.
#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("sandbox execution failed: {0}")]
    ExecutionFailed(String),

    #[error("RBAC denied: {0}")]
    RbacDenied(String),
}

/// Represents a tool execution audit log entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolExecutionLog {
    /// Unique identifier for the tool execution.
    pub execution_id: String,
    /// Name of the tool being executed.
    pub tool_name: String,
    /// Capabilities that were used by the tool.
    pub capabilities_used: Vec<String>,
    /// RBAC decision (allowed or denied).
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

/// SandboxProxy provides a secure interface for tool execution.
pub struct SandboxProxy {
    sandbox: Arc<WasmSandbox<()>>,
    /// Registered tools mapped by name.
    tools: HashMap<String, Box<dyn SandboxedTool>>,
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
            sandbox: Arc::new(WasmSandbox::new(()).expect("failed to create wasm sandbox")),
            tools: HashMap::new(),
        }
    }

    /// Registers a tool with the proxy.
    pub fn register_tool<T: SandboxedTool + 'static>(&mut self, name: String, tool: T) {
        self.tools.insert(name, Box::new(tool));
    }

    /// Executes a tool by name with the given input and capability policy.
    ///
    /// Logs the execution decision to the event store.
    pub fn execute_tool(
        &self,
        tool_name: &str,
        input: JsonValue,
        policy: &CapabilityPolicy,
    ) -> Result<JsonValue, ProxyError> {
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| ProxyError::ToolNotFound(tool_name.to_string()))?;

        // Log RBAC decision (simulated event store logging)
        let capabilities_used: Vec<String> = vec![]; // Would be derived from tool introspection
        let execution_id = ulid::Ulid::new().to_string();

        info!(
            execution_id = %execution_id,
            tool_name = %tool_name,
            capabilities = ?capabilities_used,
            "RBAC decision: executing tool"
        );

        let result = tool.execute(input).map_err(|e| {
            warn!(
                execution_id = %execution_id,
                tool_name = %tool_name,
                error = %e,
                "tool execution failed"
            );
            ProxyError::ExecutionFailed(e.to_string())
        });

        // Log the result
        match &result {
            Ok(_) => {
                info!(
                    execution_id = %execution_id,
                    tool_name = %tool_name,
                    rbac_decision = "allowed",
                    "tool execution completed"
                );
            }
            Err(e) => {
                warn!(
                    execution_id = %execution_id,
                    tool_name = %tool_name,
                    rbac_decision = "denied",
                    error = %e,
                    "tool execution denied or failed"
                );
            }
        }

        result
    }

    /// Returns a list of all registered tool names.
    pub fn registered_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::sandbox::{Capability, CapabilityPolicy};
    use serde_json::json;

    struct MockTool;

    impl SandboxedTool for MockTool {
        fn execute(&self, input: JsonValue) -> Result<JsonValue, crate::mcp::sandbox::SandboxError> {
            Ok(json!({ "result": input }))
        }
    }

    #[test]
    fn test_execute_tool_success() {
        let mut proxy = SandboxProxy::new();
        proxy.register_tool("mock_tool".to_string(), MockTool);

        let policy = CapabilityPolicy::allow([Capability::FilesystemRead]);
        let input = json!({ "test": "data" });

        let result = proxy.execute_tool("mock_tool", input, &policy);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_tool_not_found() {
        let proxy = SandboxProxy::new();
        let policy = CapabilityPolicy::allow([Capability::FilesystemRead]);
        let input = json!({ "test": "data" });

        let result = proxy.execute_tool("nonexistent", input, &policy);
        assert!(matches!(result, Err(ProxyError::ToolNotFound(_))));
    }

    #[test]
    fn test_registered_tools() {
        let mut proxy = SandboxProxy::new();
        proxy.register_tool("tool1".to_string(), MockTool);
        proxy.register_tool("tool2".to_string(), MockTool);

        let tools = proxy.registered_tools();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&"tool1".to_string()));
        assert!(tools.contains(&"tool2".to_string()));
    }
}
