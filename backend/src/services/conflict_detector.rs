//! Conflict Detection Service
//!
//! This module analyzes execution traces to detect resource conflicts,
//! deadlocks, and missing preconditions.

use crate::models::dry_run::{ExecutionStep, ExecutionTrace};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use utoipa::ToSchema;

/// A resource conflict where the same resource is accessed by overlapping steps.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResourceConflict {
    /// The resource that has conflicting access.
    pub resource: String,
    /// Indices of steps that conflict over this resource.
    pub steps: Vec<usize>,
}

/// A deadlock detected in the execution trace.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Deadlock {
    /// The cycle of step/tool names that form the deadlock.
    pub cycle: Vec<String>,
}

/// A missing precondition detected during execution.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MissingPrecondition {
    /// The step index that has a missing precondition.
    pub step: usize,
    /// The tool/action that was expected but not available.
    pub missing_tool: String,
}

/// Complete conflict analysis report.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct ConflictReport {
    /// All detected resource conflicts.
    pub resource_conflicts: Vec<ResourceConflict>,
    /// All detected deadlocks.
    pub deadlocks: Vec<Deadlock>,
    /// All detected missing preconditions.
    pub missing_preconditions: Vec<MissingPrecondition>,
}

impl ConflictReport {
    /// Check if any conflicts were detected.
    pub fn has_conflicts(&self) -> bool {
        !self.resource_conflicts.is_empty()
            || !self.deadlocks.is_empty()
            || !self.missing_preconditions.is_empty()
    }

    /// Get the total count of all conflicts.
    pub fn total_conflict_count(&self) -> usize {
        self.resource_conflicts.len() + self.deadlocks.len() + self.missing_preconditions.len()
    }
}

/// Tool for detecting conflicts in execution traces.
pub struct ConflictDetector;

impl ConflictDetector {
    /// Analyze an execution trace for conflicts.
    ///
    /// This method detects:
    /// - Resource conflicts: same resource accessed by overlapping steps
    /// - Deadlocks: circular wait between workflows
    /// - Missing preconditions: required tools not available before a step
    pub fn analyze(trace: &ExecutionTrace) -> ConflictReport {
        let mut report = ConflictReport::default();

        // Detect resource conflicts
        report.resource_conflicts = Self::detect_resource_conflicts(trace);

        // Detect deadlocks
        report.deadlocks = Self::detect_deadlocks(trace);

        // Detect missing preconditions
        report.missing_preconditions = Self::detect_missing_preconditions(trace);

        report
    }

    /// Detect resource conflicts in the trace.
    fn detect_resource_conflicts(trace: &ExecutionTrace) -> Vec<ResourceConflict> {
        let mut conflicts = Vec::new();
        let mut resource_steps: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();

        // Group steps by resource
        for step in &trace.steps {
            let resources = Self::extract_resources(&step.action, &step.input);
            for resource in resources {
                resource_steps.entry(resource).or_default().push(step.step_index);
            }
        }

        // Find conflicts: same resource accessed by overlapping step ranges
        for (resource, steps) in resource_steps {
            if Self::has_overlapping_access(&steps) {
                conflicts.push(ResourceConflict {
                    resource,
                    steps,
                });
            }
        }

        conflicts
    }

    /// Extract resources from a step's action and input.
    fn extract_resources(action: &str, input: &serde_json::Value) -> Vec<String> {
        let mut resources = Vec::new();

        // Add the action itself as a resource
        resources.push(action.to_string());

        // Extract resources from input parameters
        if let Some(obj) = input.as_object() {
            for (key, value) in obj {
                // Common resource-related parameter names
                match key.as_str() {
                    "resource" | "path" | "file" | "url" | "uri" | "endpoint" => {
                        if let Some(s) = value.as_str() {
                            resources.push(s.to_string());
                        }
                    }
                    "resources" => {
                        if let Some(arr) = value.as_array() {
                            for v in arr {
                                if let Some(s) = v.as_str() {
                                    resources.push(s.to_string());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        resources
    }

    /// Check if a list of steps has overlapping access patterns.
    fn has_overlapping_access(steps: &[usize]) -> bool {
        if steps.len() < 2 {
            return false;
        }

        // Sort steps
        let mut sorted = steps.to_vec();
        sorted.sort();

        // Check for adjacent or close steps (within 2 indices)
        for window in sorted.windows(2) {
            if window[1] - window[0] <= 2 {
                return true;
            }
        }

        false
    }

    /// Detect deadlocks in the execution trace.
    fn detect_deadlocks(trace: &ExecutionTrace) -> Vec<Deadlock> {
        let mut deadlocks = Vec::new();

        // Build dependency graph: step -> steps it waits for
        let mut dependencies: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        let mut reverse_deps: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();

        for step in &trace.steps {
            if let Some(preconditions) = step.input.get("preconditions").and_then(|p| p.as_array())
            {
                let mut deps = Vec::new();
                for precondition in preconditions {
                    // Parse precondition format: "step_N.tool_name" or just "tool_name"
                    if let Some(s) = precondition.as_str() {
                        if s.starts_with("step_") {
                            if let Some(idx) = s
                                .strip_prefix("step_")
                                .and_then(|rest| rest.split('.').next())
                                .and_then(|idx| idx.parse::<usize>().ok())
                            {
                                deps.push(idx);
                            }
                        }
                    }
                }

                if !deps.is_empty() {
                    dependencies.insert(step.step_index, deps.clone());
                    for dep in deps {
                        reverse_deps.entry(dep).or_default().push(step.step_index);
                    }
                }
            }
        }

        // Find cycles using DFS
        let mut visited = std::collections::HashSet::new();
        let mut rec_stack = std::collections::HashSet::new();
        let mut path: Vec<usize> = Vec::new();

        for step in &trace.steps {
            if !visited.contains(&step.step_index) {
                if let Some(cycle) =
                    Self::detect_cycle_dfs(step.step_index, &dependencies, &mut visited, &mut rec_stack, &mut path)
                {
                    let cycle_tools: Vec<String> = cycle
                        .iter()
                        .filter_map(|&idx| {
                            trace.steps.iter().find(|s| s.step_index == idx)
                        })
                        .map(|s| s.action.clone())
                        .collect();

                    deadlocks.push(Deadlock { cycle: cycle_tools });
                }
            }
        }

        deadlocks
    }

    /// DFS helper for cycle detection.
    fn detect_cycle_dfs(
        node: usize,
        dependencies: &std::collections::HashMap<usize, Vec<usize>>,
        visited: &mut std::collections::HashSet<usize>,
        rec_stack: &mut std::collections::HashSet<usize>,
        path: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        visited.insert(node);
        rec_stack.insert(node);
        path.push(node);

        if let Some(deps) = dependencies.get(&node) {
            for &dep in deps {
                if !visited.contains(&dep) {
                    if let Some(cycle) =
                        Self::detect_cycle_dfs(dep, dependencies, visited, rec_stack, path)
                    {
                        return Some(cycle);
                    }
                } else if rec_stack.contains(&dep) {
                    // Found a cycle
                    let cycle_start = path.iter().position(|&x| x == dep).unwrap();
                    return Some(path[cycle_start..].to_vec());
                }
            }
        }

        path.pop();
        rec_stack.remove(&node);
        None
    }

    /// Detect missing preconditions in the execution trace.
    fn detect_missing_preconditions(trace: &ExecutionTrace) -> Vec<MissingPrecondition> {
        let mut preconditions = Vec::new();
        let mut available_tools: HashSet<String> = HashSet::new();

        // First, collect all tools that are defined in the plan
        for step in &trace.steps {
            available_tools.insert(step.action.clone());
        }

        // Now check each step's input for preconditions
        for step in &trace.steps {
            if let Some(required) = step.input.get("requires").and_then(|r| r.as_array()) {
                for req in required {
                    if let Some(tool_name) = req.as_str() {
                        if !available_tools.contains(tool_name)
                            && !Self::is_builtin_tool(tool_name)
                        {
                            preconditions.push(MissingPrecondition {
                                step: step.step_index,
                                missing_tool: tool_name.to_string(),
                            });
                        }
                    }
                }
            }
        }

        preconditions
    }

    /// Check if a tool is a built-in tool (always available).
    fn is_builtin_tool(tool: &str) -> bool {
        matches!(
            tool,
            "noop"
                | "log"
                | "assert"
                | "env_get"
                | "env_set"
                | "sleep"
                | "timestamp"
                | "uuid"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_conflict_report_has_conflicts() {
        let mut report = ConflictReport::default();
        assert!(!report.has_conflicts());

        report.resource_conflicts.push(ResourceConflict {
            resource: "memory".to_string(),
            steps: vec![0, 1, 2],
        });
        assert!(report.has_conflicts());
    }

    #[test]
    fn test_resource_conflict_detection() {
        let mut trace = ExecutionTrace::new();
        trace.add_step(ExecutionStep::new(
            0,
            "read",
            serde_json::json!({"resource": "db_connection"}),
        ));
        trace.add_step(ExecutionStep::new(
            1,
            "write",
            serde_json::json!({"resource": "db_connection"}),
        ));
        trace.add_step(ExecutionStep::new(
            2,
            "read",
            serde_json::json!({"resource": "db_connection"}),
        ));

        let report = ConflictDetector::analyze(&trace);
        // Steps 0, 1, 2 all access the same resource consecutively
        assert!(!report.resource_conflicts.is_empty());
    }

    #[test]
    fn test_missing_precondition_detection() {
        let mut trace = ExecutionTrace::new();
        trace.add_step(ExecutionStep::new(
            0,
            "process",
            serde_json::json!({"requires": ["init", "setup"]}),
        ));

        let report = ConflictDetector::analyze(&trace);
        // "init" and "setup" are not defined in the plan
        assert!(!report.missing_preconditions.is_empty());
        assert_eq!(report.missing_preconditions.len(), 2);
    }

    #[test]
    fn test_builtin_tools_not_missing() {
        let mut trace = ExecutionTrace::new();
        trace.add_step(ExecutionStep::new(
            0,
            "process",
            serde_json::json!({"requires": ["noop", "log"]}),
        ));

        let report = ConflictDetector::analyze(&trace);
        // "noop" and "log" are built-in, should not be reported as missing
        assert!(report.missing_preconditions.is_empty());
    }

    #[test]
    fn test_extract_resources() {
        let resources = ConflictDetector::extract_resources(
            "read_file",
            &serde_json::json!({"path": "/tmp/test.txt"}),
        );
        assert!(resources.contains(&"read_file".to_string()));
        assert!(resources.contains(&"/tmp/test.txt".to_string()));
    }
}
