//! Eval harness scaffold — W3.4
//!
//! Evaluation framework for benchmarking adaptive memory strategies.
//!
//! Provides:
//! - [`EvalTestCase`] — a single benchmark scenario with expected outcomes
//! - [`EvalResult`] — the recorded outcome of running one case
//! - [`EvalSuite`] — a collection of cases plus their results
//! - [`EvalSummary`] — aggregate pass/fail and average metrics
//! - [`build_standard_suite`] — three built-in benchmark cases
//!
//! The current implementation is a scaffold: [`EvalSuite::run`] records
//! placeholder results (`passed = true`) without invoking the real
//! scheduler/predictor. This lets downstream wiring and reporting land
//! before the prediction path is hooked up.

use serde::{Deserialize, Serialize};

use crate::models::{MemoryType, Modality, ReasoningDepth, ResourceConstraints, TaskContext, TaskType, TemporalScope};

/// A single evaluation test case.
///
/// Describes the task context, the resource envelope, and the expected
/// behaviour of the adaptive memory pipeline for that scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalTestCase {
    /// Human-readable case name, used as the join key for results.
    pub name: String,
    /// The task profile the scheduler should react to.
    pub task_context: TaskContext,
    /// Resource limits the scheduler must respect.
    pub resource_constraints: ResourceConstraints,
    /// Memory types the optimal strategy is expected to select.
    pub expected_memory_types: Vec<MemoryType>,
    /// Minimum acceptable efficiency score for the case to pass.
    pub min_expected_efficiency: f64,
}

/// The recorded outcome of running a single [`EvalTestCase`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    /// Name of the case this result belongs to.
    pub test_name: String,
    /// Whether the case met its acceptance criteria.
    pub passed: bool,
    /// Measured efficiency score (higher is better).
    pub actual_efficiency: f64,
    /// Measured coherence score (higher is better).
    pub actual_coherence: f64,
    /// Memory types the strategy actually selected, as lowercase strings
    /// (e.g. `"stm"`, `"ltm"`, `"kg"`, `"mm"`).
    pub selected_memory_types: Vec<String>,
    /// Wall-clock duration of the run, in milliseconds.
    pub duration_ms: u64,
}

/// A suite of evaluation cases and their accumulated results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSuite {
    /// Suite name, e.g. `"standard"`.
    pub name: String,
    /// Cases registered for this suite.
    pub cases: Vec<EvalTestCase>,
    /// Results recorded so far. Populated by [`EvalSuite::run`].
    pub results: Vec<EvalResult>,
}

impl EvalSuite {
    /// Create an empty suite with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cases: Vec::new(),
            results: Vec::new(),
        }
    }

    /// Append a case to the suite.
    pub fn add_case(&mut self, case: EvalTestCase) {
        self.cases.push(case);
    }

    /// Run every registered case and record placeholder results.
    ///
    /// This is a scaffold: it does NOT invoke the real scheduler or
    /// predictor. Each case is recorded as passing with efficiency equal
    /// to its `min_expected_efficiency` threshold and coherence `1.0`,
    /// so the suite reports a clean pass until the real path is wired in.
    pub fn run(&mut self) {
        let mut results = Vec::with_capacity(self.cases.len());
        for case in &self.cases {
            // Placeholder logic — replace with a real scheduler call in a
            // later work item. Returning `passed = true` with the threshold
            // efficiency keeps the reporting path honest while the
            // prediction layer is being integrated.
            let selected_memory_types = case
                .expected_memory_types
                .iter()
                .map(memory_type_label)
                .collect();
            results.push(EvalResult {
                test_name: case.name.clone(),
                passed: true,
                actual_efficiency: case.min_expected_efficiency,
                actual_coherence: 1.0,
                selected_memory_types,
                duration_ms: 0,
            });
        }
        self.results = results;
    }

    /// Compute aggregate pass/fail and mean metrics from the recorded results.
    ///
    /// Returns zeroes for every average when no results have been recorded
    /// yet, so callers never see `NaN`.
    pub fn summary(&self) -> EvalSummary {
        let total = self.results.len();
        if total == 0 {
            return EvalSummary {
                total: 0,
                passed: 0,
                failed: 0,
                avg_efficiency: 0.0,
                avg_coherence: 0.0,
            };
        }
        let passed = self.results.iter().filter(|r| r.passed).count();
        let failed = total - passed;
        let sum_efficiency: f64 =
            self.results.iter().map(|r| r.actual_efficiency).sum();
        let sum_coherence: f64 =
            self.results.iter().map(|r| r.actual_coherence).sum();
        let avg_efficiency = sum_efficiency / total as f64;
        let avg_coherence = sum_coherence / total as f64;
        EvalSummary {
            total,
            passed,
            failed,
            avg_efficiency,
            avg_coherence,
        }
    }
}

/// Aggregate metrics computed over a suite's results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSummary {
    /// Total number of results recorded.
    pub total: usize,
    /// How many cases passed.
    pub passed: usize,
    /// How many cases failed.
    pub failed: usize,
    /// Mean `actual_efficiency` across all results.
    pub avg_efficiency: f64,
    /// Mean `actual_coherence` across all results.
    pub avg_coherence: f64,
}

/// Render a [`MemoryType`] as the lowercase label used in
/// [`EvalResult::selected_memory_types`].
fn memory_type_label(m: &MemoryType) -> String {
    match m {
        MemoryType::Stm => "stm".to_string(),
        MemoryType::Ltm => "ltm".to_string(),
        MemoryType::Kg => "kg".to_string(),
        MemoryType::Mm => "mm".to_string(),
    }
}

/// Build the standard benchmark suite: three representative cases spanning
/// simple text, multimodal, and deep-reasoning scenarios.
pub fn build_standard_suite() -> EvalSuite {
    let mut suite = EvalSuite::new("standard");
    suite.add_case(simple_text_task());
    suite.add_case(complex_multimodal_task());
    suite.add_case(deep_reasoning_task());
    suite
}

/// "simple_text_task" — a low-complexity, text-only conversation.
///
/// Expected strategy: STM as primary memory, no multimodal or graph
/// engagement.
fn simple_text_task() -> EvalTestCase {
    EvalTestCase {
        name: "simple_text_task".to_string(),
        task_context: TaskContext {
            task_id: "eval-simple-text-001".to_string(),
            task_type: TaskType::Conversation,
            complexity: 0.2,
            modality_requirements: vec![Modality::Text],
            temporal_scope: TemporalScope::Short,
            reasoning_depth: ReasoningDepth::Shallow,
            context_dependency: 0.1,
            user_id: "eval-user".to_string(),
            agent_id: "eval-agent".to_string(),
        },
        resource_constraints: ResourceConstraints {
            max_memory_usage_mb: 128,
            max_cpu_usage_percent: 30,
            max_response_time_ms: 200,
            storage_quota_percent: 50,
        },
        expected_memory_types: vec![MemoryType::Stm],
        min_expected_efficiency: 0.8,
    }
}

/// "complex_multimodal_task" — a medium-complexity task mixing text and
/// image modalities over a medium temporal scope.
///
/// Expected strategy: STM + LTM + MM engagement.
fn complex_multimodal_task() -> EvalTestCase {
    EvalTestCase {
        name: "complex_multimodal_task".to_string(),
        task_context: TaskContext {
            task_id: "eval-multimodal-001".to_string(),
            task_type: TaskType::Task,
            complexity: 0.65,
            modality_requirements: vec![Modality::Text, Modality::Image],
            temporal_scope: TemporalScope::Medium,
            reasoning_depth: ReasoningDepth::Medium,
            context_dependency: 0.55,
            user_id: "eval-user".to_string(),
            agent_id: "eval-agent".to_string(),
        },
        resource_constraints: ResourceConstraints {
            max_memory_usage_mb: 512,
            max_cpu_usage_percent: 60,
            max_response_time_ms: 800,
            storage_quota_percent: 70,
        },
        expected_memory_types: vec![MemoryType::Stm, MemoryType::Ltm, MemoryType::Mm],
        min_expected_efficiency: 0.7,
    }
}

/// "deep_reasoning_task" — a high-complexity, deep-reasoning query spanning
/// a long temporal scope.
///
/// Expected strategy: LTM + KG engagement.
fn deep_reasoning_task() -> EvalTestCase {
    EvalTestCase {
        name: "deep_reasoning_task".to_string(),
        task_context: TaskContext {
            task_id: "eval-deep-reasoning-001".to_string(),
            task_type: TaskType::Query,
            complexity: 0.9,
            modality_requirements: vec![Modality::Text],
            temporal_scope: TemporalScope::Long,
            reasoning_depth: ReasoningDepth::Deep,
            context_dependency: 0.85,
            user_id: "eval-user".to_string(),
            agent_id: "eval-agent".to_string(),
        },
        resource_constraints: ResourceConstraints {
            max_memory_usage_mb: 1024,
            max_cpu_usage_percent: 80,
            max_response_time_ms: 2000,
            storage_quota_percent: 85,
        },
        expected_memory_types: vec![MemoryType::Ltm, MemoryType::Kg],
        min_expected_efficiency: 0.6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_suite_has_three_cases() {
        let suite = build_standard_suite();
        assert_eq!(suite.cases.len(), 3);
        assert_eq!(suite.cases[0].name, "simple_text_task");
        assert_eq!(suite.cases[1].name, "complex_multimodal_task");
        assert_eq!(suite.cases[2].name, "deep_reasoning_task");
    }

    #[test]
    fn run_records_a_result_per_case() {
        let mut suite = build_standard_suite();
        assert!(suite.results.is_empty());
        suite.run();
        assert_eq!(suite.results.len(), 3);
        for result in &suite.results {
            assert!(result.passed, "case {} should pass in scaffold", result.test_name);
        }
    }

    #[test]
    fn summary_aggregates_results() {
        let mut suite = build_standard_suite();
        suite.run();
        let summary = suite.summary();
        assert_eq!(summary.total, 3);
        assert_eq!(summary.passed, 3);
        assert_eq!(summary.failed, 0);
        // placeholder efficiency equals min_expected_efficiency per case
        let expected_avg =
            (0.8 + 0.7 + 0.6) / 3.0;
        assert!((summary.avg_efficiency - expected_avg).abs() < 1e-9);
        assert!((summary.avg_coherence - 1.0).abs() < 1e-9);
    }

    #[test]
    fn summary_on_empty_suite_is_zero_without_nan() {
        let suite = EvalSuite::new("empty");
        let summary = suite.summary();
        assert_eq!(summary.total, 0);
        assert_eq!(summary.passed, 0);
        assert_eq!(summary.failed, 0);
        assert!(summary.avg_efficiency.is_finite());
        assert!(summary.avg_coherence.is_finite());
        assert_eq!(summary.avg_efficiency, 0.0);
    }

    #[test]
    fn selected_memory_types_are_lowercase_labels() {
        let mut suite = build_standard_suite();
        suite.run();
        // simple_text_task -> ["stm"]
        assert_eq!(suite.results[0].selected_memory_types, vec!["stm"]);
        // complex_multimodal_task -> ["stm","ltm","mm"]
        assert_eq!(
            suite.results[1].selected_memory_types,
            vec!["stm", "ltm", "mm"]
        );
        // deep_reasoning_task -> ["ltm","kg"]
        assert_eq!(
            suite.results[2].selected_memory_types,
            vec!["ltm", "kg"]
        );
    }
}
