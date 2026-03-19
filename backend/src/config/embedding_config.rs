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
    /// When `true`, the embedding service will query `hardware_detector` at
    /// startup and override `model` / `dimension` with the best recommendation
    /// for the detected hardware. Set to `false` to lock to the configured values.
    #[serde(default = "default_auto_detect")]
    pub auto_detect: bool,
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

fn default_auto_detect() -> bool {
    false
}
