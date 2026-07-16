//! A2A Protocol Integration Tests
#![cfg(feature = "a2a")]

use axum::{
    body::{self, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

use backend::a2a::{a2a_router, handler::A2AHandler};
use std::sync::Arc;

fn create_test_router() -> Router {
    let handler = Arc::new(A2AHandler::new());
    a2a_router("http://localhost:8008".to_string(), handler)
}

#[tokio::test]
async fn test_agent_card_endpoint() {
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

#[tokio::test]
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
