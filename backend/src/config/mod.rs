use std::net::IpAddr;
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

    // APP_DB_ADMIN_URL enables the RLS-bypassing maintenance pool (see
    // `db::ADMIN_POOL`). Needs an explicit handler for the same reason as the
    // secrets below: figment's prefixed env does not map `APP_DB_ADMIN_URL` onto
    // the nested `db.admin_url` key, so without this the documented .env.example
    // switch would be silently ignored and the feature unreachable.
    if let Ok(url) = std::env::var("APP_DB_ADMIN_URL") {
        if !url.is_empty() {
            config.db.admin_url = Some(url);
        }
    }

    // APP_DB_AUTO_MIGRATE — same reason: figment would map this to
    // `db.auto.migrate` (splitting on `_`) instead of `db.auto_migrate`.
    if let Ok(val) = std::env::var("APP_DB_AUTO_MIGRATE") {
        if let Ok(b) = val.parse::<bool>() {
            config.db.auto_migrate = b;
        }
    }

    // APP_DB_ALLOW_SQLITE_FALLBACK — same reason (figment would look for
    // `db.allow.sqlite.fallback`). Gates the isolation-dropping SQLite fallback
    // below; without this handler the documented env switch would be ignored and
    // an operator's explicit local-dev opt-in would fail to take effect.
    if let Ok(val) = std::env::var("APP_DB_ALLOW_SQLITE_FALLBACK") {
        if let Ok(b) = val.parse::<bool>() {
            config.db.allow_sqlite_fallback = b;
        }
    }

    // APP_RECONCILIATION_INTERVAL_SECONDS — same reason: figment would map
    // this to `reconciliation.interval.seconds` instead of `reconciliation.interval_seconds`.
    if let Ok(val) = std::env::var("APP_RECONCILIATION_INTERVAL_SECONDS") {
        if let Ok(secs) = val.parse::<u64>() {
            config.reconciliation.interval_seconds = secs;
        }
    }

    // APP_RECONCILIATION_MODE — same reason: figment would map this to
    // `reconciliation.mode` (which is correct, but only if you use `_` in the
    // env name — spelling it out explicitly avoids ambiguity).
    if let Ok(mode) = std::env::var("APP_RECONCILIATION_MODE") {
        if !mode.is_empty() {
            config.reconciliation.mode = mode;
        }
    }

    // Secret env vars take precedence over the config file. figment's prefixed env
    // (`Env::prefixed("APP_")`) does not map single-underscore names into nested keys
    // like `jwt.secret` / `neo4j.password`, so read them explicitly (mirrors
    // DATABASE_URL above). This is what .env.example / docker-compose advertise.
    if let Ok(secret) = std::env::var("APP_JWT_SECRET") {
        if !secret.is_empty() {
            config.jwt.secret = secret;
        }
    }
    if let Ok(pw) = std::env::var("APP_NEO4J_PASSWORD") {
        if !pw.is_empty() {
            config.neo4j.password = pw;
        }
    }

    // API key overrides for LLM / Embedding / Rerank (OpenAI-compatible backends)
    if let Ok(key) = std::env::var("APP_LLM_API_KEY") {
        if !key.is_empty() {
            config.llm.api_key = Some(key);
        }
    }
    if let Ok(key) = std::env::var("APP_EMBEDDING_API_KEY") {
        if !key.is_empty() {
            config.embedding.api_key = Some(key);
        }
    }
    if let Ok(key) = std::env::var("APP_RERANK_API_KEY") {
        if !key.is_empty() {
            config.rerank.api_key = Some(key);
        }
    }

    // SQLite fallback for an empty `db.url`.
    //
    // This must be opted into. SQLite has no Row Level Security, so falling back
    // to it silently drops every database-layer tenant isolation guarantee — the
    // exact protection P0-1 was fixed to establish. A production deployment
    // whose DATABASE_URL goes missing (empty env var, bad secret injection,
    // stripped config) has to fail to boot rather than come up with weaker
    // isolation than the operator thinks it has.
    //
    // NOTE: `eprintln!`, not `tracing::`. `init()` runs before
    // `otel::init_tracing` in `main()`, so every `tracing::` call in this
    // function is emitted with no subscriber installed and is silently dropped.
    // The previous `tracing::warn!` here was therefore invisible in practice —
    // the degradation was not merely under-alarmed, it was completely silent.
    if config.db.url.is_empty() {
        if !config.db.allow_sqlite_fallback {
            eprintln!(
                "[startup] FATAL: no database URL configured.\n\
                 \n\
                 `db.url` is empty and DATABASE_URL is unset, so there is no\n\
                 PostgreSQL to connect to. Refusing to fall back to local SQLite:\n\
                 SQLite has no Row Level Security, so the fallback would silently\n\
                 drop all database-layer tenant isolation.\n\
                 \n\
                 Fix one of:\n\
                   - set DATABASE_URL (production: from a secrets manager), or\n\
                   - set `db.url` in the config file, or\n\
                   - for LOCAL DEVELOPMENT ONLY, opt into the SQLite fallback with\n\
                     `db.allow_sqlite_fallback = true` or\n\
                     APP_DB_ALLOW_SQLITE_FALLBACK=true"
            );
            std::process::exit(1);
        }

        let storage = crate::config::StorageConfig::resolve_local_sqlite("adaptive_memory.db");
        eprintln!(
            "\n\
             ╔══════════════════════════════════════════════════════════════╗\n\
             ║  SQLITE FALLBACK ACTIVE — NO DB-LAYER TENANT ISOLATION       ║\n\
             ║  Row Level Security is a PostgreSQL feature and is absent    ║\n\
             ║  here. Tenant isolation depends entirely on every query      ║\n\
             ║  passing the correct tenant_id. Local development only.      ║\n\
             ╚══════════════════════════════════════════════════════════════╗\n\
             [startup] Falling back to local SQLite: {}",
            storage.url
        );
        config.db.url = storage.url;
        config.db.backend = crate::config::DatabaseBackend::Sqlite;
    }

    // Embedding is a hard dependency: fail-fast on a config that can never
    // produce a valid embedding, before we set CONFIG and advertise readiness.
    // `eprintln!`, not `tracing::`, for the same reason as the SQLite block
    // above — `init()` runs before the tracing subscriber is installed, so a
    // `tracing::` call here would be silently dropped. Reachability is NOT
    // checked here (see `validate_embedding_config`); it belongs in `/readyz`.
    if let Err(e) = validate_embedding_config(&config.embedding) {
        eprintln!(
            "[startup] FATAL: invalid embedding configuration.\n\
             \n\
             {e}\n\
             \n\
             Embedding is a hard dependency — vectors are generated on the LTM\n\
             write and search hot path, so an unusable embedding config makes\n\
             those paths fail on every request. Refusing to boot."
        );
        std::process::exit(1);
    }

    crate::config::CONFIG
        .set(config)
        .expect("config should be set");
}

pub fn get() -> &'static ServerConfig {
    CONFIG.get().expect("config should be set")
}

/// Known placeholder / example JWT secrets shipped in the repo or compose files.
/// Booting with any of these while auth is enabled is a hard security error.
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
/// but the secret is a known placeholder or shorter than [`MIN_JWT_SECRET_LEN`].
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

/// Known placeholder embedding API keys shipped in examples/templates. Booting
/// with one while `api_type == "openai"` is a deterministic misconfiguration —
/// the operator copied an example and never set a real key.
const PLACEHOLDER_EMBEDDING_API_KEYS: &[&str] = &[
    "your-api-key",
    "your_api_key",
    "sk-your-api-key",
    "sk-xxxxxxxx",
    "changeme",
    "replace-me",
    "<your-api-key>",
];

/// Validate the embedding config for a startup fail-fast.
///
/// Embedding is a HARD dependency: vectors are generated on the LTM write and
/// search hot path, so a config that can NEVER produce a valid embedding must
/// stop the boot rather than surface as a per-request failure later.
///
/// Only DETERMINISTIC, config-only failures are checked here — deliberately
/// NOT network reachability. Reachability is runtime state: probing it at
/// startup would turn "embedding backend comes up a few seconds after the app"
/// (common with docker-compose lacking healthcheck ordering) into a boot
/// failure. That is the same trap D-d hit with Neo4j's blocking retry.
/// Reachability belongs in the `/readyz` probe, not here.
pub fn validate_embedding_config(config: &EmbeddingConfig) -> Result<(), String> {
    if config.base_url.trim().is_empty() {
        return Err(
            "embedding.base_url is empty — there is no embedding backend to \
                    call. Set it in the config file or via APP_EMBEDDING_BASE_URL."
                .to_string(),
        );
    }
    if config.model.trim().is_empty() {
        return Err(
            "embedding.model is empty — the backend requires a model name. \
                    Set embedding.model (e.g. `nomic-embed-text`)."
                .to_string(),
        );
    }
    if config.dimension == 0 {
        return Err("embedding.dimension is 0 — every embedding would fail the \
                    dimension check. Set embedding.dimension to the model's output \
                    size (e.g. 768 for nomic-embed-text)."
            .to_string());
    }
    // api_key is only meaningful for OpenAI-compatible backends. A MISSING key is
    // NOT an error: keyless local OpenAI-compatible servers (vLLM, LM Studio) are
    // valid, so we reject only an obvious copied-example placeholder, never
    // absence — flagging absence would be a false fail-fast (the D-d anti-pattern).
    if config.api_type == "openai" {
        if let Some(key) = config.api_key.as_deref() {
            if PLACEHOLDER_EMBEDDING_API_KEYS.contains(&key.trim()) {
                return Err(
                    "embedding.api_key is a known placeholder value. Set a real \
                            key via APP_EMBEDDING_API_KEY, or remove it entirely if your \
                            OpenAI-compatible backend needs no key."
                        .to_string(),
                );
            }
        }
    }
    Ok(())
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
    pub reconciliation: ReconciliationConfig,
    /// Trusted reverse-proxy IPs whose `X-Forwarded-For` header is honoured.
    ///
    /// When empty (the default), the XFF header is **never** trusted — the
    /// rate limiter always uses the direct peer IP from the TCP connection.
    /// When non-empty, only connections from addresses in this list may
    /// supply a client IP via XFF, and the rightmost non-trusted entry is
    /// used (not the leftmost, which is client-supplied and forgeable).
    #[serde(default)]
    pub trusted_proxies: Vec<IpAddr>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct JwtConfig {
    pub secret: String,
    pub expiry: i64,
    #[serde(default)]
    pub disabled: bool,
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

#[cfg(test)]
mod tests {
    use super::{check_jwt_secret, validate_embedding_config, EmbeddingConfig};

    #[test]
    fn rejects_known_placeholder_secrets_when_enabled() {
        let placeholder = "REPLACE_WITH_STRONG_SECRET_OR_USE_APP_JWT_SECRET";
        assert!(check_jwt_secret(false, placeholder).is_err());
        assert!(check_jwt_secret(false, "change-me-in-production-32chars").is_err());
        // whitespace around a placeholder must not sneak past.
        assert!(check_jwt_secret(false, "  secret  ").is_err());
    }

    #[test]
    fn rejects_too_short_secret_when_enabled() {
        assert!(check_jwt_secret(false, "0123456789abcdef").is_err()); // 16 chars
    }

    #[test]
    fn accepts_strong_secret_when_enabled() {
        let strong = "b8f2c1a9e7d4463fa0c5182b6e93d7a41f0c2e5b8a9d6c3f";
        assert!(check_jwt_secret(false, strong).is_ok());
    }

    #[test]
    fn skips_validation_when_disabled() {
        assert!(check_jwt_secret(true, "secret").is_ok());
    }

    // --- SQLite fallback opt-in ------------------------------------------ //

    /// Deserialize a `DbConfig` from raw TOML through figment — the same
    /// provider `init()` uses, so serde defaults are exercised for real rather
    /// than via a hand-built struct.
    fn db_config_from_toml(body: &str) -> super::DbConfig {
        // `Format` must be in scope: `Toml::string` is a trait method, not an
        // inherent one.
        use figment::providers::Format;

        figment::Figment::new()
            .merge(figment::providers::Toml::string(body))
            .extract()
            .expect("DbConfig should deserialize")
    }

    /// The SQLite fallback must be opt-in, because SQLite has no Row Level
    /// Security: falling back silently drops every database-layer tenant
    /// isolation guarantee. A deployment that loses its `DATABASE_URL` has to
    /// fail to boot rather than come up weaker than the operator believes.
    ///
    /// This pins the default. `init()` itself calls `std::process::exit(1)` on
    /// the deny path, which cannot be asserted in-process, so the guard here is
    /// the default value that gates it.
    #[test]
    fn sqlite_fallback_is_denied_by_default() {
        let cfg = db_config_from_toml(r#"url = """#);
        assert!(
            !cfg.allow_sqlite_fallback,
            "db.allow_sqlite_fallback must default to false — the fallback drops \
             all DB-layer tenant isolation and must never be implicit"
        );
    }

    #[test]
    fn sqlite_fallback_can_be_explicitly_enabled() {
        let cfg = db_config_from_toml(
            r#"
            url = ""
            allow_sqlite_fallback = true
            "#,
        );
        assert!(cfg.allow_sqlite_fallback);
    }

    // --- Embedding config fail-fast (D-e2) ------------------------------- //

    fn valid_embedding_cfg() -> EmbeddingConfig {
        EmbeddingConfig {
            base_url: "http://localhost:11434".to_string(),
            model: "nomic-embed-text".to_string(),
            dimension: 768,
            timeout_seconds: 30,
            auto_detect: false,
            api_type: "ollama".to_string(),
            api_key: None,
        }
    }

    #[test]
    fn accepts_valid_ollama_embedding_config() {
        assert!(validate_embedding_config(&valid_embedding_cfg()).is_ok());
    }

    #[test]
    fn rejects_empty_base_url() {
        let mut cfg = valid_embedding_cfg();
        cfg.base_url = "   ".to_string();
        assert!(validate_embedding_config(&cfg).is_err());
    }

    #[test]
    fn rejects_empty_model() {
        let mut cfg = valid_embedding_cfg();
        cfg.model = String::new();
        assert!(validate_embedding_config(&cfg).is_err());
    }

    #[test]
    fn rejects_zero_dimension() {
        let mut cfg = valid_embedding_cfg();
        cfg.dimension = 0;
        assert!(validate_embedding_config(&cfg).is_err());
    }

    #[test]
    fn rejects_placeholder_api_key_for_openai() {
        let mut cfg = valid_embedding_cfg();
        cfg.api_type = "openai".to_string();
        cfg.api_key = Some("  your-api-key  ".to_string()); // whitespace must not sneak past
        assert!(validate_embedding_config(&cfg).is_err());
    }

    #[test]
    fn accepts_missing_api_key_for_keyless_openai_backend() {
        // Keyless local OpenAI-compatible servers (vLLM, LM Studio) are valid;
        // absence must NOT fail (that would be a false fail-fast).
        let mut cfg = valid_embedding_cfg();
        cfg.api_type = "openai".to_string();
        cfg.api_key = None;
        assert!(validate_embedding_config(&cfg).is_ok());
    }

    #[test]
    fn accepts_real_api_key_for_openai() {
        let mut cfg = valid_embedding_cfg();
        cfg.api_type = "openai".to_string();
        cfg.api_key = Some("sk-9f8e7d6c5b4a3210fedcba9876543210".to_string());
        assert!(validate_embedding_config(&cfg).is_ok());
    }
}
