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

pub fn init() {
    let raw_config = Figment::new()
        .merge(Toml::file(
            Env::var("APP_CONFIG").as_deref().unwrap_or("config.toml"),
        ))
        .merge(Env::prefixed("APP_").global());

    let mut config = match raw_config.extract::<ServerConfig>() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("It looks like your config is invalid. The following error occurred: {e}");
            std::process::exit(1);
        }
    };
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if !url.is_empty() {
            config.db.url = url;
        }
    }
    if config.db.url.is_empty() {
        config.db.url = std::env::var("DATABASE_URL").unwrap_or_default();
    }
    if config.db.url.is_empty() {
        eprintln!("DATABASE_URL is not set and db.url is empty in config");
        std::process::exit(1);
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
