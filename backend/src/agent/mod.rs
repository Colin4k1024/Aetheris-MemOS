//! Memory Agent - Automatic Memory Management
//!
//! This module provides automatic memory management capabilities:
//! - Compression: Compress STM to LTM
//! - Merging: Merge similar memories
//! - Forgetting: Intelligent memory eviction

pub mod memory_agent;
pub mod compressor;
pub mod merger;
pub mod forgetter;

pub use memory_agent::AgentMemoryInterface;
pub use compressor::MemoryCompressor;
pub use merger::MemoryMerger;
pub use forgetter::MemoryForGetter;

use crate::kernel::types::*;
use crate::kernel::error::MemoryResult;

/// Memory agent configuration.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum entries in STM before compression
    pub stm_compression_threshold: usize,
    /// Minimum importance to avoid forgetting
    pub min_importance_threshold: f64,
    /// Maximum age in seconds before considering forgetting
    pub max_age_seconds: i64,
    /// Merge similarity threshold (0-1)
    pub merge_similarity_threshold: f64,
    /// Compression batch size
    pub compression_batch_size: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            stm_compression_threshold: 100,
            min_importance_threshold: 0.3,
            max_age_seconds: 86400 * 7, // 7 days
            merge_similarity_threshold: 0.85,
            compression_batch_size: 10,
        }
    }
}
