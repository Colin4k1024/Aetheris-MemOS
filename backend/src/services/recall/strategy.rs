use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecallStrategy {
    Keyword,
    Embedding,
    Hybrid,
}

impl Default for RecallStrategy {
    fn default() -> Self {
        Self::Hybrid
    }
}

impl RecallStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Embedding => "embedding",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecallSource {
    L1Atom,
    L2Scene,
    L3Persona,
}
