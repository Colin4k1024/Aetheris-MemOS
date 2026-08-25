//! Provider Configuration

use crate::kernel::provider::ProviderType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default = "default_active")]
    pub active: ProviderType,
    pub mem0: Option<ExternalProviderConfig>,
    pub zep: Option<ExternalProviderConfig>,
    /// Reserved — Letta provider is not implemented (#87). Selecting
    /// `active: "letta"` is rejected by `create_provider` at runtime.
    /// This field is kept for future use; it is ignored in production.
    #[serde(default)]
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

impl ProviderConfig {
    /// Validate the provider config at startup — fail-fast if an unimplemented
    /// provider is selected (#87).
    pub fn validate(&self) -> Result<(), String> {
        if self.active == ProviderType::Letta {
            return Err(
                "provider.active = 'letta' is reserved but not implemented; \
                 select 'builtin', 'mem0', or 'zep'"
                    .to_string(),
            );
        }
        Ok(())
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
            .field(
                "api_key_env",
                &self.api_key_env.as_ref().map(|_| "[REDACTED]"),
            )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_validates() {
        let cfg = ProviderConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn builtin_config_validates() {
        let cfg = ProviderConfig {
            active: ProviderType::Builtin,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn letta_config_is_rejected_at_validation() {
        let cfg = ProviderConfig {
            active: ProviderType::Letta,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("letta"), "expected 'letta' in error: {err}");
        assert!(err.contains("not implemented"), "expected 'not implemented' in error: {err}");
    }

    #[test]
    fn mem0_config_validates() {
        let cfg = ProviderConfig {
            active: ProviderType::Mem0,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn zep_config_validates() {
        let cfg = ProviderConfig {
            active: ProviderType::Zep,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }
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
