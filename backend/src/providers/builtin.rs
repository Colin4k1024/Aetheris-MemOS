//! Builtin Provider - wraps internal MemoryLayer chain

use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::provider::*;
use crate::kernel::types::*;
use crate::layers::create_layers;
use crate::kernel::traits::MemoryLayer;

pub struct BuiltinProvider {
    layers: Vec<Box<dyn MemoryLayer + Send + Sync>>,
}

impl BuiltinProvider {
    pub fn new() -> Self {
        Self {
            layers: create_layers(),
        }
    }

    fn find_layer(&self, layer_type: LayerType) -> Option<&(dyn MemoryLayer + Send + Sync)> {
        self.layers.iter().find(|l| l.layer_type() == layer_type).map(|l| l.as_ref())
    }

    fn default_layer(&self) -> MemoryResult<&(dyn MemoryLayer + Send + Sync)> {
        self.layers
            .first()
            .map(|l| l.as_ref())
            .ok_or_else(|| MemoryError::Layer("no layers configured".to_string()))
    }
}

impl Default for BuiltinProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl MemoryProvider for BuiltinProvider {
    fn provider_name(&self) -> &str {
        "builtin"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_vector_search: true,
            supports_graph: true,
            supports_metadata_filter: true,
            supports_eviction: true,
            max_entry_size_bytes: None,
        }
    }

    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId> {
        let layer = self.find_layer(entry.layer)
            .ok_or_else(|| MemoryError::Layer(format!("no layer for type {:?}", entry.layer)))?;
        layer.store(entry).await
    }

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry> {
        for layer in &self.layers {
            match layer.retrieve(id).await {
                Ok(entry) => return Ok(entry),
                Err(MemoryError::NotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(MemoryError::NotFound(format!("memory not found across all layers: {}", id.as_str())))
    }

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        if let Some(layer_type) = query.layer {
            if let Some(layer) = self.find_layer(layer_type) {
                return layer.search(query).await;
            }
        }

        let mut all_results = Vec::new();
        for layer in &self.layers {
            if let Ok(mut results) = layer.search(query).await {
                all_results.append(&mut results);
            }
        }
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        all_results.truncate(query.limit);
        Ok(all_results)
    }

    async fn update(&self, id: &MemoryId, entry: MemoryEntry) -> MemoryResult<()> {
        let layer = self.find_layer(entry.layer)
            .ok_or_else(|| MemoryError::Layer(format!("no layer for type {:?}", entry.layer)))?;
        layer.update(id, entry).await
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        for layer in &self.layers {
            match layer.delete(id).await {
                Ok(()) => return Ok(()),
                Err(MemoryError::NotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(MemoryError::NotFound(format!("memory not found for deletion: {}", id.as_str())))
    }

    async fn health_check(&self) -> MemoryResult<ProviderHealth> {
        Ok(ProviderHealth {
            status: HealthStatus::Healthy,
            latency_ms: 0,
            message: None,
        })
    }
}
