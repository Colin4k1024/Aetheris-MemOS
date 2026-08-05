use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct LLMConfig {
    #[serde(default = "default_llm_base_url")]
    pub base_url: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    /// API 格式："ollama"（默认）或 "openai"（OpenAI 兼容，含 DashScope）
    #[serde(default = "default_api_type")]
    pub api_type: String,
    /// API Key，用于 OpenAI 兼容接口认证
    #[serde(default)]
    pub api_key: Option<String>,
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

fn default_api_type() -> String {
    "ollama".to_string()
}
