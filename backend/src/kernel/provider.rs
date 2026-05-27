//! Memory Provider - External Backend Abstraction
//!
//! MemoryProvider sits parallel to MemoryLayer in the Kernel hierarchy.
//! It abstracts external memory systems (Mem0, Zep, Letta) behind a unified interface.

use crate::kernel::error::MemoryResult;
use crate::kernel::types::*;
use serde::{Deserialize, Serialize};

#[async_trait::async_trait]
pub trait MemoryProvider: Send + Sync {
    fn provider_name(&self) -> &str;

    fn capabilities(&self) -> ProviderCapabilities;

    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId>;

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry>;

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>>;

    async fn update(&self, id: &MemoryId, entry: MemoryEntry) -> MemoryResult<()>;

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()>;

    async fn health_check(&self) -> MemoryResult<ProviderHealth>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub supports_vector_search: bool,
    pub supports_graph: bool,
    pub supports_metadata_filter: bool,
    pub supports_eviction: bool,
    pub max_entry_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub status: HealthStatus,
    pub latency_ms: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Builtin,
    Mem0,
    Zep,
    Letta,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Builtin => write!(f, "builtin"),
            ProviderType::Mem0 => write!(f, "mem0"),
            ProviderType::Zep => write!(f, "zep"),
            ProviderType::Letta => write!(f, "letta"),
        }
    }
}
