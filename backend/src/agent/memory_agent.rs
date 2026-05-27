//! Memory Agent - Main Interface
//!
//! This module provides the main interface for Agent-Memory integration.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::agent::compressor::MemoryCompressor;
use crate::agent::forgetter::MemoryForGetter;
use crate::agent::merger::MemoryMerger;
use crate::agent::AgentConfig;
use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::traits::MemoryKernel;
use crate::kernel::types::*;

/// Main Memory Agent that provides automatic memory management.
///
/// This agent handles:
/// - remember(): Store new memories
/// - recall(): Retrieve relevant memories
/// - augment(): Enhance reasoning with memory
/// - forget(): Intelligent memory eviction
pub struct MemoryAgent {
    kernel: Arc<dyn MemoryKernel>,
    config: AgentConfig,
    compressor: MemoryCompressor,
    merger: MemoryMerger,
    forgetter: MemoryForGetter,
}

impl MemoryAgent {
    pub fn new(kernel: Arc<dyn MemoryKernel>) -> Self {
        Self::with_config(kernel, AgentConfig::default())
    }

    pub fn with_config(kernel: Arc<dyn MemoryKernel>, config: AgentConfig) -> Self {
        let compression_batch_size = config.compression_batch_size;
        let merge_similarity_threshold = config.merge_similarity_threshold;
        let min_importance_threshold = config.min_importance_threshold;
        let max_age_seconds = config.max_age_seconds;
        Self {
            kernel,
            config,
            compressor: MemoryCompressor::new(compression_batch_size),
            merger: MemoryMerger::new(merge_similarity_threshold),
            forgetter: MemoryForGetter::new(min_importance_threshold, max_age_seconds),
        }
    }

    /// Remember new information.
    ///
    /// This stores the content in the appropriate memory layer based on
    /// the context and configuration.
    pub async fn remember(
        &self,
        user_id: &str,
        session_id: Option<&str>,
        agent_id: Option<&str>,
        content: impl Into<MemoryContent>,
    ) -> MemoryResult<MemoryId> {
        let content = content.into();

        // Determine layer based on content type
        let layer = match &content {
            MemoryContent::Text(_) => LayerType::Stm, // Start with STM
            MemoryContent::Json(_) => LayerType::Ltm,
            MemoryContent::Binary(_) => LayerType::Mm,
            MemoryContent::Graph(_) => LayerType::Kg,
        };

        let entry = MemoryEntry::new(layer, content).with_metadata(MemoryMetadata {
            user_id: Some(user_id.to_string()),
            session_id: session_id.map(String::from),
            agent_id: agent_id.map(String::from),
            tags: vec![],
            importance: 0.5, // Default importance
            access_count: 0,
            last_accessed: None,
            expires_at: None,
            source: Some("agent".to_string()),
            extra: Default::default(),
        });

        self.kernel.store(entry).await
    }

    /// Recall relevant memories based on query.
    pub async fn recall(&self, user_id: &str, query: &str) -> MemoryResult<Vec<MemoryMatch>> {
        let memory_query = MemoryQuery {
            layer: None,
            text: Some(query.to_string()),
            embedding: None,
            filters: MemoryFilters {
                user_id: Some(user_id.to_string()),
                ..Default::default()
            },
            limit: 10,
            offset: 0,
        };

        self.kernel.search(&memory_query).await
    }

    /// Augment reasoning with relevant memories.
    ///
    /// This is the core method for RAG (Retrieval-Augmented Generation).
    pub async fn augment(&self, user_id: &str, task: &str) -> MemoryResult<MemoryAugmentation> {
        // Search for relevant memories
        let matches = self.recall(user_id, task).await?;

        // Extract context from memories
        let context: Vec<String> = matches
            .iter()
            .map(|m| match &m.entry.content {
                MemoryContent::Text(s) => s.clone(),
                MemoryContent::Json(j) => j.to_string(),
                _ => String::new(),
            })
            .filter(|s| !s.is_empty())
            .collect();

        // Build augmentation result
        Ok(MemoryAugmentation {
            context,
            source_memories: matches.iter().map(|m| m.entry.id.clone()).collect(),
            confidence: if matches.is_empty() {
                0.0
            } else {
                matches.iter().map(|m| m.score).sum::<f64>() / matches.len() as f64
            },
        })
    }

    /// Trigger memory forgetting process.
    pub async fn forget(&self, user_id: &str) -> MemoryResult<ForgetResult> {
        let query = MemoryQuery {
            layer: None,
            text: None,
            embedding: None,
            filters: MemoryFilters {
                user_id: Some(user_id.to_string()),
                ..Default::default()
            },
            limit: 1000, // Process in batches
            offset: 0,
        };

        let all_memories = self.kernel.search(&query).await?;

        let entries: Vec<MemoryEntry> = all_memories.iter().map(|m| m.entry.clone()).collect();
        self.forgetter.evict(&entries).await
    }

    /// Run periodic maintenance.
    ///
    /// This should be called periodically to:
    /// - Compress STM to LTM
    /// - Merge similar memories
    /// - Evict low-importance memories
    pub async fn maintain(&self, user_id: &str) -> MemoryResult<MaintenanceResult> {
        let mut results = MaintenanceResult::default();

        // Get STM memories for compression
        let stm_query = MemoryQuery {
            layer: Some(LayerType::Stm),
            filters: MemoryFilters {
                user_id: Some(user_id.to_string()),
                ..Default::default()
            },
            limit: self.config.stm_compression_threshold,
            ..Default::default()
        };

        let stm_memories = self.kernel.search(&stm_query).await?;

        if stm_memories.len() >= self.config.stm_compression_threshold {
            // Compress old STM to LTM
            let entries: Vec<MemoryEntry> = stm_memories.iter().map(|m| m.entry.clone()).collect();
            let compressed = self.compressor.compress(&entries).await;
            results.compressed_count = compressed.len();
        }

        // Run forgetting
        let forget_result = self.forget(user_id).await?;
        results.forgotten_count = forget_result.evicted_count;

        Ok(results)
    }
}

/// Memory augmentation result for RAG.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryAugmentation {
    pub context: Vec<String>,
    pub source_memories: Vec<MemoryId>,
    pub confidence: f64,
}

/// Result of forgetting operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ForgetResult {
    pub evicted_count: usize,
    pub reasons: Vec<String>,
}

/// Result of maintenance operation.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MaintenanceResult {
    pub compressed_count: usize,
    pub merged_count: usize,
    pub forgotten_count: usize,
}

/// Trait for Agent Memory Interface (from the plan).
///
/// This trait provides a standardized interface for agent runtimes
/// to interact with memory.
pub trait AgentMemoryInterface: Send + Sync {
    /// Store a memory
    fn remember(
        &self,
        user_id: &str,
        session_id: Option<&str>,
        content: &str,
    ) -> impl std::future::Future<Output = MemoryResult<MemoryId>> + Send;

    /// Retrieve memories
    fn recall(
        &self,
        user_id: &str,
        query: &str,
    ) -> impl std::future::Future<Output = MemoryResult<Vec<MemoryMatch>>> + Send;

    /// Augment with relevant memories
    fn augment(
        &self,
        user_id: &str,
        task: &str,
    ) -> impl std::future::Future<Output = MemoryResult<MemoryAugmentation>> + Send;

    /// Forget memories based on policy
    fn forget(
        &self,
        user_id: &str,
    ) -> impl std::future::Future<Output = MemoryResult<ForgetResult>> + Send;
}

impl AgentMemoryInterface for MemoryAgent {
    async fn remember(
        &self,
        user_id: &str,
        session_id: Option<&str>,
        content: &str,
    ) -> MemoryResult<MemoryId> {
        self.remember(user_id, session_id, None, content.to_string())
            .await
    }

    async fn recall(&self, user_id: &str, query: &str) -> MemoryResult<Vec<MemoryMatch>> {
        self.recall(user_id, query).await
    }

    async fn augment(&self, user_id: &str, task: &str) -> MemoryResult<MemoryAugmentation> {
        self.augment(user_id, task).await
    }

    async fn forget(&self, user_id: &str) -> MemoryResult<ForgetResult> {
        self.forget(user_id).await
    }
}
