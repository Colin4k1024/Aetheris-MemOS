//! Memory Kernel - Core Data Types
//!
//! This module defines the core data types for the Memory Kernel system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ulid::Ulid;

/// Unique identifier for a memory entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryId(pub String);

impl MemoryId {
    pub fn new() -> Self {
        Self(Ulid::new().to_string())
    }

    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for MemoryId {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents the type of memory layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerType {
    /// Short-term memory (ephemeral, fast)
    Stm,
    /// Long-term memory (persistent, indexed)
    Ltm,
    /// Knowledge graph (structured, relational)
    Kg,
    /// Multimodal memory (images, audio, video)
    Mm,
}

impl std::fmt::Display for LayerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerType::Stm => write!(f, "STM"),
            LayerType::Ltm => write!(f, "LTM"),
            LayerType::Kg => write!(f, "KG"),
            LayerType::Mm => write!(f, "MM"),
        }
    }
}

/// Memory entry content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MemoryContent {
    /// Plain text content
    Text(String),
    /// Structured JSON data
    Json(serde_json::Value),
    /// Binary data (for multimodal)
    Binary(Vec<u8>),
    /// Graph node/edge data
    Graph(GraphData),
}

/// Memory weights for layer selection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryWeights {
    pub stm: f64,
    pub ltm: f64,
    pub kg: f64,
    pub mm: f64,
}

/// Graph data for knowledge graph memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: MemoryId,
    pub layer: LayerType,
    pub content: MemoryContent,
    pub metadata: MemoryMetadata,
    pub created_at: i64,
    pub updated_at: i64,
}

impl MemoryEntry {
    pub fn new(layer: LayerType, content: MemoryContent) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: MemoryId::new(),
            layer,
            content,
            metadata: MemoryMetadata::default(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_metadata(mut self, metadata: MemoryMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Metadata associated with a memory entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryMetadata {
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub tags: Vec<String>,
    pub importance: f64,
    pub access_count: u32,
    pub last_accessed: Option<i64>,
    pub expires_at: Option<i64>,
    pub source: Option<String>,
    pub extra: HashMap<String, serde_json::Value>,
}

/// Memory query parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQuery {
    pub layer: Option<LayerType>,
    pub text: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub filters: MemoryFilters,
    pub limit: usize,
    pub offset: usize,
}

impl Default for MemoryQuery {
    fn default() -> Self {
        Self {
            layer: None,
            text: None,
            embedding: None,
            filters: MemoryFilters::default(),
            limit: 10,
            offset: 0,
        }
    }
}

/// Filters for memory queries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryFilters {
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub min_importance: Option<f64>,
    pub created_after: Option<i64>,
    pub created_before: Option<i64>,
}

/// Result of a memory search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMatch {
    pub entry: MemoryEntry,
    pub score: f64,
    pub highlights: Vec<String>,
}

/// Memory operation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryOperation {
    pub id: MemoryId,
    pub layer: LayerType,
    pub operation: OperationType,
    pub timestamp: i64,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    Store,
    Retrieve,
    Update,
    Delete,
    Search,
    Evict,
}
