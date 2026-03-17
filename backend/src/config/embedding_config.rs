use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct EmbeddingConfig {
    #[serde(default = "default_embedding_base_url")]
    pub base_url: String,
    #[serde(default = "default_embedding_model")]
    pub model: String,
    #[serde(default = "default_embedding_dimension")]
    pub dimension: usize,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_embedding_base_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_embedding_model() -> String {
    "nomic-embed-text".to_string()
}

fn default_embedding_dimension() -> usize {
    768
}

fn default_timeout() -> u64 {
    30
}
