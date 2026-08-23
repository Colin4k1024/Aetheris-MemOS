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

use super::agent_card::create_agent_card;
use super::handler::A2AHandler;
use crate::tenant::RequestTenantContext;

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

    let protected_routes = Router::new()
        .route("/a2a/jsonrpc", post(handle_jsonrpc))
        .route("/a2a/rest/messages", post(handle_rest_message))
        .route("/a2a/rest/messages/stream", post(handle_stream_message))
        .route("/a2a/rest/tasks/{task_id}", get(handle_get_task))
        .route("/a2a/rest/tasks", get(handle_list_tasks))
        .route_layer(middleware::from_fn(crate::hoops::jwt::auth_middleware));

    Router::new()
        .route("/.well-known/agent-card.json", get(get_agent_card))
        .merge(protected_routes)
        .with_state(state)
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
            let response =
                handle_send_message(state, tenant_ctx.tenant_id.clone(), request.params).await;
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
                        code: -32000,
                        message: e,
                        data: None,
                    }),
                    id: request.id,
                })),
            }
        }
        "task/get" => {
            let response = handle_get_task_rpc(state, &tenant_ctx.tenant_id, request.params).await;
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
                        code: if e.starts_with("Task not found:") {
                            -32001
                        } else {
                            -32603
                        },
                        message: e,
                        data: None,
                    }),
                    id: request.id,
                })),
            }
        }
        "task/list" => Ok(Json(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(json!({ "tasks": state.handler.list_tasks(&tenant_ctx.tenant_id).await })),
            error: None,
            id: request.id,
        })),
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
    tenant_id: crate::tenant::TenantId,
    params: Option<Value>,
) -> Result<Value, String> {
    let params = params.ok_or("Missing parameters")?;
    let request: a2a::types::SendMessageRequest =
        serde_json::from_value(params).map_err(|e| format!("Invalid request: {}", e))?;

    let response = state.handler.handle_message(request, tenant_id).await?;
    serde_json::to_value(response).map_err(|e| format!("Serialization error: {}", e))
}

async fn handle_get_task_rpc(
    state: A2AState,
    tenant_id: &crate::tenant::TenantId,
    params: Option<Value>,
) -> Result<Value, String> {
    let params = params.ok_or("Missing parameters")?;
    let task_id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing task ID")?;

    let task = state
        .handler
        .get_task(tenant_id, task_id)
        .await
        .ok_or_else(|| format!("Task not found: {task_id}"))?;
    serde_json::to_value(task).map_err(|error| format!("Serialization error: {error}"))
}

async fn handle_rest_message(
    State(state): State<A2AState>,
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(request): Json<a2a::types::SendMessageRequest>,
) -> (StatusCode, Json<Value>) {
    match state
        .handler
        .handle_message(request, tenant_ctx.tenant_id)
        .await
    {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap_or_else(|_| json!({}))),
        ),
        Err(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": {
                    "code": -32000,
                    "message": message,
                }
            })),
        ),
    }
}

async fn handle_get_task(
    State(state): State<A2AState>,
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    axum::extract::Path(task_id): axum::extract::Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let task = state
        .handler
        .get_task(&tenant_ctx.tenant_id, &task_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    serde_json::to_value(task)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn handle_list_tasks(
    State(state): State<A2AState>,
    Extension(tenant_ctx): Extension<RequestTenantContext>,
) -> Json<Value> {
    Json(json!({
        "tasks": state.handler.list_tasks(&tenant_ctx.tenant_id).await
    }))
}

async fn handle_stream_message(
    State(state): State<A2AState>,
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(request): Json<a2a::types::SendMessageRequest>,
) -> axum::response::Sse<
    impl futures::stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
> {
    use axum::response::sse::Event;
    use futures::stream::StreamExt;

    let handler = state.handler.clone();
    let tenant_id = tenant_ctx.tenant_id;

    let stream = async_stream::stream! {
        // Send initial working status
        let task_id = uuid::Uuid::new_v4().to_string();
        let context_id = request.message.context_id.clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let working_event = serde_json::json!({
            "taskId": task_id,
            "contextId": context_id,
            "status": {
                "state": "working",
                "timestamp": chrono::Utc::now().to_rfc3339()
            },
            "final": false
        });

        yield Ok(Event::default().data(working_event.to_string()));

        // Process the message
        match handler.handle_message_for_task(request, tenant_id.clone(), task_id.clone()).await {
            Ok(response) => {
                let completed_event = serde_json::json!({
                    "taskId": task_id,
                    "contextId": context_id,
                    "status": {
                        "state": "completed",
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    },
                    "final": true,
                    "response": response,
                });

                yield Ok(Event::default().data(completed_event.to_string()));
            }
            Err(e) => {
                let response = handler
                    .get_task(&tenant_id, &task_id)
                    .await
                    .map(|task| json!({ "task": task }));
                let error_event = serde_json::json!({
                    "taskId": task_id,
                    "contextId": context_id,
                    "status": {
                        "state": "failed",
                        "message": format!("Error: {}", e),
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    },
                    "final": true,
                    "response": response,
                });

                yield Ok(Event::default().data(error_event.to_string()));
            }
        }
    };

    axum::response::Sse::new(stream)
}
