use crate::db::{
    decision_trace::DecisionTraceRepository, memory::MemoryConfigRepository,
    weights::WeightHistoryRepository,
};
use crate::models::{ResourceConstraints, TaskContext, TaskPreferences};
use crate::services::evidence_graph::record_decision_trace_as_evidence;
use crate::services::scheduler::{DecisionTrace, MemorySelectionResult};
use crate::services::AdaptiveMemoryScheduler;
use crate::AppError;

#[derive(Debug, Clone)]
pub struct SelectionOptions {
    pub explain: bool,
    pub dry_run: bool,
    pub persist_trace: bool,
    pub what_if_constraints: Option<ResourceConstraints>,
}

#[derive(Debug, Clone)]
pub struct SelectionOutcome {
    pub final_result: MemorySelectionResult,
    pub trace: Option<DecisionTrace>,
    pub what_if_result: Option<MemorySelectionResult>,
}

#[derive(Debug, Clone)]
pub struct DecisionTraceEnvelope {
    pub trace_id: String,
    pub task_id: String,
    pub created_at: String,
    pub trace: DecisionTrace,
}

#[derive(Debug, Clone)]
pub struct PersistedTraceArtifacts {
    pub trace_id: String,
    pub workflow_id: String,
    pub run_id: String,
    pub verified: bool,
}

pub async fn select_memory(
    scheduler: &AdaptiveMemoryScheduler,
    task_context: &TaskContext,
    resource_constraints: &ResourceConstraints,
    preferences: &TaskPreferences,
    options: SelectionOptions,
) -> Result<SelectionOutcome, AppError> {
    // Explain and dry-run flows are trace-first by design.
    if options.dry_run || options.explain {
        let trace = scheduler
            .adaptive_memory_selection_trace(task_context, resource_constraints, preferences)
            .await?;

        if !options.dry_run {
            persist_from_trace(task_context, &trace).await?;
        }

        if options.persist_trace && !options.dry_run {
            persist_trace_record(&trace).await?;
        }

        let what_if_result = if let Some(ref w) = options.what_if_constraints {
            Some(
                scheduler
                    .adaptive_memory_selection_trace(task_context, w, preferences)
                    .await?
                    .final_result,
            )
        } else {
            None
        };

        return Ok(SelectionOutcome {
            final_result: trace.final_result.clone(),
            trace: Some(trace),
            what_if_result,
        });
    }

    // Keep the normal selection path behavior unchanged (including its persistence side-effects).
    let result = scheduler
        .adaptive_memory_selection(task_context, resource_constraints, preferences)
        .await?;

    // When persist_trace=true (and not dry_run), also persist a decision trace even if explain=false.
    // This aligns API semantics without forcing trace payload in response.
    if options.persist_trace {
        let trace = scheduler
            .adaptive_memory_selection_trace(task_context, resource_constraints, preferences)
            .await?;
        persist_trace_record(&trace).await?;
    }

    let what_if_result = if let Some(ref w) = options.what_if_constraints {
        Some(
            scheduler
                .adaptive_memory_selection_trace(task_context, w, preferences)
                .await?
                .final_result,
        )
    } else {
        None
    };

    Ok(SelectionOutcome {
        final_result: result,
        trace: None,
        what_if_result,
    })
}

pub async fn select_memory_trace(
    scheduler: &AdaptiveMemoryScheduler,
    task_context: &TaskContext,
    resource_constraints: &ResourceConstraints,
    preferences: &TaskPreferences,
    persist_trace: bool,
) -> Result<DecisionTrace, AppError> {
    let trace = scheduler
        .adaptive_memory_selection_trace(task_context, resource_constraints, preferences)
        .await?;
    if persist_trace {
        persist_trace_record(&trace).await?;
    }
    Ok(trace)
}

pub async fn list_decision_traces(
    task_id: Option<&str>,
    limit: Option<i32>,
) -> Result<Vec<DecisionTraceEnvelope>, AppError> {
    let rows = if let Some(task_id) = task_id {
        DecisionTraceRepository::get_by_task_id(task_id, limit).await?
    } else {
        DecisionTraceRepository::get_recent(limit, None, None).await?
    };

    let mut traces = Vec::with_capacity(rows.len());
    for row in rows {
        let trace: DecisionTrace = serde_json::from_str(&row.trace_json)
            .map_err(|e| AppError::Internal(format!("Failed to parse stored trace: {}", e)))?;
        traces.push(DecisionTraceEnvelope {
            trace_id: row.trace_id,
            task_id: row.task_id,
            created_at: row.created_at,
            trace,
        });
    }
    Ok(traces)
}

async fn persist_trace_record(trace: &DecisionTrace) -> Result<PersistedTraceArtifacts, AppError> {
    let trace_json = serde_json::to_string(trace)
        .map_err(|e| AppError::Internal(format!("Failed to serialize trace: {}", e)))?;
    let trace_id = DecisionTraceRepository::create(&trace.task_id, &trace_json).await?;
    let evidence = record_decision_trace_as_evidence(trace).await?;
    Ok(PersistedTraceArtifacts {
        trace_id,
        workflow_id: evidence.run.workflow_id,
        run_id: evidence.run.run_id,
        verified: evidence.verification.verified,
    })
}

async fn persist_from_trace(
    task_context: &TaskContext,
    trace: &DecisionTrace,
) -> Result<(), AppError> {
    let r = &trace.final_result;
    let _config_id = MemoryConfigRepository::create(
        &task_context.user_id,
        &task_context.agent_id,
        &format!("Config for task {}", trace.task_id),
        "optimized",
        &r.memory_config,
    )
    .await?;

    // History persistence failure should not fail selection success path.
    if let Err(e) = WeightHistoryRepository::create(
        &trace.task_id,
        &trace.initial_memory_config.memory_weights,
        &trace.weight_adjustment.adjusted_weights,
        &trace.weight_adjustment.adjustment_reasons,
        ((trace.cost_benefit_ratio - 1.0) * 0.1) as f32,
        None,
    )
    .await
    {
        tracing::warn!(task_id = %trace.task_id, error = %e, "failed to persist weight history");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Modality, ReasoningDepth, TaskType, TemporalScope};

    fn sample_task_context() -> TaskContext {
        TaskContext {
            task_id: "what-if-failure-test".to_string(),
            task_type: TaskType::Task,
            complexity: 0.7,
            modality_requirements: vec![Modality::Text],
            temporal_scope: TemporalScope::Medium,
            reasoning_depth: ReasoningDepth::Medium,
            context_dependency: 0.5,
            user_id: "u_test".to_string(),
            agent_id: "a_test".to_string(),
        }
    }

    fn base_constraints() -> ResourceConstraints {
        ResourceConstraints {
            max_memory_usage_mb: 1024,
            max_cpu_usage_percent: 80,
            max_response_time_ms: 2000,
            storage_quota_percent: 90,
        }
    }

    fn impossible_constraints() -> ResourceConstraints {
        ResourceConstraints {
            max_memory_usage_mb: 64,
            max_cpu_usage_percent: 10,
            max_response_time_ms: 200,
            storage_quota_percent: 5,
        }
    }

    fn default_preferences() -> TaskPreferences {
        TaskPreferences {
            prioritize_efficiency: true,
            prioritize_coherence: false,
            enable_multimodal: true,
            enable_reasoning: true,
        }
    }

    #[tokio::test]
    async fn test_select_memory_returns_error_when_what_if_fails() {
        let scheduler = AdaptiveMemoryScheduler::new();
        let result = select_memory(
            &scheduler,
            &sample_task_context(),
            &base_constraints(),
            &default_preferences(),
            SelectionOptions {
                explain: false,
                dry_run: true,
                persist_trace: false,
                what_if_constraints: Some(impossible_constraints()),
            },
        )
        .await;

        assert!(result.is_err());
    }
}
