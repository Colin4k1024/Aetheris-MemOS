//! Long-Term Memory (LTM) Layer Implementation
//!
//! LTM provides persistent storage for long-term knowledge entries.
//! Currently uses in-memory storage; will integrate Qdrant + PostgreSQL in production.

use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::traits::{LayerStats, MemoryLayer};
use crate::kernel::types::*;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct LtmMemoryLayer {
    store: RwLock<HashMap<String, MemoryEntry>>,
    max_cache_size: usize,
}

impl LtmMemoryLayer {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            max_cache_size: 10000,
        }
    }
}

impl Default for LtmMemoryLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl MemoryLayer for LtmMemoryLayer {
    fn layer_type(&self) -> LayerType {
        LayerType::Ltm
    }

    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId> {
        let id = entry.id.clone();
        let mut store = self.store.write().await;
        if store.len() >= self.max_cache_size {
            if let Some(key) = store.keys().next().cloned() {
                store.remove(&key);
            }
        }
        store.insert(id.0.clone(), entry);
        Ok(id)
    }

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry> {
        let store = self.store.read().await;
        store
            .get(&id.0)
            .cloned()
            .ok_or_else(|| MemoryError::NotFound(format!("LTM entry not found: {}", id.0)))
    }

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        let store = self.store.read().await;
        let mut results = Vec::new();

        if let Some(text) = &query.text {
            let lower = text.to_lowercase();
            for entry in store.values() {
                let content_text = match &entry.content {
                    MemoryContent::Text(t) => t.clone(),
                    MemoryContent::Json(j) => j.to_string(),
                    _ => continue,
                };
                if content_text.to_lowercase().contains(&lower) {
                    results.push(MemoryMatch {
                        entry: entry.clone(),
                        score: 1.0,
                        highlights: vec![text.clone()],
                    });
                }
            }
        }

        results.truncate(query.limit);
        Ok(results)
    }

    async fn update(&self, id: &MemoryId, entry: MemoryEntry) -> MemoryResult<()> {
        let mut store = self.store.write().await;
        if !store.contains_key(&id.0) {
            return Err(MemoryError::NotFound(format!(
                "LTM entry not found: {}",
                id.0
            )));
        }
        store.insert(id.0.clone(), entry);
        Ok(())
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        let mut store = self.store.write().await;
        store
            .remove(&id.0)
            .ok_or_else(|| MemoryError::NotFound(format!("LTM entry not found: {}", id.0)))?;
        Ok(())
    }

    async fn stats(&self) -> MemoryResult<LayerStats> {
        let store = self.store.read().await;
        Ok(LayerStats {
            entry_count: store.len(),
            size_bytes: 0,
            avg_access_count: 0.0,
        })
    }
}
