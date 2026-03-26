use std::path::PathBuf;
use std::sync::OnceLock;

use backend::config::{DatabaseBackend, DbConfig};
use backend::models::{
    Modality, ReasoningDepth, ResourceConstraints, TaskContext, TaskPreferences, TaskType,
    TemporalScope,
};
use backend::services::evidence_graph::{record_decision_trace_as_evidence, verify_chain};
use backend::services::AdaptiveMemoryScheduler;
use serde_json::json;

static DB_PATH: OnceLock<String> = OnceLock::new();
static INIT_DB: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

async fn init_test_db() {
    let db_path = DB_PATH
        .get_or_init(|| {
            let mut path = std::env::temp_dir();
            path.push(format!(
                "adaptive-memory-hash-chain-{}.db",
                std::process::id()
            ));
            if path.exists() {
                let _ = std::fs::remove_file(&path);
            }
            path_to_string(path)
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
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
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
        user_id: "u_hash_chain".to_string(),
        agent_id: "a_hash_chain".to_string(),
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
async fn verify_chain_accepts_the_unmodified_persisted_nodes() {
    init_test_db().await;

    let trace = sample_trace("workflow-hash-chain-valid").await;
    let recorded = record_decision_trace_as_evidence(&trace)
        .await
        .expect("persist evidence graph");
    let verification = verify_chain(
        &recorded.run.workflow_id,
        &recorded.run.run_id,
        &recorded.nodes,
    )
    .expect("verify unmodified chain");

    assert!(verification.verified);
    assert!(verification.violations.is_empty());
    assert_eq!(verification.expected_node_count, recorded.nodes.len());
}

#[tokio::test]
async fn verify_chain_rejects_prev_hash_and_node_hash_tampering() {
    init_test_db().await;

    let trace = sample_trace("workflow-hash-chain-prev-hash").await;
    let recorded = record_decision_trace_as_evidence(&trace)
        .await
        .expect("persist evidence graph");

    let mut tampered_prev_hash = recorded.nodes.clone();
    tampered_prev_hash[1].prev_hash = Some("tampered-prev-hash".to_string());
    let verification = verify_chain(
        &recorded.run.workflow_id,
        &recorded.run.run_id,
        &tampered_prev_hash,
    )
    .expect("verify tampered prev hash");
    assert!(!verification.verified);
    assert!(verification.violations.iter().any(|item| item.contains("prev_hash")));

    let mut tampered_node_hash = recorded.nodes.clone();
    tampered_node_hash[1].node_hash = "tampered-node-hash".to_string();
    let verification = verify_chain(
        &recorded.run.workflow_id,
        &recorded.run.run_id,
        &tampered_node_hash,
    )
    .expect("verify tampered node hash");
    assert!(!verification.verified);
    assert!(verification.violations.iter().any(|item| item.contains("node_hash")));
}

#[tokio::test]
async fn verify_chain_rejects_context_snapshot_byte_changes() {
    init_test_db().await;

    let trace = sample_trace("workflow-hash-chain-context").await;
    let recorded = record_decision_trace_as_evidence(&trace)
        .await
        .expect("persist evidence graph");

    let mut tampered_snapshot = recorded.nodes.clone();
    tampered_snapshot[0]
        .context_snapshot
        .insert("context_snapshot".to_string(), json!("tampered"));
    let verification = verify_chain(
        &recorded.run.workflow_id,
        &recorded.run.run_id,
        &tampered_snapshot,
    )
    .expect("verify tampered snapshot");

    assert!(!verification.verified);
    assert!(verification
        .violations
        .iter()
        .any(|item| item.contains("canonical")));
}
