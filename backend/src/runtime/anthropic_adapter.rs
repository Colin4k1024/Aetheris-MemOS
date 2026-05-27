//! Anthropic Adapter
//!
//! This adapter integrates with Anthropic's Claude for memory operations.

use crate::agent::memory_agent::MemoryAgent;
use crate::kernel::error::MemoryResult;
use crate::kernel::types::*;
use crate::runtime::{AdapterConfig, MessageRole, RuntimeAdapter, RuntimeMessage};

/// Anthropic Memory Adapter
///
/// Provides integration with Anthropic Claude for automatic memory management.
pub struct AnthropicMemoryAdapter {
    agent: std::sync::Arc<MemoryAgent>,
    config: AdapterConfig,
}

impl AnthropicMemoryAdapter {
    pub fn new(agent: std::sync::Arc<MemoryAgent>) -> Self {
        Self {
            agent,
            config: AdapterConfig::default(),
        }
    }

    pub fn with_config(agent: std::sync::Arc<MemoryAgent>, config: AdapterConfig) -> Self {
        Self { agent, config }
    }

    /// Process a Claude message and store in memory.
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

    /// Build system prompt with memory context for Claude.
    pub async fn build_system_prompt(&self, session_id: &str) -> MemoryResult<String> {
        // Get recent conversation
        let recent = self.get_history(session_id, 5).await?;

        let mut prompt = String::new();

        // Add memory instructions
        prompt.push_str("You have access to a memory system. ");
        prompt.push_str("Important information will be stored automatically.\n\n");

        // Add recent conversation as context
        if !recent.is_empty() {
            prompt.push_str("Recent conversation:\n");
            for msg in recent {
                let role_str = match msg.role {
                    MessageRole::User => "Human",
                    MessageRole::Assistant => "Assistant",
                    MessageRole::System => "System",
                    MessageRole::Tool => "Tool",
                };
                prompt.push_str(&format!("{}: {}\n", role_str, msg.content));
            }
        }

        Ok(prompt)
    }

    /// Handle Claude tool use results.
    pub async fn handle_tool_use(
        &self,
        session_id: &str,
        tool_use_id: &str,
        output: &str,
    ) -> MemoryResult<MemoryId> {
        let message = RuntimeMessage {
            role: MessageRole::Tool,
            content: format!("[Tool: {}] {}", tool_use_id, output),
            session_id: session_id.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            metadata: {
                let mut m = std::collections::HashMap::new();
                m.insert("tool_use_id".to_string(), serde_json::json!(tool_use_id));
                m
            },
        };

        self.store_message(&message).await
    }
}

#[async_trait::async_trait]
impl RuntimeAdapter for AnthropicMemoryAdapter {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn store_message(&self, message: &RuntimeMessage) -> MemoryResult<MemoryId> {
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
        Ok(vec![])
    }
}

/// Anthropic-specific memory tools for tool use.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnthropicMemoryTools {
    /// Tool definition for Anthropic
    pub tools: Vec<serde_json::Value>,
}

impl AnthropicMemoryTools {
    pub fn new() -> Self {
        Self {
            tools: vec![
                serde_json::json!({
                    "name": "memory_search",
                    "description": "Search through stored memories for relevant information",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Search query"
                            }
                        },
                        "required": ["query"]
                    }
                }),
                serde_json::json!({
                    "name": "memory_store",
                    "description": "Store important information in memory",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "content": {
                                "type": "string",
                                "description": "Content to remember"
                            },
                            "importance": {
                                "type": "number",
                                "description": "Importance level 0-1"
                            }
                        },
                        "required": ["content"]
                    }
                }),
            ],
        }
    }
}

impl Default for AnthropicMemoryTools {
    fn default() -> Self {
        Self::new()
    }
}
