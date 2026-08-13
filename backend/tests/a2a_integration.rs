//! A2A Protocol Integration Tests

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

fn authenticated_request() -> axum::http::request::Builder {
    authenticated_request_for("a2a-test-tenant")
}

fn authenticated_request_for(tenant_id: &str) -> axum::http::request::Builder {
    backend::config::init();
    let (token, _) = backend::hoops::jwt::get_token(tenant_id).expect("generate A2A test JWT");
    Request::builder().header("authorization", format!("Bearer {token}"))
}

#[tokio::test]
async fn test_a2a_operations_require_a_jwt() {
    backend::config::init();
    let app = create_test_router();
    let request_body = json!({
        "message": {
            "messageId": "anonymous-a2a-request",
            "role": "ROLE_USER",
            "parts": [{ "text": "Search for memories" }]
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

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_agent_card_advertises_mounted_api_endpoints_without_wildcard_host() {
    let handler = Arc::new(A2AHandler::new());
    let app = a2a_router("http://0.0.0.0:8008/api".to_string(), handler);

    let response = app
        .clone()
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
    let urls: Vec<&str> = card["supportedInterfaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|interface| interface["url"].as_str().unwrap())
        .collect();
    assert_eq!(
        urls,
        vec![
            "http://127.0.0.1:8008/api/a2a/jsonrpc",
            "http://127.0.0.1:8008/api/a2a/rest/messages",
        ]
    );
    assert!(urls.iter().all(|url| !url.contains("0.0.0.0")));
}

#[tokio::test]
async fn test_jsonrpc_send_message_returns_protocol_error_for_unsupported_operation() {
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
                        "text": "Compose a limerick about a lighthouse"
                    }
                ]
            }
        },
        "id": 1
    });

    let response = app
        .oneshot(
            authenticated_request()
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
    assert!(result["result"].is_null());
    assert_eq!(result["error"]["code"], -32000);
    assert!(result["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Unsupported A2A operation"));
}

#[tokio::test]
async fn test_jsonrpc_status_request_returns_protocol_error_when_status_is_not_advertised() {
    let app = create_test_router();
    let request_body = json!({
        "jsonrpc": "2.0",
        "method": "message/send",
        "params": {
            "message": {
                "messageId": "test-status-1",
                "role": "ROLE_USER",
                "parts": [{ "text": "Check memory system status" }]
            }
        },
        "id": 2
    });

    let response = app
        .oneshot(
            authenticated_request()
                .method("POST")
                .uri("/a2a/jsonrpc")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(result["result"].is_null());
    assert_eq!(result["error"]["code"], -32000);
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
            authenticated_request()
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
async fn test_rest_memory_store_returns_error_when_backing_services_are_unavailable() {
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
            authenticated_request()
                .method("POST")
                .uri("/a2a/rest/messages")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(result["error"]["code"], -32000);
    assert!(result["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Memory services are not initialized"));
}

#[tokio::test]
async fn test_get_task_returns_not_found_for_unknown_id() {
    let app = create_test_router();

    let response = app
        .oneshot(
            authenticated_request()
                .uri("/a2a/rest/tasks/test-task-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_task_list_and_get_return_the_same_stored_failed_task() {
    let app = create_test_router();

    let rejected_request = json!({
        "jsonrpc": "2.0",
        "method": "message/send",
        "params": {
            "message": {
                "messageId": "test-rejected-task",
                "role": "ROLE_USER",
                "parts": [{ "text": "Compose a limerick about a lighthouse" }]
            }
        },
        "id": 3
    });

    let rejected_response = app
        .clone()
        .oneshot(
            authenticated_request()
                .method("POST")
                .uri("/a2a/jsonrpc")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&rejected_request).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let rejected_body = body::to_bytes(rejected_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let rejected: Value = serde_json::from_slice(&rejected_body).unwrap();
    assert_eq!(rejected["error"]["code"], -32000);

    let response = app
        .clone()
        .oneshot(
            authenticated_request()
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

    let tasks = result["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    let task_id = tasks[0]["id"].as_str().unwrap();
    assert_eq!(tasks[0]["status"]["state"], "TASK_STATE_FAILED");

    let response = app
        .clone()
        .oneshot(
            authenticated_request()
                .uri(format!("/a2a/rest/tasks/{task_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let task: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(task["id"], task_id);
    assert_eq!(task["status"]["state"], "TASK_STATE_FAILED");

    let rpc_response = app
        .clone()
        .oneshot(
            authenticated_request()
                .method("POST")
                .uri("/a2a/jsonrpc")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "method": "task/get",
                        "params": { "id": task_id },
                        "id": 4,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let rpc_body = body::to_bytes(rpc_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let rpc_task: Value = serde_json::from_slice(&rpc_body).unwrap();
    assert_eq!(rpc_task["result"]["id"], task_id);
    assert_eq!(rpc_task["result"]["status"]["state"], "TASK_STATE_FAILED");

    let rpc_list_response = app
        .oneshot(
            authenticated_request()
                .method("POST")
                .uri("/a2a/jsonrpc")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "method": "task/list",
                        "params": {},
                        "id": 5,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let rpc_list_body = body::to_bytes(rpc_list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let rpc_list: Value = serde_json::from_slice(&rpc_list_body).unwrap();
    assert_eq!(rpc_list["result"]["tasks"][0]["id"], task_id);
    assert_eq!(
        rpc_list["result"]["tasks"][0]["status"]["state"],
        "TASK_STATE_FAILED"
    );
}

#[tokio::test]
async fn test_task_lookup_does_not_cross_tenant_boundaries() {
    let app = create_test_router();
    let rejected_request = json!({
        "jsonrpc": "2.0",
        "method": "message/send",
        "params": {
            "message": {
                "messageId": "tenant-scoped-task",
                "role": "ROLE_USER",
                "parts": [{ "text": "Compose a limerick about a lighthouse" }]
            }
        },
        "id": 6
    });

    let rejected_response = app
        .clone()
        .oneshot(
            authenticated_request_for("tenant-a")
                .method("POST")
                .uri("/a2a/jsonrpc")
                .header("content-type", "application/json")
                .body(Body::from(rejected_request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let rejected_body = body::to_bytes(rejected_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let rejected: Value = serde_json::from_slice(&rejected_body).unwrap();

    let owner_tasks = app
        .clone()
        .oneshot(
            authenticated_request_for("tenant-a")
                .uri("/a2a/rest/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let owner_body = body::to_bytes(owner_tasks.into_body(), usize::MAX)
        .await
        .unwrap();
    let owner_tasks: Value = serde_json::from_slice(&owner_body).unwrap();
    let task_id = owner_tasks["tasks"][0]["id"].as_str().unwrap().to_string();

    let other_tenant_response = app
        .oneshot(
            authenticated_request_for("tenant-b")
                .uri(format!("/a2a/rest/tasks/{task_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(rejected["error"]["code"], -32000);
    assert_eq!(other_tenant_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_streaming_endpoint_uses_one_task_id_and_emits_final_response_payload() {
    let app = create_test_router();

    let request_body = json!({
        "message": {
            "messageId": "test-stream-1",
            "role": "ROLE_USER",
            "parts": [
                {
                        "text": "Compose a limerick about a lighthouse"
                }
            ]
        }
    });

    let response = app
        .oneshot(
            authenticated_request()
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

    let body = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let events: Vec<Value> = std::str::from_utf8(&body)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str(data).unwrap())
        .collect();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["taskId"], events[1]["taskId"]);
    assert_eq!(events[1]["final"], true);
    assert_eq!(events[1]["response"]["task"]["id"], events[0]["taskId"]);
    assert_eq!(
        events[1]["response"]["task"]["status"]["state"],
        "TASK_STATE_FAILED"
    );
}
