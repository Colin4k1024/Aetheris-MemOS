use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct RerankConfig {
    #[serde(default = "default_rerank_base_url")]
    pub base_url: String,
    #[serde(default = "default_rerank_model")]
    pub model: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_candidate_multiplier")]
    pub candidate_multiplier: usize,
    #[serde(default = "default_min_score_threshold")]
    pub min_score_threshold: f32,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_rerank_base_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_rerank_model() -> String {
    "bge-reranker-base".to_string()
}

fn default_candidate_multiplier() -> usize {
    2
}

fn default_min_score_threshold() -> f32 {
    0.3
}

fn default_timeout() -> u64 {
    30
}

fn default_true() -> bool {
    true
}
