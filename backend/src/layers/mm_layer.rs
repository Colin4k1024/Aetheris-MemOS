//! Multimodal Memory (MM) Layer Implementation
//!
//! MM provides storage for images, audio, video, and other media types.

use std::collections::HashMap;
use tokio::sync::RwLock;
use crate::kernel::types::*;
use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::traits::{MemoryLayer, LayerStats};
use crate::services::qdrant::get_qdrant_client;
use crate::db::mm::MMRepository;
use crate::db::pool;
use qdrant_client::qdrant::{vectors_config, VectorParams, Distance};

/// Multimodal memory layer implementation.
///
/// MM stores media data (images, audio, video) with vector embeddings.
/// Uses Qdrant for vector similarity search and PostgreSQL for metadata.
pub struct MmMemoryLayer {
    storage: RwLock<HashMap<String, MemoryEntry>>,
    /// Maximum storage size in bytes
    max_storage_size: usize,
}

impl MmMemoryLayer {
    pub fn new() -> Self {
        Self {
            storage: RwLock::new(HashMap::new()),
            max_storage_size: 100 * 1024 * 1024, // 100MB default
        }
    }

    /// Ensure Qdrant collection exists
    pub async fn ensure_collection(&self) -> MemoryResult<()> {
        let client = get_qdrant_client()
            .map_err(|e| MemoryError::Internal(format!("Qdrant not available: {}", e)))?;

        let collection_name = "multimodal_memory";

        let exists = client.collection_exists(collection_name)
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to check collection: {}", e)))?;

        if !exists {
            client.create_collection(collection_name, vectors_config::Config::Params(
                VectorParams {
                    size: 768,
                    distance: Distance::Cosine.into(),
                }
            ))
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to create collection: {}", e)))?;
        }

        Ok(())
    }

    /// Get current storage size
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

impl MemoryLayer for MmMemoryLayer {
    fn layer_type(&self) -> LayerType {
        LayerType::Mm
    }

    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId> {
        // Validate content is binary
        let data = match &entry.content {
            MemoryContent::Binary(data) if !data.is_empty() => data.clone(),
            MemoryContent::Binary(_) => return Err(MemoryError::InvalidOperation(
                "Cannot store empty binary data".to_string()
            )),
            _ => return Err(MemoryError::InvalidOperation(
                "MM layer requires Binary content".to_string()
            )),
        };

        // Check storage size limit
        let current_size = self.get_storage_size().await;
        if current_size + data.len() > self.max_storage_size {
            return Err(MemoryError::InvalidOperation(
                "Storage size limit exceeded".to_string()
            ));
        }

        // Generate embedding for the binary data (using description or empty)
        let embedding_service = crate::services::embedding::get_embedding_service()
            .map_err(|e| MemoryError::Internal(format!("Embedding service not available: {}", e)))?;

        // For binary data, we would typically extract features
        // Here we use a placeholder - in production, use a vision model
        let query_text = entry.metadata.description.as_deref().unwrap_or("");
        let embedding = embedding_service
            .generate_embedding(query_text)
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to generate embedding: {}", e)))?;

        // Store metadata to PostgreSQL
        let pool = pool();
        MMRepository::create(
            pool,
            &entry.id.0,
            "binary".to_string(),
            None,
            Some(&data),
            None,
            serde_json::to_string(&embedding).ok().as_deref(),
            Some(1.0),
        )
        .await
        .map_err(|e| MemoryError::Internal(format!("Failed to store MM metadata: {}", e)))?;

        // Store to in-memory
        let mut storage = self.storage.write().await;
        let id = entry.id.clone();
        storage.insert(id.0.clone(), entry);

        Ok(id)
    }

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry> {
        // Try in-memory first
        {
            let storage = self.storage.read().await;
            if let Some(entry) = storage.get(&id.0) {
                return Ok(entry.clone());
            }
        }

        // Fetch from database
        let pool = pool();
        let mm_entry = MMRepository::get_by_id(pool, &id.0)
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to retrieve MM: {}", e)))?
            .ok_or_else(|| MemoryError::NotFound(format!("Memory not found: {}", id.0)))?;

        // Reconstruct entry
        let data = mm_entry.binary_data.unwrap_or_default();
        let entry = MemoryEntry {
            id: MemoryId(mm_entry.entry_id),
            layer: LayerType::Mm,
            content: MemoryContent::Binary(data),
            metadata: MemoryMetadata {
                description: mm_entry.description,
                ..Default::default()
            },
            created_at: mm_entry.created_at.timestamp(),
            updated_at: mm_entry.updated_at.timestamp(),
        };

        // Cache in memory
        let mut storage = self.storage.write().await;
        storage.insert(id.0.clone(), entry.clone());

        Ok(entry)
    }

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        let client = get_qdrant_client()
            .map_err(|e| MemoryError::Internal(format!("Qdrant not available: {}", e)))?;

        // Generate query embedding
        let embedding_service = crate::services::embedding::get_embedding_service()
            .map_err(|e| MemoryError::Internal(format!("Embedding service not available: {}", e)))?;

        let query_text = query.text.as_deref().unwrap_or("");
        let query_vector = embedding_service
            .generate_embedding(query_text)
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to generate query embedding: {}", e)))?;

        // Search Qdrant
        let results = client
            .search(
                "multimodal_memory",
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
            if let Ok(Some(mm_entry)) = MMRepository::get_by_id(pool(), &id.uuid_or_default()) {
                let data = mm_entry.binary_data.unwrap_or_default();
                let entry = MemoryEntry {
                    id: MemoryId(mm_entry.entry_id),
                    layer: LayerType::Mm,
                    content: MemoryContent::Binary(data),
                    metadata: MemoryMetadata {
                        description: mm_entry.description,
                        ..Default::default()
                    },
                    created_at: mm_entry.created_at.timestamp(),
                    updated_at: mm_entry.updated_at.timestamp(),
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
        let mut storage = self.storage.write().await;

        if !storage.contains_key(&id.0) {
            return Err(MemoryError::NotFound(format!("Memory not found: {}", id.0)));
        }

        storage.insert(id.0.clone(), entry);
        Ok(())
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        // Remove from memory
        {
            let mut storage = self.storage.write().await;
            storage.remove(&id.0);
        }

        // Remove from database
        let pool = pool();
        MMRepository::delete(pool, &id.0)
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to delete MM: {}", e)))?;

        Ok(())
    }

    async fn stats(&self) -> MemoryResult<LayerStats> {
        let pool = pool();

        let count = MMRepository::count(pool)
            .await
            .map_err(|e| MemoryError::Internal(format!("Failed to get MM stats: {}", e)))?;

        let storage_size = self.get_storage_size().await;

        Ok(LayerStats {
            entry_count: count as i64,
            size_bytes: storage_size as i64,
            avg_access_count: 0.0,
        })
    }
}
