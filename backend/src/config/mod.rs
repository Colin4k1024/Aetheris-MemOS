use std::sync::OnceLock;

use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::Deserialize;

mod log_config;
pub use log_config::LogConfig;
mod db_config;
pub use db_config::{DatabaseBackend, DbConfig};
mod storage;
pub use storage::{StorageBackend, StorageConfig};
mod llm_config;
pub use llm_config::LLMConfig;
mod embedding_config;
pub use embedding_config::EmbeddingConfig;
mod qdrant_config;
pub use qdrant_config::QdrantConfig;
mod rerank_config;
pub use rerank_config::RerankConfig;
mod neo4j_config;
pub use neo4j_config::Neo4jConfig;

pub static CONFIG: OnceLock<ServerConfig> = OnceLock::new();

/// Discover a config file by probing multiple candidate paths.
///
/// Resolution order:
/// 1. `APP_CONFIG` env var (explicit override)
/// 2. `config.toml` in the current directory
/// 3. `local.toml` in the current directory
/// 4. `~/.adaptive-memory/config.toml` (user-level config)
fn discover_config_file() -> Option<String> {
    // 1. Explicit env var override
    if let Ok(path) = std::env::var("APP_CONFIG") {
        if !path.is_empty() && std::path::Path::new(&path).exists() {
            eprintln!("[config] Using config file from APP_CONFIG: {}", path);
            return Some(path);
        }
    }

    // 2. Standard candidates in current directory
    for candidate in &["config.toml", "local.toml"] {
        if std::path::Path::new(candidate).exists() {
            eprintln!("[config] Found config file: {}", candidate);
            return Some(candidate.to_string());
        }
    }

    // 3. User-level config
    let home_candidates: Vec<_> = [
        std::env::var("HOME").ok(),
        std::env::var("USERPROFILE").ok(),
    ]
    .into_iter()
    .flatten()
    .map(|h| format!("{}/.adaptive-memory/config.toml", h))
    .collect();

    for path in home_candidates {
        if std::path::Path::new(&path).exists() {
            eprintln!("[config] Found user config file: {}", path);
            return Some(path);
        }
    }

    None
}

pub fn init() {
    let config_file = discover_config_file()
        .unwrap_or_else(|| std::env::var("APP_CONFIG").unwrap_or_else(|_| "config.toml".to_string()));

    let raw_config = Figment::new()
        .merge(Toml::file(&config_file))
        .merge(Env::prefixed("APP_").global());

    let mut config = match raw_config.extract::<ServerConfig>() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Config error (file: {}): {}", config_file, e);
            std::process::exit(1);
        }
    };

    // DATABASE_URL env var takes precedence over config file
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if !url.is_empty() {
            config.db.url = url;
        }
    }

    // Graceful fallback: if no database URL configured, use a local SQLite database
    if config.db.url.is_empty() {
        let storage = crate::config::StorageConfig::resolve_local_sqlite("adaptive_memory.db");
        eprintln!(
            "[config] DATABASE_URL not set. Falling back to local SQLite: {}",
            storage.url
        );
        config.db.url = storage.url;
        config.db.backend = crate::config::DatabaseBackend::Sqlite;
    }

    crate::config::CONFIG
        .set(config)
        .expect("config should be set");
}

pub fn get() -> &'static ServerConfig {
    CONFIG.get().expect("config should be set")
}

#[derive(Deserialize, Clone, Debug)]
pub struct MemoryTransferConfig {
    #[serde(default = "default_check_interval")]
    pub check_interval: u64,
    #[serde(default = "default_message_count_threshold")]
    pub message_count_threshold: i32,
    #[serde(default = "default_session_time_threshold")]
    pub session_time_threshold: i32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ServerConfig {
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    pub db: DbConfig,
    pub log: LogConfig,
    pub jwt: JwtConfig,
    pub tls: Option<TlsConfig>,
    pub llm: LLMConfig,
    pub embedding: EmbeddingConfig,
    pub qdrant: QdrantConfig,
    pub rerank: RerankConfig,
    pub neo4j: Neo4jConfig,
    #[serde(default = "default_memory_transfer_config")]
    pub memory_transfer: MemoryTransferConfig,
}

#[derive(Deserialize, Clone, Debug)]
pub struct JwtConfig {
    pub secret: String,
    pub expiry: i64,
}
#[derive(Deserialize, Clone, Debug)]
pub struct TlsConfig {
    pub cert: String,
    pub key: String,
}

#[allow(dead_code)]
pub fn default_false() -> bool {
    false
}
#[allow(dead_code)]
pub fn default_true() -> bool {
    true
}

fn default_listen_addr() -> String {
    "127.0.0.1:8008".into()
}

fn default_check_interval() -> u64 {
    300 // 默认5分钟检查一次
}

fn default_message_count_threshold() -> i32 {
    100 // 默认消息数量阈值为100
}

fn default_session_time_threshold() -> i32 {
    24 // 默认会话时间阈值为24小时
}

fn default_memory_transfer_config() -> MemoryTransferConfig {
    MemoryTransferConfig {
        check_interval: default_check_interval(),
        message_count_threshold: default_message_count_threshold(),
        session_time_threshold: default_session_time_threshold(),
    }
}
