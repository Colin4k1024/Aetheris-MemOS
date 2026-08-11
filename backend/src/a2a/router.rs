use axum::{
    extract::{Extension, State},
    http::StatusCode,
    middleware,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::tenant::RequestTenantContext;

use super::agent_card::create_agent_card;
use super::handler::A2AHandler;

#[derive(Clone)]
pub struct A2AState {
    pub handler: Arc<A2AHandler>,
    pub base_url: String,
}

pub fn a2a_router(base_url: String, handler: Arc<A2AHandler>) -> Router {
    let state = A2AState {
        handler,
        base_url: base_url.clone(),
    };

    // Public discovery endpoint. The A2A agent card is the well-known capability
    // descriptor other agents fetch *before* authenticating — per the A2A spec it
    // also advertises the agent's security schemes, i.e. it is how a caller learns
    // how to authenticate. Gating it behind auth would be a chicken-and-egg break
    // of discovery, so it stays unauthenticated on purpose — exactly like MCP's
    // public `/initialize` (routers/mcp.rs).
    let public_router = Router::new().route("/.well-known/agent-card.json", get(get_agent_card));

    // Protected agent-interop surface. Every handler here reads
    // `Extension<RequestTenantContext>`, which `auth_middleware` injects only
    // after a valid JWT (ADR-0007: A2A message/stream endpoints reuse the *same*
    // axum `auth_middleware` as REST/MCP, converging on the transport-agnostic
    // `authenticate()` core). Without this layer the extractor fails 500 and,
    // far worse, these endpoints would run unauthenticated with no tenant
    // isolation. `auth` is applied as the outer `.layer` so it runs first —
    // mirrors `routers/mcp.rs`.
    let protected_router = Router::new()
        .route("/a2a/jsonrpc", post(handle_jsonrpc))
        .route("/a2a/rest/messages", post(handle_rest_message))
        .route("/a2a/rest/messages/stream", post(handle_stream_message))
        .route("/a2a/rest/tasks/{task_id}", get(handle_get_task))
        .route("/a2a/rest/tasks", get(handle_list_tasks))
        .layer(middleware::from_fn(crate::hoops::jwt::auth_middleware));

    public_router.merge(protected_router).with_state(state)
}

async fn get_agent_card(State(state): State<A2AState>) -> Json<Value> {
    let card = create_agent_card(&state.base_url);
    Json(serde_json::to_value(card).unwrap_or_else(|_| json!({})))
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    result: Option<Value>,
    error: Option<JsonRpcError>,
    id: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<Value>,
}

async fn handle_jsonrpc(
    State(state): State<A2AState>,
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(request): Json<JsonRpcRequest>,
) -> Result<Json<JsonRpcResponse>, StatusCode> {
    if request.jsonrpc != "2.0" {
        return Ok(Json(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32600,
                message: "Invalid Request".to_string(),
                data: None,
            }),
            id: request.id,
        }));
    }

    match request.method.as_str() {
        "message/send" => {
            let response = handle_send_message(state, request.params, &tenant_ctx).await;
            match response {
                Ok(result) => Ok(Json(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(result),
                    error: None,
                    id: request.id,
                })),
                Err(e) => Ok(Json(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32603,
                        message: e,
                        data: None,
                    }),
                    id: request.id,
                })),
            }
        }
        "task/get" => {
            let response = handle_get_task_rpc(state, request.params).await;
            match response {
                Ok(result) => Ok(Json(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(result),
                    error: None,
                    id: request.id,
                })),
                Err(e) => Ok(Json(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32603,
                        message: e,
                        data: None,
                    }),
                    id: request.id,
                })),
            }
        }
        _ => Ok(Json(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
            id: request.id,
        })),
    }
}

async fn handle_send_message(
    state: A2AState,
    params: Option<Value>,
    tenant_ctx: &RequestTenantContext,
) -> Result<Value, String> {
    let params = params.ok_or("Missing parameters")?;
    let request: a2a::types::SendMessageRequest =
        serde_json::from_value(params).map_err(|e| format!("Invalid request: {}", e))?;

    let response = state.handler.handle_message(request, tenant_ctx).await?;
    serde_json::to_value(response).map_err(|e| format!("Serialization error: {}", e))
}

async fn handle_get_task_rpc(_state: A2AState, params: Option<Value>) -> Result<Value, String> {
    let params = params.ok_or("Missing parameters")?;
    let task_id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing task ID")?;

    Ok(json!({
        "id": task_id,
        "status": {
            "state": "completed",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }
    }))
}

async fn handle_rest_message(
    State(state): State<A2AState>,
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(request): Json<a2a::types::SendMessageRequest>,
) -> Result<Json<Value>, StatusCode> {
    match state.handler.handle_message(request, &tenant_ctx).await {
        Ok(response) => Ok(Json(
            serde_json::to_value(response).unwrap_or_else(|_| json!({})),
        )),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn handle_get_task(
    State(_state): State<A2AState>,
    axum::extract::Path(task_id): axum::extract::Path<String>,
) -> Json<Value> {
    Json(json!({
        "id": task_id,
        "status": {
            "state": "completed",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }
    }))
}

async fn handle_list_tasks(State(_state): State<A2AState>) -> Json<Value> {
    Json(json!({
        "tasks": []
    }))
}

/// One server-sent event as plain data — its optional SSE `event:` name and its
/// `data:` JSON line. Kept separate from axum's opaque `Event` (which exposes no
/// getters) so the response→payload mapping in `success_payload` stays
/// unit-testable without spinning up a handler or a memory backend.
struct StreamPayload {
    event: Option<&'static str>,
    data: String,
}

/// Map a successful handler reply to the terminal SSE payload the caller receives.
///
/// This is the fix for E-12. The streaming endpoint used to bind the handler's
/// `Ok(response)` and then ignore it, always emitting a hardcoded "completed"
/// that carried a *freshly minted* task id — so every SSE client got the same
/// empty envelope no matter what the agent produced. We now surface a `Task`
/// reply under its own id/context and forward a bare `Message` reply verbatim,
/// matching the non-streaming `/a2a/rest/messages` handler.
fn success_payload(response: a2a::types::SendMessageResponse) -> StreamPayload {
    match response {
        a2a::types::SendMessageResponse::Task(task) => {
            let completed_event = json!({
                "taskId": task.id,
                "contextId": task.context_id,
                "status": {
                    "state": "completed",
                    "timestamp": chrono::Utc::now().to_rfc3339()
                },
                "final": true
            });
            StreamPayload {
                event: None,
                data: completed_event.to_string(),
            }
        }
        a2a::types::SendMessageResponse::Message(msg) => StreamPayload {
            event: Some("message"),
            data: serde_json::to_string(&msg).unwrap_or_default(),
        },
    }
}

async fn handle_stream_message(
    State(state): State<A2AState>,
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(request): Json<a2a::types::SendMessageRequest>,
) -> axum::response::Sse<
    impl futures::stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
> {
    use axum::response::sse::Event;

    let handler = state.handler.clone();

    let stream = async_stream::stream! {
        // Emit an immediate "working" status so the caller sees the task was
        // accepted before the (potentially slow) handler runs. These ids are
        // provisional — the terminal event below carries the handler's real
        // task identity.
        let task_id = uuid::Uuid::new_v4().to_string();
        let context_id = request.message.context_id.clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let working_event = json!({
            "taskId": task_id,
            "contextId": context_id,
            "status": {
                "state": "working",
                "timestamp": chrono::Utc::now().to_rfc3339()
            },
            "final": false
        });

        yield Ok(Event::default().data(working_event.to_string()));

        // Process the message with the real tenant context, then stream back
        // whatever the handler actually produced (see `success_payload`).
        match handler.handle_message(request, &tenant_ctx).await {
            Ok(response) => {
                let payload = success_payload(response);
                let mut event = Event::default();
                if let Some(name) = payload.event {
                    event = event.event(name);
                }
                yield Ok(event.data(payload.data));
            }
            Err(e) => {
                let error_event = json!({
                    "taskId": task_id,
                    "contextId": context_id,
                    "status": {
                        "state": "failed",
                        "message": format!("Error: {}", e),
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    },
                    "final": true
                });

                yield Ok(Event::default().data(error_event.to_string()));
            }
        }
    };

    axum::response::Sse::new(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2a::types::{Message, Part, Role, SendMessageResponse, Task, TaskState, TaskStatus};

    // E-12 regression lock. The streaming success branch used to discard the
    // handler's response and emit a hardcoded "completed" with a fabricated
    // task id. These tests pin the response→payload mapping so a regression to
    // "ignore the response" fails here. They need no memory backend — unlike an
    // end-to-end stream test, whose Ok path requires a reachable embedding/DB
    // backend (see the #[ignore] test in tests/a2a_integration.rs).

    #[test]
    fn success_payload_task_preserves_handler_task_identity() {
        let task = Task {
            id: "real-task-id".to_string(),
            context_id: "real-context-id".to_string(),
            status: TaskStatus {
                state: TaskState::Completed,
                message: None,
                timestamp: None,
            },
            artifacts: None,
            history: None,
            metadata: None,
        };

        let payload = success_payload(SendMessageResponse::Task(task));

        // Default (unnamed) event carrying the task's OWN id/context — not the
        // provisional working-event uuid the buggy version re-emitted.
        assert!(payload.event.is_none());
        let data: Value = serde_json::from_str(&payload.data).unwrap();
        assert_eq!(data["taskId"], "real-task-id");
        assert_eq!(data["contextId"], "real-context-id");
        assert_eq!(data["status"]["state"], "completed");
        assert_eq!(data["final"], true);
    }

    #[test]
    fn success_payload_message_is_forwarded_verbatim() {
        let msg = Message {
            message_id: "msg-42".to_string(),
            context_id: Some("ctx-1".to_string()),
            task_id: None,
            role: Role::Agent,
            parts: vec![Part::text("hello from agent".to_string())],
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        };

        let payload = success_payload(SendMessageResponse::Message(msg));

        // A bare Message reply reaches the client under the `message` event with
        // the serialized body — the buggy version dropped it entirely.
        assert_eq!(payload.event, Some("message"));
        assert!(
            payload.data.contains("hello from agent"),
            "serialized message must carry the agent's text, got: {}",
            payload.data
        );
    }
}
