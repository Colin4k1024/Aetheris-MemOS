//! Integration tests for Planner Sandbox

use backend::models::dry_run::{DryRunConfig, DryRunResult, ExecutionPlan, ExecutionStep, ExecutionTrace};
use backend::runtime::planner_sandbox::{PlannerSandbox, ToolRegistry, VirtualEffectStore};
use backend::services::conflict_detector::{ConflictDetector, ConflictReport, ResourceConflict};
use std::time::Duration;

#[test]
fn test_dry_run_config() {
    let config = DryRunConfig {
        max_steps: 10,
        timeout_secs: 60,
        is_dry_run: true,
    };
    assert_eq!(config.max_steps, 10);
    assert!(config.is_dry_run);
}

#[test]
fn test_execution_step() {
    let step = ExecutionStep::new(0, "test_tool", serde_json::json!({}));
    assert_eq!(step.step_index, 0);
    assert_eq!(step.action, "test_tool");
}

#[test]
fn test_execution_trace() {
    let mut trace = ExecutionTrace::new();
    trace.add_step(ExecutionStep::new(0, "tool1", serde_json::json!({})));
    trace.add_step(ExecutionStep::new(1, "tool2", serde_json::json!({})));
    assert_eq!(trace.steps.len(), 2);
}

#[test]
fn test_tool_registry_builtins() {
    let registry = ToolRegistry::new();
    assert!(registry.contains("noop"));
    assert!(registry.contains("log"));
    assert!(registry.contains("memory_read"));
}

#[test]
fn test_tool_registry_classification() {
    let registry = ToolRegistry::new();
    assert!(registry.is_network_tool("http_request"));
    assert!(!registry.is_network_tool("noop"));
    assert!(registry.is_filesystem_tool("read_file"));
    assert!(!registry.is_filesystem_tool("noop"));
}

#[test]
fn test_conflict_detector_no_conflicts() {
    let mut trace = ExecutionTrace::new();
    trace.add_step(ExecutionStep::new(0, "read", serde_json::json!({"resource": "res_a"})));
    trace.add_step(ExecutionStep::new(1, "read", serde_json::json!({"resource": "res_b"})));

    let report = ConflictDetector::analyze(&trace);
    assert!(report.resource_conflicts.is_empty());
}

#[test]
fn test_dry_run_result_helpers() {
    let result = DryRunResult::new(ExecutionTrace::new())
        .with_warning("test warning")
        .with_error("test error");

    assert!(result.has_errors());
    assert!(result.has_warnings());
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.errors.len(), 1);
}

#[test]
fn test_virtual_effect_store() {
    let store = VirtualEffectStore::new();
    assert_eq!(store.effects.len(), 0);
}
