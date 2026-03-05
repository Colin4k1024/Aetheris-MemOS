//! Long-Term Memory (LTM) Layer Implementation
//!
//! LTM provides persistent, indexed memory for storing knowledge and experiences.

use crate::kernel::types::*;
use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::traits::{MemoryLayer, LayerStats};
use crate::db::ltm::LTMRepository;
use crate::db::pool;

/// Long-term memory layer implementation.
/// 
/// LTM stores persistent data with vector indexing for semantic search.
/// It bridges to the existing LTMRepository.
pub struct LtmMemoryLayer {
    repository: LTMRepository,
}

impl LtmMemoryLayer {
    pub fn new() -> Self {
        Self {
            repository: LTMRepository,
        }
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
        let content = match &entry.content {
            MemoryContent::Text(text) => text.clone(),
            MemoryContent::Json(json) => json.to_string(),
            MemoryContent::Binary(_) => {
                return Err(MemoryError::InvalidOperation(
                    "Binary content not supported in LTM".to_string()
                ));
            }
            MemoryContent::Graph(_) => {
                return Err(MemoryError::InvalidOperation(
                    "Graph content not supported in LTM, use KG layer".to_string()
                ));
            }
        };

        // Generate embedding (in production, this would call embedding service)
        let embedding = vec![0.0f32; 768]; // Placeholder
        let embedding_str = serde_json::to_string(&embedding)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;

        let entry_id = self.repository
            .create_knowledge_entry(
                &entry.id.0,
                "memory_kernel",
                None,
                &content,
                "text",
                &embedding,
                "placeholder",
                768,
                Some(entry.metadata.importance),
            )
            .await
            .map_err(|e| MemoryError::Storage(e.to_string()))?;

        Ok(MemoryId(entry_id))
    }

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry> {
        let entry = self.repository
            .get_entry_by_id(&id.0)
            .await
            .map_err(|e| MemoryError::Storage(e.to_string()))?
            .ok_or_else(|| MemoryError::NotFound(format!("Memory not found: {}", id.0)))?;

        let content = MemoryContent::Text(entry.content);
        let metadata = MemoryMetadata {
            user_id: None,
            session_id: None,
            agent_id: None,
            tags: vec![],
            importance: entry.quality_score.unwrap_or(0.0),
            access_count: entry.access_count as u32,
            last_accessed: entry.last_accessed_at
                .and_then(|s| s.parse::<i64>().ok()),
            expires_at: None,
            source: Some(entry.source_type),
            extra: Default::default(),
        };

        Ok(MemoryEntry {
            id: id.clone(),
            layer: LayerType::Ltm,
            content,
            metadata,
            created_at: entry.created_at.parse().unwrap_or(0),
            updated_at: entry.updated_at.parse().unwrap_or(0),
        })
    }

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        // For now, this is a simplified implementation
        // In production, would use vector similarity search via Qdrant
        let entries = self.repository
            .list_entries(0, 100)
            .await
            .map_err(|e| MemoryError::Storage(e.to_string()))?;

        let mut results = Vec::new();
        
        for entry in entries {
            // Simple text match if query has text
            let score = if let Some(text) = &query.text {
                if entry.content.contains(text) {
                    1.0
                } else {
                    continue;
                }
            } else {
                entry.relevance_score.unwrap_or(0.5)
            };

            let content = MemoryContent::Text(entry.content);
            let memory_entry = MemoryEntry {
                id: MemoryId(entry.entry_id),
                layer: LayerType::Ltm,
                content,
                metadata: MemoryMetadata::default(),
                created_at: entry.created_at.parse().unwrap_or(0),
                updated_at: entry.updated_at.parse().unwrap_or(0),
            };

            results.push(MemoryMatch {
                entry: memory_entry,
                score,
                highlights: vec![],
            });
        }

        results.truncate(query.limit);
        Ok(results)
    }

    async fn update(&self, id: &MemoryId, entry: MemoryEntry) -> MemoryResult<()> {
        // LTM entries are typically immutable, but we can update metadata
        // This is a simplified implementation
        Ok(())
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        self.repository
            .delete_entry(&id.0)
            .await
            .map_err(|e| MemoryError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn stats(&self) -> MemoryResult<LayerStats> {
        let pool = pool();
        
        let count: (i64,) = sqlx::fetch_optional(
            sqlx::query_as("SELECT COUNT(*) FROM knowledge_entries"),
            &*pool
        )
        .await
        .map_err(|e| MemoryError::Storage(e.to_string()))?
        .map(|r| r.0)
        .unwrap_or(0);

        Ok(LayerStats {
            entry_count: count as usize,
            size_bytes: 0, // Would need to calculate from DB
            avg_access_count: 0.0,
        })
    }
}
