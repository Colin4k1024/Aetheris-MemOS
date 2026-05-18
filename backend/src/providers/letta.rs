//! Letta Provider - Stub implementation (interface reserved)

use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::provider::*;
use crate::kernel::types::*;

pub struct LettaProvider;

impl LettaProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LettaProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl MemoryProvider for LettaProvider {
    fn provider_name(&self) -> &str {
        "letta"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_vector_search: true,
            supports_graph: false,
            supports_metadata_filter: true,
            supports_eviction: false,
            max_entry_size_bytes: None,
        }
    }

    async fn store(&self, _entry: MemoryEntry) -> MemoryResult<MemoryId> {
        Err(MemoryError::InvalidOperation(
            "Letta provider is not yet implemented".to_string(),
        ))
    }

    async fn retrieve(&self, _id: &MemoryId) -> MemoryResult<MemoryEntry> {
        Err(MemoryError::InvalidOperation(
            "Letta provider is not yet implemented".to_string(),
        ))
    }

    async fn search(&self, _query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        Err(MemoryError::InvalidOperation(
            "Letta provider is not yet implemented".to_string(),
        ))
    }

    async fn update(&self, _id: &MemoryId, _entry: MemoryEntry) -> MemoryResult<()> {
        Err(MemoryError::InvalidOperation(
            "Letta provider is not yet implemented".to_string(),
        ))
    }

    async fn delete(&self, _id: &MemoryId) -> MemoryResult<()> {
        Err(MemoryError::InvalidOperation(
            "Letta provider is not yet implemented".to_string(),
        ))
    }

    async fn health_check(&self) -> MemoryResult<ProviderHealth> {
        Ok(ProviderHealth {
            status: HealthStatus::Unavailable,
            latency_ms: 0,
            message: Some("Letta provider not implemented".to_string()),
        })
    }
}
