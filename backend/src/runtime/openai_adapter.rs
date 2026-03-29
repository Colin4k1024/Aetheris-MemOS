//! OpenAI Agents SDK Adapter
//!
//! This adapter integrates with OpenAI's Agents SDK for memory operations.

use crate::agent::memory_agent::MemoryAgent;
use crate::kernel::error::MemoryResult;
use crate::kernel::types::*;
use crate::runtime::{AdapterConfig, MessageRole, RuntimeAdapter, RuntimeMessage};

/// OpenAI Memory Adapter
///
/// Provides integration with OpenAI Agents SDK for automatic memory management.
pub struct OpenAIMemoryAdapter {
    agent: std::sync::Arc<MemoryAgent>,
    config: AdapterConfig,
}

impl OpenAIMemoryAdapter {
    pub fn new(agent: std::sync::Arc<MemoryAgent>) -> Self {
        Self {
            agent,
            config: AdapterConfig::default(),
        }
    }

    pub fn with_config(agent: std::sync::Arc<MemoryAgent>, config: AdapterConfig) -> Self {
        Self { agent, config }
    }

    /// Process an OpenAI message and store in memory.
    pub async fn process_message(
        &self,
        session_id: &str,
        role: MessageRole,
        content: &str,
    ) -> MemoryResult<MemoryId> {
        let message = RuntimeMessage {
            role,
            content: content.to_string(),
            session_id: session_id.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            metadata: Default::default(),
        };

        self.store_message(&message).await
    }

    /// Build context from memory for OpenAI API.
    pub async fn build_context(
        &self,
        session_id: &str,
        current_query: &str,
    ) -> MemoryResult<String> {
        // Get recent messages
        let recent = self.get_history(session_id, 10).await?;

        // Search for relevant memories
        let search_results = self.search(current_query).await?;

        let mut context = String::new();

        // Add relevant memories as context
        if !search_results.is_empty() {
            context.push_str("Relevant memories:\n");
            for result in search_results.iter().take(5) {
                if let MemoryContent::Text(text) = &result.entry.content {
                    context.push_str(&format!("- {}\n", text));
                }
            }
            context.push('\n');
        }

        // Add recent conversation
        if !recent.is_empty() {
            context.push_str("Recent conversation:\n");
            for msg in recent {
                let role_str = match msg.role {
                    MessageRole::User => "User",
                    MessageRole::Assistant => "Assistant",
                    MessageRole::System => "System",
                    MessageRole::Tool => "Tool",
                };
                context.push_str(&format!("{}: {}\n", role_str, msg.content));
            }
        }

        Ok(context)
    }

    /// Handle OpenAI tool calls with memory.
    pub async fn handle_tool_result(
        &self,
        session_id: &str,
        tool_name: &str,
        result: &str,
    ) -> MemoryResult<MemoryId> {
        let message = RuntimeMessage {
            role: MessageRole::Tool,
            content: format!("Tool '{}' result: {}", tool_name, result),
            session_id: session_id.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            metadata: {
                let mut m = std::collections::HashMap::new();
                m.insert("tool_name".to_string(), serde_json::json!(tool_name));
                m
            },
        };

        self.store_message(&message).await
    }
}

#[async_trait::async_trait]
impl RuntimeAdapter for OpenAIMemoryAdapter {
    fn name(&self) -> &str {
        "openai"
    }

    async fn store_message(&self, message: &RuntimeMessage) -> MemoryResult<MemoryId> {
        // Determine user_id from session_id (simplified)
        let user_id = &message.session_id;

        self.agent
            .remember(
                user_id,
                Some(&message.session_id),
                None,
                message.content.clone(),
            )
            .await
    }

    async fn get_history(
        &self,
        session_id: &str,
        limit: usize,
    ) -> MemoryResult<Vec<RuntimeMessage>> {
        let query = MemoryQuery {
            layer: None,
            text: None,
            filters: MemoryFilters {
                session_id: Some(session_id.to_string()),
                ..Default::default()
            },
            limit,
            ..Default::default()
        };

        // This would need a kernel reference - simplified for now
        Ok(vec![])
    }

    async fn search(&self, query: &str) -> MemoryResult<Vec<MemoryMatch>> {
        // This would need user context - simplified for now
        Ok(vec![])
    }
}

/// OpenAI-specific memory tools for function calling.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryTools {
    /// Tool definition for OpenAI function calling
    pub store_memory: serde_json::Value,
    /// Tool definition for searching memory
    pub search_memory: serde_json::Value,
    /// Tool definition for getting context
    pub get_context: serde_json::Value,
}

impl MemoryTools {
    pub fn new() -> Self {
        Self {
            store_memory: serde_json::json!({
                "type": "function",
                "function": {
                    "name": "store_memory",
                    "description": "Store important information in memory for future retrieval",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "content": {
                                "type": "string",
                                "description": "The content to remember"
                            },
                            "importance": {
                                "type": "number",
                                "description": "Importance level (0-1)",
                                "minimum": 0,
                                "maximum": 1
                            }
                        },
                        "required": ["content"]
                    }
                }
            }),
            search_memory: serde_json::json!({
                "type": "function",
                "function": {
                    "name": "search_memory",
                    "description": "Search for relevant memories",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "The search query"
                            }
                        },
                        "required": ["query"]
                    }
                }
            }),
            get_context: serde_json::json!({
                "type": "function",
                "function": {
                    "name": "get_context",
                    "description": "Get relevant context from memory for current task",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "task": {
                                "type": "string",
                                "description": "Current task or query"
                            }
                        },
                        "required": ["task"]
                    }
                }
            }),
        }
    }
}

impl Default for MemoryTools {
    fn default() -> Self {
        Self::new()
    }
}
