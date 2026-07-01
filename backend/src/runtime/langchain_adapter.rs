//! LangChain Adapter
//!
//! This adapter integrates with LangChain Rust for memory operations.
//! It provides both a RuntimeAdapter implementation and a LangChain tool
//! that can be used by LLM agents in LangChain workflows.

use crate::kernel::error::MemoryResult;
use crate::kernel::types::*;
use crate::runtime::{AdapterConfig, MessageRole, RuntimeAdapter, RuntimeMessage};

/// LangChain Memory Adapter
///
/// Provides integration with LangChain Rust for automatic memory management.
/// This adapter can be used with LangChain agents and tools.
pub struct LangChainAdapter {
    agent: std::sync::Arc<crate::agent::memory_agent::MemoryAgent>,
    config: AdapterConfig,
}

impl LangChainAdapter {
    /// Create a new LangChain adapter with the given memory agent.
    pub fn new(agent: std::sync::Arc<crate::agent::memory_agent::MemoryAgent>) -> Self {
        Self {
            agent,
            config: AdapterConfig::default(),
        }
    }

    /// Create a new LangChain adapter with custom configuration.
    pub fn with_config(
        agent: std::sync::Arc<crate::agent::memory_agent::MemoryAgent>,
        config: AdapterConfig,
    ) -> Self {
        Self { agent, config }
    }

    /// Process a message and store it in memory.
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

    /// Build context from memory for LLM prompts.
    pub async fn build_context(
        &self,
        session_id: &str,
        current_query: &str,
    ) -> MemoryResult<String> {
        let recent = self.get_history(session_id, 10).await?;
        let search_results = self.search(current_query).await?;

        let mut context = String::new();

        if !search_results.is_empty() {
            context.push_str("Relevant memories:\n");
            for result in search_results.iter().take(5) {
                if let MemoryContent::Text(text) = &result.entry.content {
                    context.push_str(&format!("- {}\n", text));
                }
            }
            context.push('\n');
        }

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
}

#[async_trait::async_trait]
impl RuntimeAdapter for LangChainAdapter {
    fn name(&self) -> &str {
        "langchain"
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
        let memories = self.agent.history(session_id, limit).await?;
        let mut messages: Vec<RuntimeMessage> = memories
            .into_iter()
            .filter_map(|m| match m.entry.content {
                MemoryContent::Text(content) => Some(RuntimeMessage {
                    role: MessageRole::User,
                    content,
                    session_id: m
                        .entry
                        .metadata
                        .session_id
                        .unwrap_or_else(|| session_id.to_string()),
                    timestamp: m.entry.created_at,
                    metadata: m.entry.metadata.extra,
                }),
                _ => None,
            })
            .collect();

        messages.sort_by_key(|m| m.timestamp);
        Ok(messages)
    }

    async fn search(&self, query: &str) -> MemoryResult<Vec<MemoryMatch>> {
        self.agent.recall_any(query, 10).await
    }
}

/// LangChain Memory Tool
///
/// A LangChain tool that wraps the memory system's store and search functions.
/// This allows LLM agents to use memory as a tool in LangChain workflows.
///
/// # Arguments
///
/// * `agent` - The memory agent to use
/// * `config` - Optional configuration
///
/// # Example
///
/// ```ignore
/// use langchain_rust::ChatOllama;
/// use langchain_rust::chain::Chain;
///
/// let memory_tool = LangChainMemoryTool::new(memory_agent);
/// let tool = memory_tool.into_tool();
///
/// // Use with LangChain
/// ```
#[derive(Clone)]
pub struct LangChainMemoryTool {
    agent: std::sync::Arc<crate::agent::memory_agent::MemoryAgent>,
    config: AdapterConfig,
}

impl std::fmt::Debug for LangChainMemoryTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LangChainMemoryTool")
            .field("config", &self.config)
            .finish()
    }
}

impl LangChainMemoryTool {
    /// Create a new LangChain memory tool.
    pub fn new(agent: std::sync::Arc<crate::agent::memory_agent::MemoryAgent>) -> Self {
        Self {
            agent,
            config: AdapterConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(
        agent: std::sync::Arc<crate::agent::memory_agent::MemoryAgent>,
        config: AdapterConfig,
    ) -> Self {
        Self { agent, config }
    }

    /// Execute the memory tool.
    ///
    /// # Arguments
    ///
    /// * `input` - JSON string with action, content, session_id, and optional query
    ///
    /// # Returns
    ///
    /// Returns a JSON string with the result of the operation.
    ///
    /// # Actions
    ///
    /// * `store` - Store content in memory (requires: content, session_id)
    /// * `search` - Search for relevant memories (requires: query)
    /// * `history` - Get conversation history (requires: session_id)
    pub async fn execute(&self, input: &str) -> MemoryResult<String> {
        #[derive(serde::Deserialize)]
        struct ToolInput {
            action: String,
            content: Option<String>,
            session_id: Option<String>,
            query: Option<String>,
        }

        let input: ToolInput = serde_json::from_str(input)
            .map_err(|e| crate::kernel::error::MemoryError::Serialization(e.to_string()))?;

        match input.action.as_str() {
            "store" => {
                let content = input.content.ok_or_else(|| {
                    crate::kernel::error::MemoryError::InvalidOperation(
                        "content is required for store action".to_string(),
                    )
                })?;
                let session_id = input.session_id.unwrap_or_else(|| "default".to_string());

                let message = RuntimeMessage {
                    role: MessageRole::User,
                    content,
                    session_id: session_id.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                    metadata: {
                        let mut m = std::collections::HashMap::new();
                        m.insert("source".to_string(), serde_json::json!("langchain_tool"));
                        m
                    },
                };

                let id = self
                    .agent
                    .remember(&session_id, Some(&session_id), None, message.content)
                    .await?;

                Ok(serde_json::json!({
                    "status": "stored",
                    "memory_id": id.as_str(),
                    "session_id": session_id
                })
                .to_string())
            }
            "search" => {
                let query = input.query.ok_or_else(|| {
                    crate::kernel::error::MemoryError::InvalidOperation(
                        "query is required for search action".to_string(),
                    )
                })?;

                let results = self.agent.recall(&"default", &query).await?;

                let response: Vec<serde_json::Value> = results
                    .iter()
                    .take(10)
                    .map(|m| {
                        let content = match &m.entry.content {
                            MemoryContent::Text(s) => s.clone(),
                            MemoryContent::Json(j) => j.to_string(),
                            MemoryContent::Binary(_) => String::from("[binary data]"),
                            MemoryContent::Graph(_) => String::from("[graph data]"),
                        };
                        serde_json::json!({
                            "content": content,
                            "score": m.score,
                            "memory_id": m.entry.id.as_str(),
                            "layer": m.entry.layer.to_string()
                        })
                    })
                    .collect();

                Ok(serde_json::json!({
                    "status": "search_complete",
                    "results": response,
                    "count": response.len()
                })
                .to_string())
            }
            "history" => {
                let session_id = input.session_id.ok_or_else(|| {
                    crate::kernel::error::MemoryError::InvalidOperation(
                        "session_id is required for history action".to_string(),
                    )
                })?;

                let memories = self.agent.history(&session_id, 20).await?;
                let messages: Vec<serde_json::Value> = memories
                    .into_iter()
                    .filter_map(|m| match m.entry.content {
                        MemoryContent::Text(content) => Some(serde_json::json!({
                            "content": content,
                            "memory_id": m.entry.id.as_str(),
                            "layer": m.entry.layer.to_string(),
                            "created_at": m.entry.created_at,
                            "score": m.score
                        })),
                        _ => None,
                    })
                    .collect();

                Ok(serde_json::json!({
                    "status": "history_retrieved",
                    "session_id": session_id,
                    "messages": messages
                })
                .to_string())
            }
            _ => Err(crate::kernel::error::MemoryError::InvalidOperation(
                format!("Unknown action: {}", input.action),
            )),
        }
    }

    /// Get the tool definition for LangChain.
    ///
    /// Returns a JSON schema that describes the tool's inputs.
    pub fn tool_definition() -> serde_json::Value {
        serde_json::json!({
            "name": "memory",
            "description": "Memory tool for storing and retrieving information. Use this to remember important facts, search for past information, or get conversation history.",
            "parameters": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "The action to perform: 'store' to save information, 'search' to find relevant memories, 'history' to get conversation history",
                        "enum": ["store", "search", "history"]
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to store in memory (required for 'store' action)"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "The session identifier for grouping related memories (used in 'store' and 'history')"
                    },
                    "query": {
                        "type": "string",
                        "description": "The search query (required for 'search' action)"
                    }
                },
                "required": ["action"]
            }
        })
    }
}

/// Create a LangChain tool from the memory tool.
///
/// This converts the LangChainMemoryTool into a format suitable for use
/// with LangChain's tool system.
pub fn create_langchain_tool(
    agent: std::sync::Arc<crate::agent::memory_agent::MemoryAgent>,
) -> LangChainMemoryTool {
    LangChainMemoryTool::new(agent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::types::MemoryContent;

    #[test]
    fn test_tool_definition_structure() {
        let def = LangChainMemoryTool::tool_definition();

        assert_eq!(def["name"], "memory");
        assert_eq!(def["parameters"]["type"], "object");
        assert!(def["parameters"]["properties"].is_object());
    }

    #[test]
    fn test_tool_input_parsing_store() {
        let input = r#"{"action": "store", "content": "Hello world", "session_id": "test123"}"#;
        let parsed: ToolInput = serde_json::from_str(input).unwrap();

        assert_eq!(parsed.action, "store");
        assert_eq!(parsed.content, Some("Hello world".to_string()));
        assert_eq!(parsed.session_id, Some("test123".to_string()));
    }

    #[test]
    fn test_tool_input_parsing_search() {
        let input = r#"{"action": "search", "query": "hello"}"#;
        let parsed: ToolInput = serde_json::from_str(input).unwrap();

        assert_eq!(parsed.action, "search");
        assert_eq!(parsed.query, Some("hello".to_string()));
    }

    #[test]
    fn test_tool_input_parsing_history() {
        let input = r#"{"action": "history", "session_id": "test123"}"#;
        let parsed: ToolInput = serde_json::from_str(input).unwrap();

        assert_eq!(parsed.action, "history");
        assert_eq!(parsed.session_id, Some("test123".to_string()));
    }

    #[test]
    fn test_invalid_json() {
        let input = "not valid json";
        let result: Result<ToolInput, _> = serde_json::from_str(input);
        assert!(result.is_err());
    }

    #[derive(serde::Deserialize)]
    struct ToolInput {
        action: String,
        content: Option<String>,
        session_id: Option<String>,
        query: Option<String>,
    }
}
