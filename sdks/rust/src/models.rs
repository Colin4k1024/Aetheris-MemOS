//! Data Models for Adaptive Memory System

use serde::{Deserialize, Serialize};

/// Memory layer type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerType {
    /// Short-term memory
    Stm,
    /// Long-term memory
    Ltm,
    /// Knowledge graph
    Kg,
    /// Multimodal memory
    Mm,
}

/// Memory metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub tags: Vec<String>,
    pub importance: f64,
    pub access_count: u32,
}

/// Memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub layer: LayerType,
    pub content: String,
    pub metadata: MemoryMetadata,
}

/// Memory search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entry_id: String,
    pub score: f32,
    pub content: String,
}

/// Session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub session_type: String,
    pub status: String,
}

/// Memory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub primary_memory: LayerType,
    pub secondary_memory: Vec<LayerType>,
    pub stm_weight: f64,
    pub ltm_weight: f64,
    pub kg_weight: f64,
    pub mm_weight: f64,
}

/// Task characteristic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCharacteristic {
    pub complexity: String,
    pub modality: String,
    pub reasoning_depth: String,
    pub estimated_tokens: u32,
}

/// Store STM request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStmRequest {
    pub user_id: String,
    pub agent_id: String,
    pub session_type: String,
    pub role: String,
    pub content: String,
}

/// Store STM response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStmResponse {
    pub session_id: String,
    pub message_id: String,
}

/// Store LTM request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreLtmRequest {
    pub source_id: String,
    pub source_type: String,
    pub content: String,
    pub title: Option<String>,
}

/// Store LTM response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreLtmResponse {
    pub entry_id: String,
}
