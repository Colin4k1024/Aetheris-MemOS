//! Dry Run Models - Virtual Execution Sandbox Types
//!
//! This module defines the data structures for planner agent dry-run simulation.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use utoipa::ToSchema;

/// Configuration for dry-run execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunConfig {
    /// Maximum number of steps to execute before halting.
    pub max_steps: usize,
    /// Timeout for the entire dry-run execution (in seconds).
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Whether this is a dry run (always true for sandbox).
    #[serde(default = "default_true")]
    pub is_dry_run: bool,
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> u64 {
    30
}

impl DryRunConfig {
    /// Get the timeout as a Duration.
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }
}

impl Default for DryRunConfig {
    fn default() -> Self {
        Self {
            max_steps: 100,
            timeout_secs: 30,
            is_dry_run: true,
        }
    }
}

/// A single step in the execution trace.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecutionStep {
    /// Zero-based step index.
    pub step_index: usize,
    /// The action/tool name being executed.
    pub action: String,
    /// Input parameters to the action.
    pub input: serde_json::Value,
    /// Mock output returned by the sandbox (None if still executing or error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mock_output: Option<serde_json::Value>,
}

impl ExecutionStep {
    /// Create a new execution step.
    pub fn new(step_index: usize, action: impl Into<String>, input: serde_json::Value) -> Self {
        Self {
            step_index,
            action: action.into(),
            input,
            mock_output: None,
        }
    }

    /// Set the mock output for this step.
    pub fn with_mock_output(mut self, output: serde_json::Value) -> Self {
        self.mock_output = Some(output);
        self
    }
}

/// Complete trace of a dry-run execution.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecutionTrace {
    /// All steps that were executed.
    pub steps: Vec<ExecutionStep>,
    /// Total duration of the dry-run execution (in milliseconds).
    #[serde(default)]
    pub total_duration_ms: u64,
}

impl ExecutionTrace {
    /// Create a new empty execution trace.
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            total_duration_ms: 0,
        }
    }

    /// Add a step to the trace.
    pub fn add_step(&mut self, step: ExecutionStep) {
        self.steps.push(step);
    }

    /// Set the total duration.
    pub fn set_duration(&mut self, duration: Duration) {
        self.total_duration_ms = duration.as_millis() as u64;
    }

    /// Get the total duration.
    pub fn total_duration(&self) -> Duration {
        Duration::from_millis(self.total_duration_ms)
    }
}

impl Default for ExecutionTrace {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a dry-run execution.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DryRunResult {
    /// Warnings encountered during execution (non-fatal issues).
    pub warnings: Vec<String>,
    /// Errors encountered during execution.
    pub errors: Vec<String>,
    /// Suggested revisions to fix errors or improve the plan.
    pub suggested_revisions: Vec<String>,
    /// The execution trace with all steps and their outcomes.
    pub execution_trace: ExecutionTrace,
}

impl DryRunResult {
    /// Create a new dry-run result.
    pub fn new(execution_trace: ExecutionTrace) -> Self {
        Self {
            warnings: Vec::new(),
            errors: Vec::new(),
            suggested_revisions: Vec::new(),
            execution_trace,
        }
    }

    /// Add a warning.
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// Add an error.
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.errors.push(error.into());
        self
    }

    /// Add a suggested revision.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggested_revisions.push(suggestion.into());
        self
    }

    /// Check if the dry run had any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Check if the dry run had any warnings.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// An execution plan to be simulated in the sandbox.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecutionPlan {
    /// Steps in the execution plan.
    pub steps: Vec<PlanStep>,
    /// Metadata about the plan.
    #[serde(default)]
    pub metadata: PlanMetadata,
}

/// A single step in an execution plan.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PlanStep {
    /// The tool or action to execute.
    pub tool: String,
    /// Input parameters for the tool.
    pub parameters: serde_json::Value,
    /// Expected preconditions for this step.
    #[serde(default)]
    pub preconditions: Vec<String>,
    /// Expected postconditions/Effects after this step.
    #[serde(default)]
    pub postconditions: Vec<String>,
}

/// Metadata about an execution plan.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct PlanMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_step_creation() {
        let step = ExecutionStep::new(0, "read_file", serde_json::json!({"path": "/test.txt"}));
        assert_eq!(step.step_index, 0);
        assert_eq!(step.action, "read_file");
        assert!(step.mock_output.is_none());
    }

    #[test]
    fn test_execution_trace_add_step() {
        let mut trace = ExecutionTrace::new();
        trace.add_step(ExecutionStep::new(0, "test", serde_json::json!({})));
        assert_eq!(trace.steps.len(), 1);
    }

    #[test]
    fn test_dry_run_result_has_errors() {
        let result = DryRunResult::new(ExecutionTrace::new()).with_error("Network call attempted");
        assert!(result.has_errors());
        assert!(!result.has_warnings());
    }
}
