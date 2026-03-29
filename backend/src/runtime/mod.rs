//! Runtime Adapters - Agent Runtime Integration
//!
//! This module provides adapters for various agent runtimes:
//! - OpenAI Agents SDK
//! - Anthropic
//! - LangChain
//! - LlamaIndex

pub mod openai_adapter;
pub mod anthropic_adapter;
pub mod planner_sandbox;
pub mod subagent_pool;

pub use openai_adapter::OpenAIMemoryAdapter;
pub use anthropic_adapter::AnthropicMemoryAdapter;
pub use planner_sandbox::{PlannerSandbox, SandboxError, ToolRegistry, VirtualEffectStore};

use crate::kernel::types::*;
use crate::kernel::error::MemoryResult;
use crate::agent::memory_agent::MemoryAgent;

/// Common trait for runtime adapters.
#[async_trait::async_trait]
pub trait RuntimeAdapter: Send + Sync {
    /// Get the adapter name.
    fn name(&self) -> &str;

    /// Store a message in memory.
    async fn store_message(&self, message: &RuntimeMessage) -> MemoryResult<MemoryId>;

    /// Get conversation history.
    async fn get_history(&self, session_id: &str, limit: usize) -> MemoryResult<Vec<RuntimeMessage>>;

    /// Search memories.
    async fn search(&self, query: &str) -> MemoryResult<Vec<MemoryMatch>>;
}

/// Message from/to agent runtime.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeMessage {
    pub role: MessageRole,
    pub content: String,
    pub session_id: String,
    pub timestamp: i64,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// Adapter configuration.
#[derive(Debug, Clone)]
pub struct AdapterConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<usize>,
}

impl Default for AdapterConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: None,
            model: None,
            max_tokens: Some(4096),
        }
    }
}
