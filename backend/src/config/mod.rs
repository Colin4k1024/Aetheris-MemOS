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
pub mod storage_utils {
    pub use super::storage::resolve_data_directory;
}
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
mod distillation_config;
pub use distillation_config::{DistillationConfig, RecallConfig, SkillsConfig};

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
            tracing::info!("[config] Using config file from APP_CONFIG: {}", path);
            return Some(path);
        }
    }

    // 2. Standard candidates in current directory
    for candidate in &["config.toml", "local.toml"] {
        if std::path::Path::new(candidate).exists() {
            tracing::info!("[config] Found config file: {}", candidate);
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
            tracing::info!("[config] Found user config file: {}", path);
            return Some(path);
        }
    }

    None
}

pub fn init() {
    let config_file = discover_config_file().unwrap_or_else(|| {
        std::env::var("APP_CONFIG").unwrap_or_else(|_| "config.toml".to_string())
    });

    let raw_config = Figment::new()
        .merge(Toml::file(&config_file))
        .merge(Env::prefixed("APP_").global());

    let mut config = match raw_config.extract::<ServerConfig>() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Config error (file: {}): {}", config_file, e);
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
        tracing::warn!(
            "[config] DATABASE_URL not set. Falling back to local SQLite: {}",
            storage.url
        );
        config.db.url = storage.url;
        config.db.backend = crate::config::DatabaseBackend::Sqlite;
    }

    // APP_RECONCILIATION_INTERVAL_SECONDS — figment would map this to
    // `reconciliation.interval.seconds` instead of `reconciliation.interval_seconds`,
    // so spell it out explicitly.
    if let Ok(val) = std::env::var("APP_RECONCILIATION_INTERVAL_SECONDS") {
        if let Ok(secs) = val.parse::<u64>() {
            config.reconciliation.interval_seconds = secs;
        }
    }

    // APP_RECONCILIATION_MODE — explicit to avoid figment nested-key ambiguity.
    if let Ok(mode) = std::env::var("APP_RECONCILIATION_MODE") {
        if !mode.is_empty() {
            config.reconciliation.mode = mode;
        }
    }

    if crate::config::CONFIG.set(config).is_err() {
        tracing::debug!("[config] Configuration already initialized; retaining existing value");
    }
}

pub fn get() -> &'static ServerConfig {
    CONFIG.get().expect("config should be set")
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct MemoryEvolutionConfig {
    #[serde(default = "default_decay_lambda")]
    pub decay_lambda: f64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct MetricsConfig {
    #[serde(default = "default_metrics_enabled")]
    pub enabled: bool,
    #[serde(default = "default_export_interval")]
    pub export_interval_seconds: u64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            export_interval_seconds: 15,
        }
    }
}

fn default_metrics_enabled() -> bool {
    true
}

fn default_export_interval() -> u64 {
    15
}

fn default_decay_lambda() -> f64 {
    0.01
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

/// Vector reconciliation scanner configuration.
///
/// The scanner periodically compares PostgreSQL `knowledge_entries` against
/// Qdrant points to detect four classes of drift: missing (DB entry with no
/// Qdrant point), orphan (Qdrant point with no DB entry), tenant mismatch, and
/// content-hash mismatch.  In `dry_run` mode the scan is **read-only** — it
/// records drift rows, logs counts and updates Prometheus gauges but never
/// enqueues outbox events.  In `repair` mode it additionally enqueues outbox
/// events to fix the drift.
///
/// The scanner is enabled by default because leaving it disabled preserves the
/// existing bug where vector drift is never detected.  `repair` mode enqueues
/// bulk Qdrant writes and must be explicitly opted into — it should never be
/// the default in committed configuration.
///
/// `mode` must be one of the values accepted by
/// [`crate::db::vector_reconciliation::ReconciliationMode::parse`] — the
/// scanner validates it once at startup and falls back to `dry_run` with a
/// `WARN` if it does not parse, so a typo degrades to read-only rather than
/// failing every scan.
#[derive(Deserialize, Clone, Debug)]
pub struct ReconciliationConfig {
    /// Enable the periodic reconciliation scanner. Defaults to true.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Seconds between reconciliation scans. Default 3600 (1 hour).
    /// Values below the scanner's floor are raised, with a `WARN`.
    #[serde(default = "default_reconciliation_interval")]
    pub interval_seconds: u64,
    /// Scan mode: `"dry_run"` (read-only) or `"repair"` (enqueues outbox writes).
    #[serde(default = "default_reconciliation_mode")]
    pub mode: String,
}

impl Default for ReconciliationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_seconds: default_reconciliation_interval(),
            mode: default_reconciliation_mode(),
        }
    }
}

fn default_reconciliation_interval() -> u64 {
    3600
}

fn default_reconciliation_mode() -> String {
    "dry_run".to_string()
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
    #[serde(default)]
    pub memory_evolution: MemoryEvolutionConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub otel: OtelConfig,
    #[serde(default)]
    pub distillation: DistillationConfig,
    #[serde(default)]
    pub recall: RecallConfig,
    #[serde(default)]
    pub skills: SkillsConfig,
    #[serde(default)]
    pub reconciliation: ReconciliationConfig,
    /// Trusted reverse-proxy IPs whose `X-Forwarded-For` header is honoured by
    /// the rate limiter. Empty (the default) → XFF is never trusted and the
    /// direct peer IP is always used. Restored from 4eeecaa (lost in the merge).
    #[serde(default)]
    pub trusted_proxies: Vec<std::net::IpAddr>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct JwtConfig {
    pub secret: String,
    pub expiry: i64,
    #[serde(default)]
    pub disabled: bool,
}

/// Known placeholder JWT secrets shipped in examples/templates. Booting with
/// one while auth is enabled is a deterministic misconfiguration.
const INSECURE_JWT_SECRETS: &[&str] = &[
    "REPLACE_WITH_STRONG_SECRET_OR_USE_APP_JWT_SECRET",
    "change-me-in-production-32chars",
    "change-me",
    "changeme",
    "secret",
    "your-secret-key",
];

/// Minimum acceptable JWT secret length (bytes) when auth is enabled.
const MIN_JWT_SECRET_LEN: usize = 32;

/// Validate that the JWT signing secret is safe for a production startup.
///
/// Returns `Err` (so the caller can fail-fast) when authentication is enabled
/// but the secret is a known placeholder or shorter than `MIN_JWT_SECRET_LEN`.
/// When `jwt.disabled` is set (explicit local/dev mode) validation is skipped.
pub fn validate_jwt_security(config: &ServerConfig) -> Result<(), String> {
    check_jwt_secret(config.jwt.disabled, &config.jwt.secret)
}

fn check_jwt_secret(disabled: bool, secret: &str) -> Result<(), String> {
    if disabled {
        return Ok(());
    }
    let secret = secret.trim();
    if INSECURE_JWT_SECRETS.contains(&secret) {
        return Err(
            "Insecure JWT secret: the configured value is a known placeholder. Set a strong \
             random secret via the APP_JWT_SECRET environment variable (e.g. `openssl rand -hex 32`), \
             or set jwt.disabled=true for local loopback dev only."
                .to_string(),
        );
    }
    if secret.len() < MIN_JWT_SECRET_LEN {
        return Err(format!(
            "Insecure JWT secret: must be at least {MIN_JWT_SECRET_LEN} characters (got {}). Set a \
             strong random secret via the APP_JWT_SECRET environment variable (e.g. \
             `openssl rand -hex 32`), or set jwt.disabled=true for local loopback dev only.",
            secret.len()
        ));
    }
    Ok(())
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

/// OpenTelemetry / distributed tracing configuration.
#[derive(Deserialize, Clone, Debug)]
pub struct OtelConfig {
    /// Enable OTLP trace export. When false, only fmt logging is used.
    #[serde(default)]
    pub enabled: bool,
    /// OTLP gRPC endpoint, e.g. "http://otel-collector:4317"
    #[serde(default = "default_otel_endpoint")]
    pub endpoint: String,
    /// Service name reported in traces.
    #[serde(default = "default_otel_service_name")]
    pub service_name: String,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: default_otel_endpoint(),
            service_name: default_otel_service_name(),
        }
    }
}

fn default_otel_endpoint() -> String {
    "http://localhost:4317".into()
}

fn default_otel_service_name() -> String {
    "aetheris-memos-backend".into()
}
