use std::sync::OnceLock;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use backend::config::{DatabaseBackend, DbConfig};
use backend::models::{
    Modality, ReasoningDepth, ResourceConstraints, TaskContext, TaskPreferences, TaskType,
    TemporalScope,
};
use backend::services::evidence_graph::record_decision_trace_as_evidence;
use backend::services::AdaptiveMemoryScheduler;
use serde_json::Value;
use tower::ServiceExt;

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
        .get_or_init(|| "file:evidence-api-tests?mode=memory&cache=shared".to_string())
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
        user_id: "u_evidence_api".to_string(),
        agent_id: "a_evidence_api".to_string(),
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
async fn get_workflow_evidence_returns_workflow_metadata_nodes_edges_and_verification() {
    let _guard = test_guard();
    init_test_db().await;

    let trace = sample_trace("workflow-evidence-api-success").await;
    let recorded = record_decision_trace_as_evidence(&trace)
        .await
        .expect("persist evidence graph");
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/workflows/{}/evidence", trace.task_id))
                .body(Body::empty())
                .expect("build evidence request"),
        )
        .await
        .expect("serve evidence request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read evidence response");
    let json: Value = serde_json::from_slice(&body).expect("parse evidence response");

    assert_eq!(json["run"]["workflow_id"], trace.task_id);
    assert_eq!(json["run"]["attempt_id"], recorded.run.attempt_id);
    assert!(json["nodes"]
        .as_array()
        .is_some_and(|nodes| !nodes.is_empty()));
    assert!(json["edges"]
        .as_array()
        .is_some_and(|edges| !edges.is_empty()));
    assert_eq!(json["verification"]["verified"], Value::Bool(true));
    assert_eq!(
        json["verification"]["root_hash"],
        Value::String(recorded.verification.root_hash.unwrap())
    );
}

#[tokio::test]
async fn get_workflow_evidence_returns_app_error_not_found_payload() {
    let _guard = test_guard();
    init_test_db().await;

    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/workflows/missing-workflow/evidence")
                .body(Body::empty())
                .expect("build missing evidence request"),
        )
        .await
        .expect("serve missing evidence request");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read missing evidence response");
    let body_text = String::from_utf8(body.to_vec()).expect("utf8 error body");
    assert_ne!(body_text.trim(), "{}");
    assert_ne!(body_text.trim(), "[]");

    let json: Value = serde_json::from_str(&body_text).expect("parse app error payload");
    assert_eq!(json["code"], Value::from(1004));
    assert_eq!(
        json["message"],
        Value::String("workflow evidence not found: missing-workflow".to_string())
    );
    assert_eq!(json["error"], json["message"]);
}

#[tokio::test]
async fn openapi_includes_get_workflow_evidence_path() {
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api-doc/openapi.json")
                .body(Body::empty())
                .expect("build openapi request"),
        )
        .await
        .expect("serve openapi request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read openapi response");
    let json: Value = serde_json::from_slice(&body).expect("parse openapi response");

    assert!(json["paths"]["/api/v1/workflows/{id}/evidence"]["get"].is_object());
}
