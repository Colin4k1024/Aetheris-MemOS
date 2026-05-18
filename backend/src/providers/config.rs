//! Provider Configuration

use crate::kernel::provider::ProviderType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default = "default_active")]
    pub active: ProviderType,
    pub mem0: Option<ExternalProviderConfig>,
    pub zep: Option<ExternalProviderConfig>,
    pub letta: Option<ExternalProviderConfig>,
}

fn default_active() -> ProviderType {
    ProviderType::Builtin
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            active: ProviderType::Builtin,
            mem0: None,
            zep: None,
            letta: None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExternalProviderConfig {
    pub api_url: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

impl std::fmt::Debug for ExternalProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalProviderConfig")
            .field("api_url", &self.api_url)
            .field("api_key_env", &self.api_key_env.as_ref().map(|_| "[REDACTED]"))
            .field("timeout_ms", &self.timeout_ms)
            .field("max_retries", &self.max_retries)
            .finish()
    }
}

fn default_timeout() -> u64 {
    5000
}

fn default_max_retries() -> u32 {
    3
}

impl Default for ExternalProviderConfig {
    fn default() -> Self {
        Self {
            api_url: "http://localhost:8080".to_string(),
            api_key_env: None,
            timeout_ms: 5000,
            max_retries: 3,
        }
    }
}

impl ExternalProviderConfig {
    pub fn resolve_api_key(&self) -> Option<String> {
        self.api_key_env
            .as_ref()
            .and_then(|env_var| std::env::var(env_var).ok())
    }
}
