//! Short-Term Memory (STM) Layer Implementation
//!
//! STM provides ephemeral, fast access memory for active sessions.

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

use crate::kernel::types::*;
use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::traits::{MemoryLayer, LayerStats};

/// Short-term memory layer implementation.
/// 
/// STM stores ephemeral data that is fast to access but has limited capacity.
pub struct StmMemoryLayer {
    cache: Arc<RwLock<HashMap<String, MemoryEntry>>>,
    max_capacity: usize,
}

impl StmMemoryLayer {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_capacity: 1000,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_capacity: capacity,
        }
    }
}

impl Default for StmMemoryLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl MemoryLayer for StmMemoryLayer {
    fn layer_type(&self) -> LayerType {
        LayerType::Stm
    }

    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId> {
        let mut cache = self.cache.write().await;
        
        if cache.len() >= self.max_capacity {
            return Err(MemoryError::ResourceConstraint(
                "STM cache is at capacity".to_string()
            ));
        }
        
        let id = entry.id.clone();
        cache.insert(id.0.clone(), entry);
        Ok(id)
    }

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry> {
        let cache = self.cache.read().await;
        cache
            .get(&id.0)
            .cloned()
            .ok_or_else(|| MemoryError::NotFound(format!("Memory not found: {}", id.0)))
    }

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        let cache = self.cache.read().await;
        let mut results = Vec::new();
        
        if let Some(text) = &query.text {
            for entry in cache.values() {
                let content_match = match &entry.content {
                    MemoryContent::Text(s) => s.contains(text),
                    MemoryContent::Json(v) => serde_json::to_string(v)
                        .map(|s| s.contains(text))
                        .unwrap_or(false),
                    _ => false,
                };
                
                if content_match {
                    results.push(MemoryMatch {
                        entry: entry.clone(),
                        score: 1.0,
                        highlights: vec![text.clone()],
                    });
                }
            }
        }
        
        if let Some(ref user_id) = query.filters.user_id {
            results.retain(|m| m.entry.metadata.user_id.as_ref() == Some(user_id));
        }
        
        results.truncate(query.limit);
        
        Ok(results)
    }

    async fn update(&self, id: &MemoryId, mut entry: MemoryEntry) -> MemoryResult<()> {
        let mut cache = self.cache.write().await;
        
        if !cache.contains_key(&id.0) {
            return Err(MemoryError::NotFound(format!("Memory not found: {}", id.0)));
        }
        
        entry.updated_at = chrono::Utc::now().timestamp();
        cache.insert(id.0.clone(), entry);
        Ok(())
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        let mut cache = self.cache.write().await;
        
        if cache.remove(&id.0).is_none() {
            return Err(MemoryError::NotFound(format!("Memory not found: {}", id.0)));
        }
        
        Ok(())
    }

    async fn stats(&self) -> MemoryResult<LayerStats> {
        let cache = self.cache.read().await;
        
        let total_size: u64 = cache.values()
            .map(|e| serde_json::to_string(e).map(|s| s.len() as u64).unwrap_or(0))
            .sum();
        
        let total_access: u64 = cache.values()
            .map(|e| e.metadata.access_count as u64)
            .sum();
        
        let avg_access = if cache.is_empty() {
            0.0
        } else {
            total_access as f64 / cache.len() as f64
        };
        
        Ok(LayerStats {
            entry_count: cache.len(),
            size_bytes: total_size,
            avg_access_count: avg_access,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stm_store_and_retrieve() {
        let layer = StmMemoryLayer::new();
        
        let entry = MemoryEntry::new(
            LayerType::Stm,
            MemoryContent::Text("test content".to_string()),
        );
        
        let id = layer.store(entry.clone()).await.unwrap();
        let retrieved = layer.retrieve(&id).await.unwrap();
        
        assert_eq!(retrieved.id, id);
    }

    #[tokio::test]
    async fn test_stm_search() {
        let layer = StmMemoryLayer::new();
        
        let entry = MemoryEntry::new(
            LayerType::Stm,
            MemoryContent::Text("hello world".to_string()),
        );
        
        layer.store(entry).await.unwrap();
        
        let query = MemoryQuery {
            text: Some("hello".to_string()),
            ..Default::default()
        };
        
        let results = layer.search(&query).await.unwrap();
        assert!(!results.is_empty());
    }
}
