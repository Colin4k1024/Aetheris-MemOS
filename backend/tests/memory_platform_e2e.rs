use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

static CONFIG_INIT: std::sync::Once = std::sync::Once::new();
static DB_INIT: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

fn e2e_enabled() -> bool {
    std::env::var("AMS_E2E").is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

fn ensure_config() {
    CONFIG_INIT.call_once(|| {
        backend::config::init();
    });
}

async fn ensure_db() {
    ensure_config();
    DB_INIT
        .get_or_init(|| async {
            let database_url =
                std::env::var("DATABASE_URL").expect("AMS_E2E requires DATABASE_URL");
            assert!(
                database_url.starts_with("postgres://")
                    || database_url.starts_with("postgresql://"),
                "AMS_E2E requires a PostgreSQL DATABASE_URL"
            );
            backend::db::init(&backend::config::get().db)
                .await
                .expect("initialize E2E database");
        })
        .await;
}

fn auth_header() -> String {
    ensure_config();
    let (token, _) = backend::hoops::jwt::get_token("e2e-user").expect("generate E2E JWT");
    format!("Bearer {token}")
}

async fn request_json(
    method: Method,
    path: &str,
    auth: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let app = backend::axum_routers::create_router();
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, auth);
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }

    let body = body.map_or_else(Body::empty, |value| Body::from(value.to_string()));
    let response = app
        .oneshot(builder.body(body).expect("build E2E request"))
        .await
        .expect("serve E2E request");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read E2E response");
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).to_string()))
    };
    (status, json)
}

#[tokio::test]
async fn environment_backed_memory_platform_flow() {
    if !e2e_enabled() {
        eprintln!("skipping environment E2E test; set AMS_E2E=1 to enable");
        return;
    }

    ensure_db().await;
    let auth = auth_header();

    let (status, stm) = request_json(
        Method::POST,
        "/api/v1/memory/storage/stm",
        &auth,
        Some(json!({
            "userId": "e2e-user",
            "agentId": "e2e-agent",
            "sessionType": "e2e",
            "role": "user",
            "content": "E2E short-term memory message",
            "maxContextLength": 128,
            "retentionHours": 1
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "STM write failed: {stm}");
    let session_id = stm["sessionId"].as_str().expect("sessionId").to_string();

    let (status, messages) = request_json(
        Method::GET,
        &format!("/api/v1/memory/storage/stm/{session_id}"),
        &auth,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "STM read failed: {messages}");
    assert!(messages.as_array().is_some_and(|items| !items.is_empty()));

    let (status, ltm) = request_json(
        Method::POST,
        "/api/v1/memory/storage/ltm",
        &auth,
        Some(json!({
            "sourceId": "e2e-source",
            "sourceType": "user_input",
            "title": "E2E Memory",
            "content": "Adaptive memory systems route agent memories through STM, LTM, KG, and multimodal stores."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "LTM write failed: {ltm}");
    assert!(ltm["entryId"].as_str().is_some());

    let (status, search) = request_json(
        Method::POST,
        "/api/v1/memory/search/hybrid",
        &auth,
        Some(json!({
            "query": "adaptive memory systems",
            "topK": 5,
            "enableRerank": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "hybrid search failed: {search}");
    assert!(search["results"].as_array().is_some());

    let (status, trace) = request_json(
        Method::POST,
        "/api/v1/memory/adaptive/trace",
        &auth,
        Some(json!({
            "task_context": {
                "task_id": "e2e-adaptive-trace",
                "task_type": "task",
                "complexity": 0.6,
                "modality_requirements": ["text"],
                "temporal_scope": "medium",
                "reasoning_depth": "medium",
                "context_dependency": 0.5,
                "user_id": "e2e-user",
                "agent_id": "e2e-agent"
            },
            "resource_constraints": {
                "max_memory_usage_mb": 1024,
                "max_cpu_usage_percent": 80,
                "max_response_time_ms": 2000,
                "storage_quota_percent": 90
            },
            "preferences": {
                "prioritize_efficiency": true,
                "prioritize_coherence": false,
                "enable_multimodal": false,
                "enable_reasoning": true
            },
            "dry_run": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "adaptive trace failed: {trace}");
    assert_eq!(
        trace["task_id"],
        Value::String("e2e-adaptive-trace".to_string())
    );

    let (status, mcp) = request_json(
        Method::POST,
        "/api/mcp/tools/call",
        &auth,
        Some(json!({
            "name": backend::protocol::mcp::TOOL_MEMORY_WRITE,
            "arguments": {
                "layer": "stm",
                "user_id": "e2e-user",
                "agent_id": "e2e-agent",
                "content": "E2E MCP memory write"
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "MCP memory_write failed: {mcp}");
    assert_eq!(mcp["is_error"], Value::Bool(false));

    let (status, backfill) = request_json(
        Method::POST,
        "/api/v1/memory/storage/qdrant/backfill-tenant-metadata",
        &auth,
        Some(json!({ "limit": 25, "offset": 0, "dryRun": true })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "Qdrant backfill dry run failed: {backfill}"
    );
    assert_eq!(backfill["dryRun"], Value::Bool(true));
}
