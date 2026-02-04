use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct LLMConfig {
    #[serde(default = "default_llm_base_url")]
    pub base_url: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_llm_base_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_llm_model() -> String {
    "llama2".to_string()
}

fn default_timeout() -> u64 {
    30
}

