//! Memory Kernel - Core Trait Definitions

#[allow(unused_imports)]
use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::types::*;
use std::collections::HashMap;

/// Core trait for Memory Kernel operations.
///
/// This is the main entry point for all memory operations in the system.
/// Implementations can delegate to different storage backends.
#[async_trait::async_trait]
pub trait MemoryKernel: Send + Sync {
    /// Store a new memory entry.
    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId>;

    /// Retrieve a memory entry by ID.
    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry>;

    /// Search memories based on query parameters.
    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>>;

    /// Update an existing memory entry.
    async fn update(&self, id: &MemoryId, entry: MemoryEntry) -> MemoryResult<()>;

    /// Delete a memory entry.
    async fn delete(&self, id: &MemoryId) -> MemoryResult<()>;

    /// Evict memories based on policy.
    async fn evict(&self, policy: &EvictionPolicy) -> MemoryResult<Vec<MemoryId>>;

    /// Get statistics about memory usage.
    async fn stats(&self) -> MemoryResult<MemoryStats>;
}

/// Policy for memory eviction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvictionPolicy {
    pub layer: Option<LayerType>,
    pub max_entries: Option<usize>,
    pub max_size_bytes: Option<u64>,
    pub older_than_seconds: Option<i64>,
    pub min_importance: Option<f64>,
}

impl EvictionPolicy {
    pub fn new() -> Self {
        Self {
            layer: None,
            max_entries: None,
            max_size_bytes: None,
            older_than_seconds: None,
            min_importance: None,
        }
    }

    pub fn layer(mut self, layer: LayerType) -> Self {
        self.layer = Some(layer);
        self
    }

    pub fn max_entries(mut self, max: usize) -> Self {
        self.max_entries = Some(max);
        self
    }

    pub fn older_than(mut self, seconds: i64) -> Self {
        self.older_than_seconds = Some(seconds);
        self
    }

    pub fn min_importance(mut self, importance: f64) -> Self {
        self.min_importance = Some(importance);
        self
    }
}

impl Default for EvictionPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryStats {
    pub total_entries: usize,
    pub by_layer: HashMap<LayerType, LayerStats>,
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LayerStats {
    pub entry_count: usize,
    pub size_bytes: u64,
    pub avg_access_count: f64,
}

/// Trait for individual memory layer implementations.
#[async_trait::async_trait]
pub trait MemoryLayer: Send + Sync {
    /// Get the layer type.
    fn layer_type(&self) -> LayerType;

    /// Store memory in this layer.
    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId>;

    /// Retrieve memory from this layer.
    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry>;

    /// Search within this layer.
    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>>;

    /// Update memory in this layer.
    async fn update(&self, id: &MemoryId, entry: MemoryEntry) -> MemoryResult<()>;

    /// Delete memory from this layer.
    async fn delete(&self, id: &MemoryId) -> MemoryResult<()>;

    /// Get layer statistics.
    async fn stats(&self) -> MemoryResult<LayerStats>;
}

/// Trait for vector similarity search.
#[async_trait::async_trait]
pub trait VectorSearch: Send + Sync {
    /// Search by embedding vector.
    async fn search_by_vector(
        &self,
        vector: &[f32],
        limit: usize,
        filters: &MemoryFilters,
    ) -> MemoryResult<Vec<MemoryMatch>>;

    /// Add vectors to the index.
    async fn upsert_vectors(
        &self,
        entries: Vec<(MemoryId, Vec<f32>, MemoryEntry)>,
    ) -> MemoryResult<()>;
}

/// Trait for graph operations (Knowledge Graph).
#[async_trait::async_trait]
pub trait GraphMemory: Send + Sync {
    /// Add a node to the graph.
    async fn add_node(&self, node: GraphNode) -> MemoryResult<()>;

    /// Add an edge to the graph.
    async fn add_edge(&self, edge: GraphEdge) -> MemoryResult<()>;

    /// Query nodes.
    async fn query_nodes(
        &self,
        labels: &[String],
        properties: &HashMap<String, serde_json::Value>,
    ) -> MemoryResult<Vec<GraphNode>>;

    /// Query edges.
    async fn query_edges(
        &self,
        from: Option<&str>,
        to: Option<&str>,
        relation: Option<&str>,
    ) -> MemoryResult<Vec<GraphEdge>>;

    /// Traverse the graph.
    async fn traverse(&self, start: &str, depth: usize) -> MemoryResult<Vec<GraphNode>>;
}

/// Trait for memory context management.
pub trait MemoryContext: Send + Sync {
    /// Get the current session context.
    fn session(&self) -> &SessionContext;

    /// Push a memory to the context.
    fn push(&self, memory: MemoryId) -> impl std::future::Future<Output = MemoryResult<()>> + Send;

    /// Pop the most recent memory.
    fn pop(&self) -> impl std::future::Future<Output = MemoryResult<Option<MemoryId>>> + Send;

    /// Get recent memories.
    fn recent(
        &self,
        count: usize,
    ) -> impl std::future::Future<Output = MemoryResult<Vec<MemoryId>>> + Send;
}

/// Session context for tracking memory operations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionContext {
    pub session_id: String,
    pub user_id: String,
    pub agent_id: Option<String>,
    pub created_at: i64,
    pub last_accessed: i64,
    pub memory_ids: Vec<MemoryId>,
}

impl SessionContext {
    pub fn new(session_id: impl Into<String>, user_id: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            session_id: session_id.into(),
            user_id: user_id.into(),
            agent_id: None,
            created_at: now,
            last_accessed: now,
            memory_ids: Vec::new(),
        }
    }
}
