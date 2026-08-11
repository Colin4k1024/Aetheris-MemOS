//! A2A Protocol Integration Tests
#![cfg(feature = "a2a")]

use axum::{
    body::{self, Body},
    http::{header, Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

use backend::a2a::{a2a_router, handler::A2AHandler};
use std::sync::Arc;

static CONFIG_INIT: std::sync::Once = std::sync::Once::new();

/// Initialise the process-wide config exactly once.
///
/// The protected A2A routes now sit behind `hoops::jwt::auth_middleware`, which
/// reads `config::get().jwt`, and `get_token` signs with that same secret — so
/// both the middleware and the token minted below need an initialised config.
/// Mirrors the `ensure_config` helper in `tests/evidence_api.rs`. The public
/// agent-card test deliberately does NOT call this: it must work with no config
/// and no token, proving discovery stays unauthenticated.
fn ensure_config() {
    CONFIG_INIT.call_once(|| {
        backend::config::init();
    });
}

/// A valid `Authorization: Bearer <jwt>` header for the protected A2A surface.
///
/// Minted with the same secret `auth_middleware` validates against, so it
/// authenticates and yields a `RequestTenantContext` whether or not the loaded
/// config has `jwt.disabled` set — if disabled, the middleware injects an
/// anonymous context and ignores the token; if enabled (the committed
/// `config.toml` default), the token is what gets the request past auth.
fn auth_header() -> String {
    ensure_config();
    let (token, _) = backend::hoops::jwt::get_token("a2a-test-agent").expect("generate test JWT");
    format!("Bearer {token}")
}

fn create_test_router() -> Router {
    let handler = Arc::new(A2AHandler::new());
    a2a_router("http://localhost:8008".to_string(), handler)
}

#[tokio::test]
async fn test_agent_card_endpoint() {
    // Public discovery endpoint — intentionally no auth header. If this ever
    // starts requiring a token, agent-to-agent discovery is broken.
    let app = create_test_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/.well-known/agent-card.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let card: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(card["name"], "Aetheris MemOS");
    assert!(card["skills"].is_array());
    assert!(card["skills"].as_array().unwrap().len() >= 5);
}

/// The protected surface must reject an unauthenticated request. This is the
/// guard that the fix is *real* auth — not merely "inject an extension so the
/// extractor stops 500ing". Without a token the request must never reach a
/// handler. Skipped only when the loaded config has auth disabled (dev mode),
/// where the middleware injects an anonymous context by design; the committed
/// `config.toml` (probed before `local.toml`) has auth enabled, which is where
/// this bites.
#[tokio::test]
async fn test_protected_endpoint_rejects_missing_token() {
    ensure_config();
    if backend::config::get().jwt.disabled {
        return;
    }

    let app = create_test_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/a2a/rest/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_jsonrpc_send_message() {
    let app = create_test_router();

    let request_body = json!({
        "jsonrpc": "2.0",
        "method": "message/send",
        "params": {
            "message": {
                "messageId": "test-msg-1",
                "role": "ROLE_USER",
                "parts": [
                    {
                        "text": "Search for memories about AI"
                    }
                ]
            }
        },
        "id": 1
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/a2a/jsonrpc")
                .header("content-type", "application/json")
                .header(header::AUTHORIZATION, auth_header())
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(result["jsonrpc"], "2.0");
    // Check if result exists (could be task or error)
    assert!(result["result"].is_object() || result["error"].is_object());
}

#[tokio::test]
async fn test_jsonrpc_invalid_method() {
    let app = create_test_router();

    let request_body = json!({
        "jsonrpc": "2.0",
        "method": "invalid/method",
        "params": {},
        "id": 1
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/a2a/jsonrpc")
                .header("content-type", "application/json")
                .header(header::AUTHORIZATION, auth_header())
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(result["jsonrpc"], "2.0");
    assert!(result["error"].is_object());
    assert_eq!(result["error"]["code"], -32601);
}

// NOTE (A-4b): this test asserts a *successful* store (`task`/`message` object),
// which requires `handle_memory_store` -> `store_ltm_for_tenant` to complete.
// That path has a hard embedding dependency (services/memory_storage.rs): the
// vector is generated on the write hot path and cannot be degraded away, so the
// store fails with 500 whenever no embedding backend is reachable. It therefore
// cannot pass in the pure in-process harness (no Ollama/PG), unlike the other
// message tests which tolerate a backend error. Marked `#[ignore]` rather than
// weakening the assertion — run it with `--ignored` in an e2e job that provides
// a real embedding backend (see tests/memory_platform_e2e.rs for the pattern).
#[tokio::test]
#[ignore = "requires a reachable embedding backend (+DB): handle_memory_store performs a real LTM write; run with --ignored under e2e infra"]
async fn test_rest_send_message() {
    let app = create_test_router();

    let request_body = json!({
        "message": {
            "messageId": "test-msg-2",
            "role": "ROLE_USER",
            "parts": [
                {
                    "text": "Remember this fact"
                }
            ]
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/a2a/rest/messages")
                .header("content-type", "application/json")
                .header(header::AUTHORIZATION, auth_header())
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(result["task"].is_object() || result["message"].is_object());
}

#[tokio::test]
async fn test_get_task() {
    let app = create_test_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/a2a/rest/tasks/test-task-123")
                .header(header::AUTHORIZATION, auth_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(result["id"], "test-task-123");
    assert!(result["status"].is_object());
}

#[tokio::test]
async fn test_list_tasks() {
    let app = create_test_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/a2a/rest/tasks")
                .header(header::AUTHORIZATION, auth_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(result["tasks"].is_array());
}

#[tokio::test]
async fn test_streaming_endpoint() {
    let app = create_test_router();

    let request_body = json!({
        "message": {
            "messageId": "test-stream-1",
            "role": "ROLE_USER",
            "parts": [
                {
                    "text": "Search for memories"
                }
            ]
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/a2a/rest/messages/stream")
                .header("content-type", "application/json")
                .header("accept", "text/event-stream")
                .header(header::AUTHORIZATION, auth_header())
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
}
