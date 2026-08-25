//! Transparent Proxy for OpenAI / Anthropic compatible APIs (#91).
//!
//! Before forwarding the request to the upstream LLM, the proxy:
//! 1. Identifies the session (from `x-session-id` header or request body)
//! 2. Recalls relevant memories via `MemorySearchService`
//! 3. Injects context into the system prompt
//!
//! After the response is streamed back, an async task extracts and stores
//! new memories via `MemoryStorageService`.

use axum::{
    extract::Extension,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::error::AppError;
use crate::services::memory_search::MemorySearchService;
use crate::services::memory_storage::MemoryStorageService;
use crate::tenant::RequestTenantContext;

pub fn router() -> Router {
    Router::new()
        .route("/openai/chat/completions", post(openai_chat_completions))
        .route("/anthropic/messages", post(anthropic_messages))
}

// ============================================================================
// OpenAI-compatible types
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenAIChatRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct OpenAIChatResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<OpenAIChoice>,
    pub usage: OpenAIUsage,
}

#[derive(Debug, Serialize)]
pub struct OpenAIChoice {
    pub index: u32,
    pub message: OpenAIMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct OpenAIUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ============================================================================
// Anthropic-compatible types
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnthropicMessagesRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub max_tokens: u32,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct AnthropicMessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<AnthropicContentBlock>,
    pub model: String,
    pub usage: AnthropicUsage,
}

#[derive(Debug, Serialize)]
pub struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/v1/proxy/openai/chat/completions
///
/// Accepts an OpenAI-compatible chat completion request, injects memory
/// context into the system prompt, and returns a placeholder response
/// (the upstream LLM forward is a follow-up — #91 increment 1).
async fn openai_chat_completions(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    headers: HeaderMap,
    Json(req): Json<OpenAIChatRequest>,
) -> Result<Json<OpenAIChatResponse>, AppError> {
    let session_id = extract_session_id(&headers, &req.messages);
    info!(
        tenant_id = %tenant_ctx.tenant_id,
        session_id = %session_id,
        model = %req.model,
        stream = req.stream,
        "proxy: OpenAI chat completions request"
    );

    // 1. Recall relevant memories and build context.
    let context = build_context(&tenant_ctx.tenant_id, &session_id, &req.messages).await;

    // 2. Inject context into the system prompt (last message if role=system).
    let _augmented_messages = inject_context(req.messages, &context);

    // 3. Forward to upstream LLM (placeholder — #91 follow-up).
    // TODO: call actual OpenAI-compatible endpoint via LLMService.

    Ok(Json(OpenAIChatResponse {
        id: format!("chatcmpl-{}", ulid::Ulid::new()),
        object: "chat.completion".to_string(),
        created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        model: req.model.clone(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: OpenAIMessage {
                role: "assistant".to_string(),
                content: "[proxy placeholder] LLM forward not yet implemented (#91)".to_string(),
            },
            finish_reason: "stop".to_string(),
        }],
        usage: OpenAIUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
    }))
}

/// POST /api/v1/proxy/anthropic/messages
///
/// Accepts an Anthropic-compatible messages request, injects memory context,
/// and returns a placeholder response.
async fn anthropic_messages(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    headers: HeaderMap,
    Json(req): Json<AnthropicMessagesRequest>,
) -> Result<Json<AnthropicMessagesResponse>, AppError> {
    let session_id = headers
        .get("x-session-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    info!(
        tenant_id = %tenant_ctx.tenant_id,
        session_id = %session_id,
        model = %req.model,
        stream = req.stream,
        "proxy: Anthropic messages request"
    );

    // 1. Build context from recent messages.
    let messages: Vec<OpenAIMessage> = req
        .messages
        .iter()
        .map(|m| OpenAIMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();
    let context = build_context(&tenant_ctx.tenant_id, &session_id, &messages).await;

    let system_with_context = if let Some(sys) = &req.system {
        format!("{sys}\n\n[Memory Context]\n{context}")
    } else {
        format!("[Memory Context]\n{context}")
    };
    let _ = system_with_context; // used when forwarding (#91 follow-up)

    Ok(Json(AnthropicMessagesResponse {
        id: format!("msg_{}", ulid::Ulid::new()),
        response_type: "message".to_string(),
        role: "assistant".to_string(),
        content: vec![AnthropicContentBlock {
            content_type: "text".to_string(),
            text: "[proxy placeholder] LLM forward not yet implemented (#91)".to_string(),
        }],
        model: req.model.clone(),
        usage: AnthropicUsage {
            input_tokens: 0,
            output_tokens: 0,
        },
    }))
}

// ============================================================================
// Helpers
// ============================================================================

/// Extract session ID from headers or request body.
fn extract_session_id(headers: &HeaderMap, messages: &[OpenAIMessage]) -> String {
    headers
        .get("x-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Fallback: derive from message content hash
            let content = messages
                .iter()
                .map(|m| format!("{}:{}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n");
            {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut h = DefaultHasher::new();
            content.hash(&mut h);
            format!("session-{:x}", h.finish())
        }
        })
}

/// Build context from memory search.
async fn build_context(
    tenant_id: &crate::tenant::TenantId,
    session_id: &str,
    messages: &[OpenAIMessage],
) -> String {
    let query = messages
        .last()
        .map(|m| m.content.as_str())
        .unwrap_or("");
    if query.is_empty() {
        return String::new();
    }
    match MemorySearchService::search_ltm_for_tenant(tenant_id, query, 5, None, None).await {
        Ok(results) => {
            if results.is_empty() {
                String::new()
            } else {
                results
                    .iter()
                    .map(|r| format!("- {}", r.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        Err(e) => {
            tracing::warn!(
                tenant_id = %tenant_id,
                session_id = %session_id,
                error = %e,
                "proxy: memory search failed, continuing without context"
            );
            String::new()
        }
    }
}

/// Inject context into the message list.
///
/// If a system message exists, append context. Otherwise, prepend a system
/// message with the context.
fn inject_context(mut messages: Vec<OpenAIMessage>, context: &str) -> Vec<OpenAIMessage> {
    if context.is_empty() {
        return messages;
    }
    if let Some(sys_msg) = messages.iter_mut().find(|m| m.role == "system") {
        sys_msg.content = format!("{}\n\n[Memory Context]\n{}", sys_msg.content, context);
    } else {
        messages.insert(
            0,
            OpenAIMessage {
                role: "system".to_string(),
                content: format!("[Memory Context]\n{}", context),
            },
        );
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_context_appends_to_system_message() {
        let msgs = vec![
            OpenAIMessage {
                role: "system".to_string(),
                content: "You are helpful.".to_string(),
            },
            OpenAIMessage {
                role: "user".to_string(),
                content: "Hi".to_string(),
            },
        ];
        let result = inject_context(msgs, "user prefers dark mode");
        assert_eq!(result.len(), 2);
        assert!(result[0].content.contains("Memory Context"));
        assert!(result[0].content.contains("user prefers dark mode"));
    }

    #[test]
    fn inject_context_prepends_when_no_system() {
        let msgs = vec![OpenAIMessage {
            role: "user".to_string(),
            content: "Hi".to_string(),
        }];
        let result = inject_context(msgs, "user prefers dark mode");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].role, "system");
        assert!(result[0].content.contains("Memory Context"));
    }

    #[test]
    fn inject_context_empty_context_is_noop() {
        let msgs = vec![OpenAIMessage {
            role: "user".to_string(),
            content: "Hi".to_string(),
        }];
        let result = inject_context(msgs.clone(), "");
        assert_eq!(result.len(), msgs.len());
    }

    #[test]
    fn extract_session_id_from_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-session-id", "sess-123".parse().unwrap());
        let id = extract_session_id(&headers, &[]);
        assert_eq!(id, "sess-123");
    }

    #[test]
    fn extract_session_id_falls_back_to_hash() {
        let headers = HeaderMap::new();
        let id = extract_session_id(
            &headers,
            &[OpenAIMessage {
                role: "user".to_string(),
                content: "test".to_string(),
            }],
        );
        assert!(id.starts_with("session-"));
    }
}