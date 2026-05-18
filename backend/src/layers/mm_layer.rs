//! Multimodal Memory (MM) Layer Implementation
//!
//! MM provides storage for images, audio, video, and other media types.
//! Currently uses in-memory storage; will integrate Qdrant + PostgreSQL in production.

use std::collections::HashMap;
use tokio::sync::RwLock;
use crate::kernel::types::*;
use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::traits::{MemoryLayer, LayerStats};

pub struct MmMemoryLayer {
    storage: RwLock<HashMap<String, MemoryEntry>>,
    max_storage_size: usize,
}

impl MmMemoryLayer {
    pub fn new() -> Self {
        Self {
            storage: RwLock::new(HashMap::new()),
            max_storage_size: 100 * 1024 * 1024, // 100MB default
        }
    }

    async fn get_storage_size(&self) -> usize {
        let storage = self.storage.read().await;
        storage.values()
            .map(|e| {
                if let MemoryContent::Binary(data) = &e.content {
                    data.len()
                } else {
                    0
                }
            })
            .sum()
    }
}

impl Default for MmMemoryLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl MemoryLayer for MmMemoryLayer {
    fn layer_type(&self) -> LayerType {
        LayerType::Mm
    }

    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId> {
        match &entry.content {
            MemoryContent::Binary(data) if !data.is_empty() => {
                let current_size = self.get_storage_size().await;
                if current_size + data.len() > self.max_storage_size {
                    return Err(MemoryError::InvalidOperation(
                        "Storage size limit exceeded".to_string()
                    ));
                }
            }
            MemoryContent::Binary(_) => return Err(MemoryError::InvalidOperation(
                "Cannot store empty binary data".to_string()
            )),
            _ => return Err(MemoryError::InvalidOperation(
                "MM layer requires Binary content".to_string()
            )),
        }

        let id = entry.id.clone();
        let mut storage = self.storage.write().await;
        storage.insert(id.0.clone(), entry);
        Ok(id)
    }

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry> {
        let storage = self.storage.read().await;
        storage.get(&id.0)
            .cloned()
            .ok_or_else(|| MemoryError::NotFound(format!("MM entry not found: {}", id.0)))
    }

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        let storage = self.storage.read().await;
        let mut results = Vec::new();

        for entry in storage.values() {
            if let MemoryContent::Binary(_) = &entry.content {
                results.push(MemoryMatch {
                    entry: entry.clone(),
                    score: 0.5,
                    highlights: vec![],
                });
            }
        }

        results.truncate(query.limit);
        Ok(results)
    }

    async fn update(&self, id: &MemoryId, entry: MemoryEntry) -> MemoryResult<()> {
        let mut storage = self.storage.write().await;
        if !storage.contains_key(&id.0) {
            return Err(MemoryError::NotFound(format!("MM entry not found: {}", id.0)));
        }
        storage.insert(id.0.clone(), entry);
        Ok(())
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        let mut storage = self.storage.write().await;
        storage.remove(&id.0)
            .ok_or_else(|| MemoryError::NotFound(format!("MM entry not found: {}", id.0)))?;
        Ok(())
    }

    async fn stats(&self) -> MemoryResult<LayerStats> {
        let storage = self.storage.read().await;
        let storage_size = storage.values()
            .map(|e| {
                if let MemoryContent::Binary(data) = &e.content {
                    data.len() as u64
                } else {
                    0
                }
            })
            .sum::<u64>();

        Ok(LayerStats {
            entry_count: storage.len(),
            size_bytes: storage_size,
            avg_access_count: 0.0,
        })
    }
}
