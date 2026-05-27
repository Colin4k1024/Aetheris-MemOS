//! Hybrid Search Types
//!
//! Data types for GraphRAG hybrid retrieval: fusion strategies,
//! search configuration, and result provenance.

use crate::kernel::types::MemoryEntry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FusionStrategy {
    VectorFirst,
    GraphFirst,
    ReciprocalRankFusion,
}

impl Default for FusionStrategy {
    fn default() -> Self {
        Self::ReciprocalRankFusion
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchConfig {
    pub default_strategy: FusionStrategy,
    pub vector_weight: f64,
    pub graph_weight: f64,
    pub rrf_k: u32,
    pub max_results: usize,
    pub timeout_ms: u64,
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            default_strategy: FusionStrategy::ReciprocalRankFusion,
            vector_weight: 0.5,
            graph_weight: 0.5,
            rrf_k: 60,
            max_results: 20,
            timeout_ms: 3000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchResult {
    pub entry: MemoryEntry,
    pub score: f64,
    pub provenance: SearchProvenance,
    pub vector_rank: Option<u32>,
    pub graph_rank: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchProvenance {
    VectorOnly,
    GraphOnly,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchRequest {
    pub query: String,
    pub strategy: Option<FusionStrategy>,
    pub vector_weight: Option<f64>,
    pub graph_weight: Option<f64>,
    pub limit: Option<usize>,
    pub filters: Option<crate::kernel::types::MemoryFilters>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchMetadata {
    pub strategy_used: FusionStrategy,
    pub vector_count: usize,
    pub graph_count: usize,
    pub fused_count: usize,
    pub vector_latency_ms: u64,
    pub graph_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchResponse {
    pub results: Vec<HybridSearchResult>,
    pub metadata: HybridSearchMetadata,
}
