use salvo::oapi::ToSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    Stm,
    Ltm,
    Kg,
    Mm,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MemoryWeights {
    pub stm: f64,
    pub ltm: f64,
    pub kg: f64,
    pub mm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MemoryConfig {
    #[serde(rename = "primary_memory")]
    pub primary_memory: MemoryType,
    #[serde(rename = "secondary_memory")]
    pub secondary_memory: Vec<MemoryType>,
    #[serde(rename = "memory_weights")]
    pub memory_weights: MemoryWeights,
    #[serde(rename = "reasoning_depth")]
    pub reasoning_depth: String,
    #[serde(rename = "enable_multimodal")]
    pub enable_multimodal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MemoryStrategy {
    #[serde(rename = "primary_memory")]
    pub primary_memory: String,
    #[serde(rename = "secondary_memory")]
    pub secondary_memory: Vec<String>,
    #[serde(rename = "enable_multimodal")]
    pub enable_multimodal: bool,
    #[serde(rename = "reasoning_depth")]
    pub reasoning_depth: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdjustmentReasons {
    pub stm: String,
    pub ltm: String,
    pub kg: String,
    pub mm: String,
}

