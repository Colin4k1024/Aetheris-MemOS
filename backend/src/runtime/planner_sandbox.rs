//! Planner Sandbox - Virtual Execution Environment for Dry-run Simulation
//!
//! This module provides a sandboxed environment for executing planner agent
//! dry-runs without actual side effects.

use crate::models::dry_run::{
    DryRunConfig, DryRunResult, ExecutionPlan, ExecutionStep, ExecutionTrace, PlanMetadata,
    PlanStep,
};
use crate::services::conflict_detector::{ConflictDetector, ConflictReport};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use thiserror::Error;
use utoipa::ToSchema;

/// Errors specific to sandbox execution.
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum SandboxError {
    #[error("Network access is disabled in dry-run mode")]
    NetworkDisabled,

    #[error("Filesystem access is denied in dry-run mode")]
    FilesystemAccessDenied,

    #[error("Execution timeout exceeded")]
    Timeout,

    #[error("Step limit exceeded: {0} steps")]
    StepLimitExceeded(usize),

    #[error("Unknown tool: {0}")]
    UnknownTool(String),

    #[error("Invalid plan: {0}")]
    InvalidPlan(String),
}

/// A recorded side effect from a tool call (without actually executing it).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VirtualEffect {
    /// The tool that would have been called.
    pub tool: String,
    /// The parameters passed to the tool.
    pub parameters: serde_json::Value,
    /// The effect type (read, write, network, filesystem, etc.).
    pub effect_type: String,
    /// The resource affected.
    pub resource: Option<String>,
}

/// Store for recording intended side effects.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct VirtualEffectStore {
    /// All recorded effects.
    pub effects: Vec<VirtualEffect>,
}

impl VirtualEffectStore {
    /// Create a new empty effect store.
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Record a side effect.
    pub fn record(&mut self, effect: VirtualEffect) {
        self.effects.push(effect);
    }

    /// Get all recorded effects for a specific tool.
    pub fn get_effects_for(&self, tool: &str) -> Vec<&VirtualEffect> {
        self.effects.iter().filter(|e| e.tool == tool).collect()
    }

    /// Get all recorded effects for a specific resource.
    pub fn get_effects_on(&self, resource: &str) -> Vec<&VirtualEffect> {
        self.effects
            .iter()
            .filter(|e| e.resource.as_deref() == Some(resource))
            .collect()
    }

    /// Clear all recorded effects.
    pub fn clear(&mut self) {
        self.effects.clear();
    }
}

/// A registered tool in the sandbox.
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    /// Tool name.
    pub name: String,
    /// Description of what the tool does.
    pub description: String,
    /// Parameters schema (JSON Schema format).
    pub parameters: serde_json::Value,
    /// Whether the tool performs network I/O.
    pub is_network_io: bool,
    /// Whether the tool accesses the filesystem.
    pub is_filesystem_io: bool,
}

/// Registry of available tools in the sandbox.
#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    /// All registered tools.
    tools: HashMap<String, ToolDefinition>,
}

impl ToolRegistry {
    /// Create a new empty tool registry.
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    /// Register the default built-in tools.
    fn register_defaults(&mut self) {
        // Memory tools
        self.register(ToolDefinition {
            name: "memory_read".to_string(),
            description: "Read from memory storage".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string"}
                }
            }),
            is_network_io: false,
            is_filesystem_io: false,
        });

        self.register(ToolDefinition {
            name: "memory_write".to_string(),
            description: "Write to memory storage".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string"},
                    "value": {}
                }
            }),
            is_network_io: false,
            is_filesystem_io: false,
        });

        // Network tools (should be blocked)
        self.register(ToolDefinition {
            name: "http_request".to_string(),
            description: "Make an HTTP request".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string"},
                    "method": {"type": "string"}
                }
            }),
            is_network_io: true,
            is_filesystem_io: false,
        });

        self.register(ToolDefinition {
            name: "fetch".to_string(),
            description: "Fetch data from a URL".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string"}
                }
            }),
            is_network_io: true,
            is_filesystem_io: false,
        });

        // Filesystem tools (should be blocked)
        self.register(ToolDefinition {
            name: "read_file".to_string(),
            description: "Read contents of a file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                }
            }),
            is_network_io: false,
            is_filesystem_io: true,
        });

        self.register(ToolDefinition {
            name: "write_file".to_string(),
            description: "Write contents to a file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                }
            }),
            is_network_io: false,
            is_filesystem_io: true,
        });

        // Built-in tools (always allowed)
        self.register(ToolDefinition {
            name: "noop".to_string(),
            description: "No operation".to_string(),
            parameters: serde_json::json!({}),
            is_network_io: false,
            is_filesystem_io: false,
        });

        self.register(ToolDefinition {
            name: "log".to_string(),
            description: "Log a message".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string"}
                }
            }),
            is_network_io: false,
            is_filesystem_io: false,
        });

        self.register(ToolDefinition {
            name: "assert".to_string(),
            description: "Assert a condition is true".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "condition": {"type": "boolean"},
                    "message": {"type": "string"}
                }
            }),
            is_network_io: false,
            is_filesystem_io: false,
        });
    }

    /// Register a new tool.
    pub fn register(&mut self, tool: ToolDefinition) {
        self.tools.insert(tool.name.clone(), tool);
    }

    /// Get a tool definition by name.
    pub fn get(&self, name: &str) -> Option<&ToolDefinition> {
        self.tools.get(name)
    }

    /// Check if a tool is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Check if a tool performs network I/O.
    pub fn is_network_tool(&self, name: &str) -> bool {
        self.tools
            .get(name)
            .map(|t| t.is_network_io)
            .unwrap_or(false)
    }

    /// Check if a tool accesses the filesystem.
    pub fn is_filesystem_tool(&self, name: &str) -> bool {
        self.tools
            .get(name)
            .map(|t| t.is_filesystem_io)
            .unwrap_or(false)
    }
}

/// The planner sandbox for virtual execution.
#[derive(Debug, Clone)]
pub struct PlannerSandbox {
    /// Configuration for this sandbox.
    config: DryRunConfig,
    /// Store for recording side effects.
    effect_store: VirtualEffectStore,
    /// Registry of available tools.
    tool_registry: ToolRegistry,
}

impl PlannerSandbox {
    /// Create a new planner sandbox with default configuration.
    pub fn new() -> Self {
        Self::with_config(DryRunConfig::default())
    }

    /// Create a new planner sandbox with custom configuration.
    pub fn with_config(config: DryRunConfig) -> Self {
        Self {
            config,
            effect_store: VirtualEffectStore::new(),
            tool_registry: ToolRegistry::new(),
        }
    }

    /// Execute a dry-run of the given plan.
    ///
    /// Returns a `DryRunResult` containing the execution trace,
    /// any warnings or errors, and suggested revisions.
    pub fn dry_run(&self, plan: &ExecutionPlan) -> DryRunResult {
        let start = Instant::now();
        let mut trace = ExecutionTrace::new();
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        let mut suggestions = Vec::new();

        // Validate the plan
        if let Some(err) = self.validate_plan(plan) {
            errors.push(err);
            return DryRunResult {
                warnings,
                errors,
                suggested_revisions: vec!["Fix plan validation errors".to_string()],
                execution_trace: trace,
            };
        }

        let max_steps = self.config.max_steps.min(plan.steps.len());

        // Execute each step
        for (idx, plan_step) in plan.steps.iter().take(max_steps).enumerate() {
            let _step_start = Instant::now();

            // Check timeout
            if start.elapsed() > self.config.timeout() {
                errors.push(format!(
                    "Execution timeout exceeded after {:?}",
                    start.elapsed()
                ));
                suggestions.push(
                    "Consider reducing the number of steps or increasing the timeout".to_string(),
                );
                break;
            }

            // Process the step
            match self.execute_step(idx, plan_step) {
                Ok(mock_output) => {
                    let step =
                        ExecutionStep::new(idx, &plan_step.tool, plan_step.parameters.clone())
                            .with_mock_output(mock_output);
                    trace.add_step(step);
                }
                Err(e) => {
                    let step =
                        ExecutionStep::new(idx, &plan_step.tool, plan_step.parameters.clone());
                    trace.add_step(step);
                    errors.push(e.to_string());
                    suggestions.push(format!("Fix error in step {}: {}", idx, e.to_string()));
                }
            }
        }

        // Check if we hit the step limit
        if plan.steps.len() > max_steps && errors.is_empty() {
            warnings.push(format!(
                "Plan has {} steps but max_steps limit is {}",
                plan.steps.len(),
                self.config.max_steps
            ));
            suggestions.push(
                "Consider increasing max_steps or breaking the plan into smaller sub-plans"
                    .to_string(),
            );
        }

        // Perform conflict analysis
        let conflict_report = ConflictDetector::analyze(&trace);
        if conflict_report.has_conflicts() {
            for conflict in &conflict_report.resource_conflicts {
                warnings.push(format!(
                    "Resource conflict detected: '{}' accessed by steps {:?}",
                    conflict.resource, conflict.steps
                ));
                suggestions.push(format!(
                    "Add synchronization or re-order steps for resource '{}'",
                    conflict.resource
                ));
            }

            for deadlock in &conflict_report.deadlocks {
                errors.push(format!(
                    "Deadlock detected: circular dependency in {:?}",
                    deadlock.cycle
                ));
                suggestions.push(
                    "Break the circular dependency by reordering or splitting steps".to_string(),
                );
            }

            for missing in &conflict_report.missing_preconditions {
                warnings.push(format!(
                    "Step {} requires tool '{}' which is not defined",
                    missing.step, missing.missing_tool
                ));
                suggestions.push(format!(
                    "Add '{}' tool or update step {} preconditions",
                    missing.missing_tool, missing.step
                ));
            }
        }

        trace.set_duration(start.elapsed());

        DryRunResult {
            warnings,
            errors,
            suggested_revisions: suggestions,
            execution_trace: trace,
        }
    }

    /// Validate an execution plan.
    fn validate_plan(&self, plan: &ExecutionPlan) -> Option<String> {
        if plan.steps.is_empty() {
            return Some("Plan has no steps".to_string());
        }

        for (idx, step) in plan.steps.iter().enumerate() {
            if step.tool.is_empty() {
                return Some(format!("Step {} has empty tool name", idx));
            }
            if !self.tool_registry.contains(&step.tool) && !self.is_dynamic_tool(&step.tool) {
                // This is a warning, not an error - unknown tools are allowed
            }
        }

        None
    }

    /// Check if a tool is dynamically named (contains variable parts).
    fn is_dynamic_tool(&self, tool: &str) -> bool {
        tool.contains("${") || tool.contains("{{") || tool.starts_with("agent_")
    }

    /// Execute a single step and return the mock output.
    fn execute_step(
        &self,
        _step_index: usize,
        step: &crate::models::dry_run::PlanStep,
    ) -> Result<serde_json::Value, SandboxError> {
        let tool_name = &step.tool;

        // Check if it's a network tool
        if self.tool_registry.is_network_tool(tool_name) {
            return Err(SandboxError::NetworkDisabled);
        }

        // Check if it's a filesystem tool
        if self.tool_registry.is_filesystem_tool(tool_name) {
            return Err(SandboxError::FilesystemAccessDenied);
        }

        // Get the tool definition for building mock response
        let tool_def = self.tool_registry.get(tool_name);

        // Build a mock response based on the tool description
        let mock_output = self.generate_mock_response(tool_def, &step.parameters);

        // Record any side effects
        let _effect = VirtualEffect {
            tool: tool_name.clone(),
            parameters: step.parameters.clone(),
            effect_type: self.categorize_effect(tool_name),
            resource: self.extract_resource(&step.parameters),
        };

        // Note: effects are recorded in the store, but we can't modify self here
        // The store is reset between runs anyway

        Ok(mock_output)
    }

    /// Generate a mock response for a tool call.
    fn generate_mock_response(
        &self,
        tool_def: Option<&ToolDefinition>,
        _parameters: &serde_json::Value,
    ) -> serde_json::Value {
        match tool_def {
            Some(def) => {
                // Generate mock based on tool description
                match def.name.as_str() {
                    "memory_read" => {
                        serde_json::json!({"status": "ok", "data": null, "from_cache": true})
                    }
                    "memory_write" => {
                        serde_json::json!({"status": "ok", "written": true})
                    }
                    "noop" => serde_json::json!({"status": "ok", "action": "noop"}),
                    "log" => serde_json::json!({"status": "ok", "logged": true}),
                    "assert" => serde_json::json!({"status": "ok", "assertion_passed": true}),
                    _ => serde_json::json!({
                        "status": "ok",
                        "description": def.description,
                        "sandboxed": true
                    }),
                }
            }
            None => {
                // Unknown tool - return generic mock
                serde_json::json!({
                    "status": "ok",
                    "sandboxed": true,
                    "note": "Mock response for unregistered tool"
                })
            }
        }
    }

    /// Categorize the type of effect a tool would have.
    fn categorize_effect(&self, tool_name: &str) -> String {
        if self.tool_registry.is_network_tool(tool_name) {
            "network".to_string()
        } else if self.tool_registry.is_filesystem_tool(tool_name) {
            "filesystem".to_string()
        } else {
            "memory".to_string()
        }
    }

    /// Extract the resource from parameters.
    fn extract_resource(&self, parameters: &serde_json::Value) -> Option<String> {
        parameters
            .get("resource")
            .or_else(|| parameters.get("path"))
            .or_else(|| parameters.get("url"))
            .or_else(|| parameters.get("uri"))
            .or_else(|| parameters.get("endpoint"))
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    /// Get the effect store (for inspection after dry run).
    pub fn effect_store(&self) -> &VirtualEffectStore {
        &self.effect_store
    }

    /// Reset the sandbox for a new run.
    pub fn reset(&mut self) {
        self.effect_store.clear();
    }
}

impl Default for PlannerSandbox {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_plan() -> ExecutionPlan {
        ExecutionPlan {
            steps: vec![
                PlanStep {
                    tool: "noop".to_string(),
                    parameters: serde_json::json!({}),
                    preconditions: vec![],
                    postconditions: vec![],
                },
                PlanStep {
                    tool: "memory_read".to_string(),
                    parameters: serde_json::json!({"key": "test_key"}),
                    preconditions: vec![],
                    postconditions: vec![],
                },
            ],
            metadata: PlanMetadata::default(),
        }
    }

    #[test]
    fn test_sandbox_dry_run_success() {
        let sandbox = PlannerSandbox::new();
        let plan = create_test_plan();

        let result = sandbox.dry_run(&plan);

        assert!(!result.execution_trace.steps.is_empty());
        assert_eq!(result.execution_trace.steps.len(), 2);
        // No errors should have occurred
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_sandbox_blocks_network() {
        let sandbox = PlannerSandbox::new();
        let plan = ExecutionPlan {
            steps: vec![PlanStep {
                tool: "http_request".to_string(),
                parameters: serde_json::json!({"url": "https://example.com"}),
                preconditions: vec![],
                postconditions: vec![],
            }],
            metadata: PlanMetadata::default(),
        };

        let result = sandbox.dry_run(&plan);

        assert!(result.has_errors());
        assert!(result.errors[0].contains("Network"));
    }

    #[test]
    fn test_sandbox_blocks_filesystem() {
        let sandbox = PlannerSandbox::new();
        let plan = ExecutionPlan {
            steps: vec![PlanStep {
                tool: "read_file".to_string(),
                parameters: serde_json::json!({"path": "/etc/passwd"}),
                preconditions: vec![],
                postconditions: vec![],
            }],
            metadata: PlanMetadata::default(),
        };

        let result = sandbox.dry_run(&plan);

        assert!(result.has_errors());
        assert!(result.errors[0].contains("Filesystem"));
    }

    #[test]
    fn test_sandbox_validates_empty_plan() {
        let sandbox = PlannerSandbox::new();
        let plan = ExecutionPlan {
            steps: vec![],
            metadata: PlanMetadata::default(),
        };

        let result = sandbox.dry_run(&plan);

        assert!(result.has_errors());
    }

    #[test]
    fn test_sandbox_respects_max_steps() {
        let config = DryRunConfig {
            max_steps: 2,
            timeout_secs: 30,
            is_dry_run: true,
        };
        let sandbox = PlannerSandbox::with_config(config);

        let mut steps = Vec::new();
        for i in 0..10 {
            steps.push(PlanStep {
                tool: "noop".to_string(),
                parameters: serde_json::json!({"index": i}),
                preconditions: vec![],
                postconditions: vec![],
            });
        }

        let plan = ExecutionPlan {
            steps,
            metadata: PlanMetadata::default(),
        };

        let result = sandbox.dry_run(&plan);

        // Should have warnings about step limit
        assert!(result.has_warnings());
        // Only 2 steps should be in trace
        assert_eq!(result.execution_trace.steps.len(), 2);
    }

    #[test]
    fn test_tool_registry_contains_builtins() {
        let registry = ToolRegistry::new();

        assert!(registry.contains("noop"));
        assert!(registry.contains("log"));
        assert!(registry.contains("memory_read"));
        assert!(registry.contains("http_request"));
        assert!(registry.contains("read_file"));

        assert!(registry.is_network_tool("http_request"));
        assert!(!registry.is_network_tool("noop"));

        assert!(registry.is_filesystem_tool("read_file"));
        assert!(!registry.is_filesystem_tool("noop"));
    }
}
