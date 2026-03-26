use std::sync::OnceLock;

use backend::config::{DatabaseBackend, DbConfig};
use backend::models::{
    Modality, ReasoningDepth, ResourceConstraints, TaskContext, TaskPreferences, TaskType,
    TemporalScope,
};
use backend::services::evidence_graph::{list_workflow_evidence, record_decision_trace_as_evidence};
use backend::services::AdaptiveMemoryScheduler;

static DB_PATH: OnceLock<String> = OnceLock::new();
static INIT_DB: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
static TEST_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

async fn init_test_db() {
    let db_path = DB_PATH
        .get_or_init(|| {
            "file:evidence-graph-tests?mode=memory&cache=shared".to_string()
        })
        .clone();

    INIT_DB
        .get_or_init(|| async move {
            backend::db::init(&DbConfig {
                backend: DatabaseBackend::Sqlite,
                url: db_path.clone(),
                path: Some(db_path),
                pool_size: 1,
                min_idle: Some(1),
                tcp_timeout: 5,
                connection_timeout: 5,
                statement_timeout: 5,
                helper_threads: 1,
                enforce_tls: false,
            })
            .await
            .expect("initialize sqlite test database");
        })
        .await;

    sqlx::raw_sql(include_str!("../migrations_sqlite/20260326000100_workflow_evidence_graph.sql"))
        .execute(backend::db::sqlite_pool())
        .await
        .expect("apply evidence graph sqlite schema");
}

fn sample_task_context(task_id: &str) -> TaskContext {
    TaskContext {
        task_id: task_id.to_string(),
        task_type: TaskType::Task,
        complexity: 0.7,
        modality_requirements: vec![Modality::Text],
        temporal_scope: TemporalScope::Medium,
        reasoning_depth: ReasoningDepth::Medium,
        context_dependency: 0.5,
        user_id: "u_evidence".to_string(),
        agent_id: "a_evidence".to_string(),
    }
}

fn sample_constraints() -> ResourceConstraints {
    ResourceConstraints {
        max_memory_usage_mb: 1024,
        max_cpu_usage_percent: 80,
        max_response_time_ms: 2000,
        storage_quota_percent: 90,
    }
}

fn sample_preferences() -> TaskPreferences {
    TaskPreferences {
        prioritize_efficiency: true,
        prioritize_coherence: false,
        enable_multimodal: true,
        enable_reasoning: true,
    }
}

async fn sample_trace(task_id: &str) -> backend::services::scheduler::DecisionTrace {
    AdaptiveMemoryScheduler::new()
        .adaptive_memory_selection_trace(
            &sample_task_context(task_id),
            &sample_constraints(),
            &sample_preferences(),
        )
        .await
        .expect("build scheduler decision trace")
}

#[tokio::test]
async fn record_decision_trace_as_evidence_persists_locked_fields() {
    let _guard = TEST_LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap();
    init_test_db().await;

    let trace = sample_trace("workflow-evidence-locked-fields").await;
    let recorded = record_decision_trace_as_evidence(&trace)
        .await
        .expect("persist evidence graph");

    assert_eq!(recorded.run.workflow_id, trace.task_id);
    assert!(!recorded.run.run_id.is_empty());
    assert!(recorded.verification.verified);
    assert!(!recorded.nodes.is_empty());
    assert_eq!(
        recorded
            .nodes
            .iter()
            .map(|node| node.sequence_number)
            .collect::<Vec<_>>(),
        (0..recorded.nodes.len() as i64).collect::<Vec<_>>()
    );
    assert!(recorded.nodes.iter().all(|node| {
        !node.attempt_id.is_empty()
            && !node.timestamp.is_empty()
            && !node.llm_input_hash.is_empty()
            && !node.llm_output_hash.is_empty()
            && !node.tool_invocations.is_empty()
            && !node.context_snapshot.is_empty()
            && !node.node_hash.is_empty()
    }));
}

#[tokio::test]
async fn list_workflow_evidence_returns_nodes_and_edges_in_sequence_order() {
    let _guard = TEST_LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap();
    init_test_db().await;

    let trace = sample_trace("workflow-evidence-listing").await;
    let recorded = record_decision_trace_as_evidence(&trace)
        .await
        .expect("persist evidence graph");
    // This covers the public behavior produced by memory_orchestrator::persist_trace_record.
    let listed = list_workflow_evidence(&trace.task_id)
        .await
        .expect("list workflow evidence");

    assert_eq!(listed.run.run_id, recorded.run.run_id);
    assert_eq!(
        listed
            .nodes
            .iter()
            .map(|node| node.sequence_number)
            .collect::<Vec<_>>(),
        (0..listed.nodes.len() as i64).collect::<Vec<_>>()
    );
    assert!(listed.edges.iter().all(|edge| {
        listed
            .nodes
            .iter()
            .any(|node| node.node_id == edge.source_node_id)
            && listed
                .nodes
                .iter()
                .any(|node| node.node_id == edge.target_node_id)
    }));
    assert_eq!(listed.verification.expected_node_count, listed.nodes.len());
}
