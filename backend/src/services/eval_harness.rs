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
//! before the prediction path is hooked up (#92, #93).

use serde::{Deserialize, Serialize};

use crate::models::{
    MemoryType, Modality, ReasoningDepth, ResourceConstraints, TaskContext, TaskType, TemporalScope,
};

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

    /// Run every registered case through the **real** adaptive scheduler and
    /// compare against expected outcomes.
    ///
    /// A case passes iff:
    /// 1. The scheduler's primary + secondary memory types are a superset of
    ///    `expected_memory_types` (the expected types are present in the selection).
    /// 2. The predicted efficiency meets or exceeds `min_expected_efficiency`.
    ///
    /// This replaced the scaffold that hardcoded `passed = true` (backlog A-7).
    /// The scheduler is a heuristic/static model (not learned — that's P3), so
    /// calling it is cheap and deterministic.
    pub async fn run(&mut self) {
        use crate::services::scheduler::AdaptiveMemoryScheduler;

        let scheduler = AdaptiveMemoryScheduler::new(Box::new(
            crate::services::predictor::PerformancePredictionModel::new(),
        ));
        let preferences = crate::models::TaskPreferences {
            prioritize_efficiency: true,
            prioritize_coherence: true,
            enable_multimodal: true,
            enable_reasoning: true,
        };
        let mut results = Vec::with_capacity(self.cases.len());

        for case in &self.cases {
            let start = std::time::Instant::now();

            let trace_result = scheduler
                .adaptive_memory_selection_trace(
                    &case.task_context,
                    &case.resource_constraints,
                    &preferences,
                )
                .await;

            let duration_ms = start.elapsed().as_millis() as u64;

            match trace_result {
                Ok(trace) => {
                    let result = &trace.final_result;
                    let mut selected: Vec<String> = vec![memory_type_label_from_model(
                        &result.memory_config.primary_memory,
                    )];
                    for sec in &result.memory_config.secondary_memory {
                        selected.push(memory_type_label_from_model(sec));
                    }

                    let expected_labels: Vec<String> = case
                        .expected_memory_types
                        .iter()
                        .map(memory_type_label)
                        .collect();
                    let types_match = expected_labels.iter().all(|e| selected.contains(e));

                    let efficiency = result.performance_prediction.efficiency_gain;
                    let coherence = result.performance_prediction.coherence_gain;
                    let meets_efficiency = efficiency >= case.min_expected_efficiency;

                    results.push(EvalResult {
                        test_name: case.name.clone(),
                        passed: types_match && meets_efficiency,
                        actual_efficiency: efficiency,
                        actual_coherence: coherence,
                        selected_memory_types: selected,
                        duration_ms,
                    });
                }
                Err(e) => {
                    results.push(EvalResult {
                        test_name: case.name.clone(),
                        passed: false,
                        actual_efficiency: 0.0,
                        actual_coherence: 0.0,
                        selected_memory_types: vec![format!("error: {e}")],
                        duration_ms,
                    });
                }
            }
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
        let sum_efficiency: f64 = self.results.iter().map(|r| r.actual_efficiency).sum();
        let sum_coherence: f64 = self.results.iter().map(|r| r.actual_coherence).sum();
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

/// Convert the models-layer `MemoryType` (from `MemoryConfig`) to a label.
fn memory_type_label_from_model(m: &MemoryType) -> String {
    memory_type_label(m)
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

    #[tokio::test]
    async fn run_records_a_result_per_case() {
        let mut suite = build_standard_suite();
        assert!(suite.results.is_empty());
        suite.run().await;
        assert_eq!(suite.results.len(), 3);
        // The real scheduler may or may not pass all cases depending on its
        // heuristic — what matters is that it produces results, not placeholders.
        for result in &suite.results {
            assert!(
                result.duration_ms < 5000,
                "case {} took too long ({}ms) — may be hanging",
                result.test_name,
                result.duration_ms
            );
            assert!(
                !result.selected_memory_types.is_empty(),
                "case {} produced no memory type selection",
                result.test_name
            );
        }
    }

    #[tokio::test]
    async fn summary_aggregates_results() {
        let mut suite = build_standard_suite();
        suite.run().await;
        let summary = suite.summary();
        assert_eq!(summary.total, 3);
        // Real scheduler results — not necessarily all pass, but summary is coherent
        assert_eq!(summary.passed + summary.failed, 3);
        assert!(summary.avg_efficiency.is_finite());
        assert!(summary.avg_coherence.is_finite());
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

    #[tokio::test]
    async fn selected_memory_types_are_valid_labels() {
        let mut suite = build_standard_suite();
        suite.run().await;
        let valid = ["stm", "ltm", "kg", "mm"];
        for result in &suite.results {
            for label in &result.selected_memory_types {
                if label.starts_with("error:") {
                    continue; // scheduler error — not a label issue
                }
                assert!(
                    valid.contains(&label.as_str()),
                    "unexpected memory type label: {label}"
                );
            }
        }
    }
}
