use std::sync::OnceLock;

use backend::config::{DatabaseBackend, DbConfig};
use backend::models::{
    Modality, ReasoningDepth, ResourceConstraints, TaskContext, TaskPreferences, TaskType,
    TemporalScope,
};
use backend::services::evidence_graph::{
    build_workflow_evidence_export, canonical_export_body_bytes,
    hash_workflow_evidence_export_body, list_workflow_evidence, record_decision_trace_as_evidence,
};
use backend::services::AdaptiveMemoryScheduler;
use chrono::{TimeZone, Utc};
use serde_json::json;

static DB_PATH: OnceLock<String> = OnceLock::new();
static INIT_DB: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
static TEST_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

async fn init_test_db() {
    let db_path = DB_PATH
        .get_or_init(|| "file:snapshot-export-tests?mode=memory&cache=shared".to_string())
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

    sqlx::raw_sql(include_str!(
        "../migrations_sqlite/20260326000100_workflow_evidence_graph.sql"
    ))
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
        user_id: "u_snapshot_export".to_string(),
        agent_id: "a_snapshot_export".to_string(),
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
async fn workflow_evidence_export_canonical_body_is_stable_across_re_exports() {
    let _guard = test_guard();
    init_test_db().await;

    let trace = sample_trace("workflow-snapshot-export-stable").await;
    record_decision_trace_as_evidence(&trace)
        .await
        .expect("persist evidence graph");
    let response = list_workflow_evidence(&trace.task_id)
        .await
        .expect("list workflow evidence");

    let first_export = build_workflow_evidence_export(
        &response,
        Utc.with_ymd_and_hms(2026, 3, 26, 0, 0, 0)
            .single()
            .expect("valid first export timestamp"),
    );
    let second_export = build_workflow_evidence_export(
        &response,
        Utc.with_ymd_and_hms(2026, 3, 26, 0, 10, 0)
            .single()
            .expect("valid second export timestamp"),
    );

    assert_ne!(
        serde_json::to_vec(&first_export).expect("serialize first export"),
        serde_json::to_vec(&second_export).expect("serialize second export")
    );
    assert_eq!(
        canonical_export_body_bytes(&first_export).expect("canonicalize first export body"),
        canonical_export_body_bytes(&second_export).expect("canonicalize second export body")
    );
    assert_eq!(
        hash_workflow_evidence_export_body(&first_export).expect("hash first export body"),
        hash_workflow_evidence_export_body(&second_export).expect("hash second export body")
    );
}

#[tokio::test]
async fn workflow_evidence_export_contains_locked_fields_for_offline_review() {
    let _guard = test_guard();
    init_test_db().await;

    let trace = sample_trace("workflow-snapshot-export-complete").await;
    let recorded = record_decision_trace_as_evidence(&trace)
        .await
        .expect("persist evidence graph");
    let export = build_workflow_evidence_export(
        &recorded,
        Utc.with_ymd_and_hms(2026, 3, 26, 1, 0, 0)
            .single()
            .expect("valid export timestamp"),
    );

    assert_eq!(export.workflow_id, trace.task_id);
    assert_eq!(export.attempt_id, recorded.run.attempt_id);
    assert_eq!(
        export.root_hash,
        recorded
            .verification
            .root_hash
            .clone()
            .expect("recorded root hash")
    );
    assert!(export.chain_verified);
    assert!(!export.nodes.is_empty());
    assert!(!export.edges.is_empty());
    assert!(export.nodes.iter().all(|node| {
        node.attempt_id == export.attempt_id
            && !node.llm_input_hash.is_empty()
            && !node.llm_output_hash.is_empty()
            && !node.tool_invocations.is_empty()
            && !node.context_snapshot.is_empty()
    }));
}

#[tokio::test]
async fn workflow_evidence_export_hash_changes_when_canonical_body_changes() {
    let _guard = test_guard();
    init_test_db().await;

    let trace = sample_trace("workflow-snapshot-export-rehash").await;
    let recorded = record_decision_trace_as_evidence(&trace)
        .await
        .expect("persist evidence graph");
    let export = build_workflow_evidence_export(
        &recorded,
        Utc.with_ymd_and_hms(2026, 3, 26, 2, 0, 0)
            .single()
            .expect("valid export timestamp"),
    );
    let baseline_hash =
        hash_workflow_evidence_export_body(&export).expect("hash baseline export body");

    let mut tampered_export = export.clone();
    tampered_export.nodes[0]
        .context_snapshot
        .insert("context_snapshot".to_string(), json!("tampered"));
    tampered_export.nodes[0].llm_output_hash = "tampered-llm-output-hash".to_string();

    let tampered_hash =
        hash_workflow_evidence_export_body(&tampered_export).expect("hash tampered export body");
    assert_ne!(tampered_hash, baseline_hash);
}
