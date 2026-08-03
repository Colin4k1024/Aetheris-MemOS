use axum::{
    extract::{Extension, State},
    response::{
        sse::{Event, Sse},
        Json,
    },
    routing::post,
    Router,
};
use futures::stream::Stream;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::StreamExt;

use a2a::types::{
    Message, Part, PartContent, Role, SendMessageRequest, Task, TaskState, TaskStatus,
};

use crate::tenant::RequestTenantContext;

use super::handler::A2AHandler;
use super::router::A2AState;

pub fn streaming_router() -> Router<A2AState> {
    Router::new().route("/a2a/rest/messages/stream", post(handle_stream_message))
}

async fn handle_stream_message(
    State(state): State<A2AState>,
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(request): Json<SendMessageRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let handler = state.handler.clone();

    let stream = async_stream::stream! {
        // Send initial task status
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

        let event_data = serde_json::to_string(&working_event).unwrap_or_default();
        yield Ok(Event::default().data(event_data));

        // Process the message with real tenant context
        match handler.handle_message(request, &tenant_ctx).await {
            Ok(response) => {
                match response {
                    a2a::types::SendMessageResponse::Task(task) => {
                        let completed_event = serde_json::json!({
                            "taskId": task.id,
                            "contextId": task.context_id,
                            "status": {
                                "state": "completed",
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            },
                            "final": true
                        });

                        let event_data = serde_json::to_string(&completed_event).unwrap_or_default();
                        yield Ok(Event::default().data(event_data));
                    }
                    a2a::types::SendMessageResponse::Message(msg) => {
                        let event_data = serde_json::to_string(&msg).unwrap_or_default();
                        yield Ok(Event::default().event("message").data(event_data));
                    }
                }
            }
            Err(e) => {
                let error_event = serde_json::json!({
                    "taskId": task_id,
                    "contextId": context_id,
                    "status": {
                        "state": "failed",
                        "message": format!("Error: {}", e),
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    },
                    "final": true
                });

                let event_data = serde_json::to_string(&error_event).unwrap_or_default();
                yield Ok(Event::default().data(event_data));
            }
        }
    };

    Sse::new(stream)
}
