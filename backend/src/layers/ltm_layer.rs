//! Long-Term Memory (LTM) Layer Implementation
//!
//! LTM provides persistent storage using Qdrant vector database and PostgreSQL.

use std::collections::HashMap;
use tokio::sync::RwLock;
use crate::kernel::types::*;
use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::traits::{MemoryLayer, LayerStats};
use crate::services::qdrant::get_qdrant_client;
use crate::db::ltm::LTMRepository;
use crate::db::pool;
use qdrant_client::qdrant::{vectors_config, VectorParams, Distance};

/// Long-Term Memory layer implementation.
///
/// LTM stores persistent knowledge entries with vector embeddings.
/// Uses Qdrant for vector similarity search and PostgreSQL for metadata.
pub struct LtmMemoryLayer {
    /// In-memory cache for recent accesses
    cache: RwLock<HashMap<String, MemoryEntry>>,
    /// Maximum cache size
    max_cache_size: usize,
}

impl LtmMemoryLayer {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            max_cache_size: 1000,
        }
    }

    /// Ensure Qdrant collection exists
    pub async fn ensure_collection(&self) -> MemoryResult<()> {
        let client = get_qdrant_client()
            .map_err(|e| MemoryError::Internal(format!("Qdrant not available: {}", e)))?;

        let collection_name = "long_term_memory";

        // Check if collection exists
        let exists = client.collection_exists(collection_name)
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to check collection: {}", e)))?;

        if !exists {
            // Create collection
            client.create_collection(collection_name, vectors_config::Config::Params(
                VectorParams {
                    size: 768, // Default embedding dimension
                    distance: Distance::Cosine.into(),
                }
            ))
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to create collection: {}", e)))?;
        }

        Ok(())
    }

    /// Store entry to cache
    async fn cache_entry(&self, entry: MemoryEntry) {
        let mut cache = self.cache.write().await;
        if cache.len() >= self.max_cache_size {
            // Remove oldest entry
            if let Some(key) = cache.keys().next().cloned() {
                cache.remove(&key);
            }
        }
        cache.insert(entry.id.0.clone(), entry);
    }

    /// Get entry from cache
    async fn get_from_cache(&self, id: &str) -> Option<MemoryEntry> {
        let cache = self.cache.read().await;
        cache.get(id).cloned()
    }
}

impl Default for LtmMemoryLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryLayer for LtmMemoryLayer {
    fn layer_type(&self) -> LayerType {
        LayerType::Ltm
    }

    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId> {
        // First, store metadata to PostgreSQL
        let pool = pool();

        // Extract content as text for embedding
        let content = match &entry.content {
            MemoryContent::Text(text) => text.clone(),
            MemoryContent::Json(json) => json.to_string(),
            _ => entry.id.0.clone(),
        };

        // Store to database
        let entry_id = LTMRepository::create(
            pool,
            &entry.id.0,
            &content,
            "memory_layer".to_string(),
            None,
            None,
            None,
            None,
            1.0,
            1.0,
        )
        .await
        .map_err(|e| MemoryError::Internal(format!("Failed to store LTM: {}", e)))?;

        // Cache the entry
        self.cache_entry(entry.clone()).await;

        Ok(MemoryId(entry_id))
    }

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry> {
        // Try cache first
        if let Some(entry) = self.get_from_cache(&id.0).await {
            return Ok(entry);
        }

        // Fetch from database
        let pool = pool();
        let entry_opt = LTMRepository::get_by_id(pool, &id.0)
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to retrieve LTM: {}", e)))?;

        match entry_opt {
            Some(ltm_entry) => {
                let entry = MemoryEntry {
                    id: MemoryId(ltm_entry.entry_id),
                    layer: LayerType::Ltm,
                    content: MemoryContent::Text(ltm_entry.content),
                    metadata: MemoryMetadata {
                        created_at: ltm_entry.created_at.timestamp(),
                        updated_at: ltm_entry.updated_at.timestamp(),
                        ..Default::default()
                    },
                    created_at: ltm_entry.created_at.timestamp(),
                    updated_at: ltm_entry.updated_at.timestamp(),
                };

                // Cache for future access
                self.cache_entry(entry.clone()).await;

                Ok(entry)
            }
            None => Err(MemoryError::NotFound(format!("LTM entry not found: {}", id.0))),
        }
    }

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        let client = get_qdrant_client()
            .map_err(|e| MemoryError::Internal(format!("Qdrant not available: {}", e)))?;

        // Generate query embedding
        let embedding_service = crate::services::embedding::get_embedding_service()
            .map_err(|e| MemoryError::Internal(format!("Embedding service not available: {}", e)))?;

        // Use query text or default
        let query_text = query.text.as_deref().unwrap_or("");

        let query_vector = embedding_service
            .generate_embedding(query_text)
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to generate query embedding: {}", e)))?;

        // Search Qdrant
        let results = client
            .search(
                "long_term_memory",
                qdrant_client::qdrant::SearchParams {
                    limit: query.limit as u64,
                    ..Default::default()
                },
                Some(query_vector),
                None,
                None,
            )
            .await
            .map_err(|e| MemoryError::Internal(format!("Qdrant search failed: {}", e)))?;

        let mut matches = Vec::new();
        for result in results.result {
            let score = result.score;
            let id = result.id.unwrap_or_default();

            // Fetch full entry from database
            if let Ok(Some(ltm_entry)) = LTMRepository::get_by_id(pool(), &id.uuid_or_default()) {
                let entry = MemoryEntry {
                    id: MemoryId(ltm_entry.entry_id),
                    layer: LayerType::Ltm,
                    content: MemoryContent::Text(ltm_entry.content),
                    metadata: MemoryMetadata {
                        created_at: ltm_entry.created_at.timestamp(),
                        updated_at: ltm_entry.updated_at.timestamp(),
                        ..Default::default()
                    },
                    created_at: ltm_entry.created_at.timestamp(),
                    updated_at: ltm_entry.updated_at.timestamp(),
                };

                matches.push(MemoryMatch {
                    entry,
                    score: score as f64,
                    highlights: vec![],
                });
            }
        }

        Ok(matches)
    }

    async fn update(&self, id: &MemoryId, entry: MemoryEntry) -> MemoryResult<()> {
        let pool = pool();
        let content = match &entry.content {
            MemoryContent::Text(text) => text.clone(),
            MemoryContent::Json(json) => json.to_string(),
            _ => return Err(MemoryError::InvalidOperation("Invalid content type".to_string())),
        };

        LTMRepository::update_content(pool, &id.0, &content)
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to update LTM: {}", e)))?;

        // Update cache
        self.cache_entry(entry).await;

        Ok(())
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        let pool = pool();

        LTMRepository::delete(pool, &id.0)
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to delete LTM: {}", e)))?;

        // Remove from cache
        let mut cache = self.cache.write().await;
        cache.remove(&id.0);

        Ok(())
    }

    async fn stats(&self) -> MemoryResult<LayerStats> {
        let pool = pool();

        let count = LTMRepository::count(pool)
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to get LTM stats: {}", e)))?;

        let cache_size = self.cache.read().await.len();

        Ok(LayerStats {
            entry_count: count + cache_size as i64,
            size_bytes: 0, // Would need to calculate from DB
            avg_access_count: 0.0,
        })
    }
}
