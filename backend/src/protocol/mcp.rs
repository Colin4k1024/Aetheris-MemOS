//! MCP (Model Context Protocol) Implementation
//!
//! This module implements the Model Context Protocol for stateful memory access.
//! MCP enables AI agents to interact with the adaptive memory system through
//! a standardized protocol.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP Protocol Version
pub const MCP_VERSION: &str = "2024-11-05";

/// MCP Server Capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Whether server supports tools
    #[serde(default)]
    pub tools: Option<ToolCapability>,
    /// Whether server supports resources
    #[serde(default)]
    pub resources: Option<ResourceCapability>,
    /// Whether server supports prompts
    #[serde(default)]
    pub prompts: Option<PromptCapability>,
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            tools: Some(ToolCapability {
                list_changed: Some(true),
            }),
            resources: Some(ResourceCapability {
                subscribe: Some(true),
                list_changed: Some(true),
            }),
            prompts: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCapability {
    #[serde(default)]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceCapability {
    #[serde(default)]
    pub subscribe: Option<bool>,
    #[serde(default)]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCapability {
    #[serde(default)]
    pub list_changed: Option<bool>,
}

/// Initialize Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeRequest {
    pub protocol_version: Option<String>,
    pub capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

/// Client Capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientCapabilities {
    #[serde(default)]
    pub tools: Option<ToolCapability>,
    #[serde(default)]
    pub resources: Option<ResourceCapability>,
}

/// Client Info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// Initialize Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResponse {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

/// Server Info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// JSON-RPC Message Wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

/// JSON-RPC Error Codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const SERVER_ERROR: i32 = -32000;
}

/// Tool Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Tools List Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsListResponse {
    pub tools: Vec<Tool>,
}

/// Tool Call Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<Value>,
}

/// Tool Call Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResponse {
    pub content: Vec<ToolContent>,
    #[serde(default)]
    pub is_error: Option<bool>,
}

/// Tool Content Block
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "text")]
pub enum ToolContent {
    Text(String),
    Image(ImageContent),
    Resource(ResourceContent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageContent {
    pub data: String, // base64
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    pub uri: String,
    pub mime_type: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub data: Option<String>,
}

/// Resource Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// Resources List Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesListResponse {
    pub resources: Vec<Resource>,
}

/// Resource Content Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContentRequest {
    pub uri: String,
}

/// Resource Content Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContentResponse {
    pub contents: Vec<ResourceContent>,
}

// === Memory-specific MCP Tools ===

/// Memory Write Tool Name
pub const TOOL_MEMORY_WRITE: &str = "memory_write";
/// Memory Search Tool Name
pub const TOOL_MEMORY_SEARCH: &str = "memory_search";
/// Memory Recall Tool Name
pub const TOOL_MEMORY_RECALL: &str = "memory_recall";
/// Memory Forget Tool Name
pub const TOOL_MEMORY_FORGET: &str = "memory_forget";
/// Memory List Tool Name
pub const TOOL_MEMORY_LIST: &str = "memory_list";

/// Get all available memory tools
pub fn get_memory_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: TOOL_MEMORY_WRITE.to_string(),
            description: "Write a new memory to the adaptive memory system. Supports STM, LTM, KG, and MM layers.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The content to store in memory"
                    },
                    "layer": {
                        "type": "string",
                        "enum": ["stm", "ltm", "kg", "mm"],
                        "description": "Memory layer type"
                    },
                    "user_id": {
                        "type": "string",
                        "description": "User identifier"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Session identifier"
                    },
                    "agent_id": {
                        "type": "string",
                        "description": "Agent identifier"
                    },
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tags for categorization"
                    },
                    "metadata": {
                        "type": "object",
                        "description": "Additional metadata"
                    }
                },
                "required": ["content", "layer"]
            }),
        },
        Tool {
            name: TOOL_MEMORY_SEARCH.to_string(),
            description: "Search memories using semantic similarity or keyword matching.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query text"
                    },
                    "layer": {
                        "type": "string",
                        "enum": ["stm", "ltm", "kg", "mm"],
                        "description": "Memory layer to search"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 10,
                        "description": "Maximum number of results"
                    },
                    "user_id": {
                        "type": "string",
                        "description": "Filter by user identifier"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Filter by session identifier"
                    }
                },
                "required": ["query"]
            }),
        },
        Tool {
            name: TOOL_MEMORY_RECALL.to_string(),
            description: "Recall memories related to a specific context or session.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session identifier to recall"
                    },
                    "user_id": {
                        "type": "string",
                        "description": "User identifier"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 10,
                        "description": "Maximum number of memories to recall"
                    }
                },
                "required": ["session_id"]
            }),
        },
        Tool {
            name: TOOL_MEMORY_FORGET.to_string(),
            description: "Actively forget or delete specific memories.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "memory_id": {
                        "type": "string",
                        "description": "ID of the memory to forget"
                    },
                    "layer": {
                        "type": "string",
                        "enum": ["stm", "ltm", "kg", "mm"],
                        "description": "Memory layer"
                    }
                },
                "required": ["memory_id", "layer"]
            }),
        },
        Tool {
            name: TOOL_MEMORY_LIST.to_string(),
            description: "List memories from a specific layer with pagination.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "layer": {
                        "type": "string",
                        "enum": ["stm", "ltm", "kg", "mm"],
                        "description": "Memory layer to list"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 20,
                        "description": "Maximum number of results"
                    },
                    "offset": {
                        "type": "integer",
                        "default": 0,
                        "description": "Pagination offset"
                    },
                    "user_id": {
                        "type": "string",
                        "description": "Filter by user identifier"
                    }
                },
                "required": ["layer"]
            }),
        },
    ]
}

/// Get memory resources (for MCP resources capability)
pub fn get_memory_resources() -> Vec<Resource> {
    vec![
        Resource {
            uri: "memory://stm/sessions".to_string(),
            name: "STM Sessions".to_string(),
            description: Some("List of active short-term memory sessions".to_string()),
            mime_type: Some("application/json".to_string()),
        },
        Resource {
            uri: "memory://ltm/entries".to_string(),
            name: "LTM Entries".to_string(),
            description: Some("List of long-term memory entries".to_string()),
            mime_type: Some("application/json".to_string()),
        },
        Resource {
            uri: "memory://kg/entities".to_string(),
            name: "Knowledge Graph Entities".to_string(),
            description: Some("List of knowledge graph entities".to_string()),
            mime_type: Some("application/json".to_string()),
        },
        Resource {
            uri: "memory://mm/entries".to_string(),
            name: "Multimodal Entries".to_string(),
            description: Some("List of multimodal memory entries".to_string()),
            mime_type: Some("application/json".to_string()),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_capabilities() {
        let caps = ServerCapabilities::default();
        assert!(caps.tools.is_some());
        assert!(caps.resources.is_some());
    }

    #[test]
    fn test_memory_tools_count() {
        let tools = get_memory_tools();
        assert_eq!(tools.len(), 5);
    }

    #[test]
    fn test_memory_resources_count() {
        let resources = get_memory_resources();
        assert_eq!(resources.len(), 4);
    }
}
