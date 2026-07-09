use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemorySkill {
    MemorySearch,
    MemoryStore,
    MemoryFusion,
    MemoryStatus,
    KnowledgeGraph,
}

impl MemorySkill {
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "memory_search" => Some(Self::MemorySearch),
            "memory_store" => Some(Self::MemoryStore),
            "memory_fusion" => Some(Self::MemoryFusion),
            "memory_status" => Some(Self::MemoryStatus),
            "knowledge_graph" => Some(Self::KnowledgeGraph),
            _ => None,
        }
    }

    pub fn id(&self) -> &'static str {
        match self {
            Self::MemorySearch => "memory_search",
            Self::MemoryStore => "memory_store",
            Self::MemoryFusion => "memory_fusion",
            Self::MemoryStatus => "memory_status",
            Self::KnowledgeGraph => "knowledge_graph",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchRequest {
    pub query: String,
    pub layer: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStoreRequest {
    pub content: String,
    pub layer: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFusionRequest {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphRequest {
    pub entity: Option<String>,
    pub query: Option<String>,
    pub limit: Option<usize>,
}
