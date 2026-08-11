use serde::{Deserialize, Serialize};

use super::default_false;

/// Database backend type
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseBackend {
    #[default]
    Postgres,
    Sqlite,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct DbConfig {
    /// Database backend type: "postgres" or "sqlite"
    #[serde(default)]
    pub backend: DatabaseBackend,

    /// Settings for the primary database. This is usually writeable, but will be read-only in
    /// some configurations.
    /// An optional follower database. Always read-only.
    #[serde(alias = "database_url")]
    pub url: String,

    /// SQLite specific: local file path (alternative to url for SQLite)
    #[serde(default)]
    pub path: Option<String>,

    #[serde(default = "default_db_pool_size")]
    pub pool_size: u32,
    pub min_idle: Option<u32>,

    /// Number of seconds to wait for unacknowledged TCP packets before treating the connection as
    /// broken. This value will determine how long crates.io stays unavailable in case of full
    /// packet loss between the application and the database: setting it too high will result in an
    /// unnecessarily long outage (before the unhealthy database logic kicks in), while setting it
    /// too low might result in healthy connections being dropped.
    #[serde(default = "default_tcp_timeout")]
    pub tcp_timeout: u64,
    /// Time to wait for a connection to become available from the connection
    /// pool before returning an error.
    /// Time to wait for a connection to become available from the connection
    /// pool before returning an error.
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
    /// Time to wait for a query response before canceling the query and
    /// returning an error.
    #[serde(default = "default_statement_timeout")]
    pub statement_timeout: u64,
    /// Number of threads to use for asynchronous operations such as connection
    /// creation.
    #[serde(default = "default_helper_threads")]
    pub helper_threads: usize,
    /// Whether to run database migrations automatically at startup.
    ///
    /// Running migrations requires DDL privileges (CREATE on schema), so the
    /// application should normally NOT do it. Leave `false` in production and
    /// run migrations as a separate owner-privileged step (e.g. `sqlx migrate
    /// run`). When `false`, the application verifies that the schema is up to
    /// date and fails fast if any migration is missing.
    ///
    /// Set to `true` only for local development where the database connection
    /// already holds DDL privileges.
    #[serde(default = "default_false")]
    pub auto_migrate: bool,

    /// Whether to enforce that all the database connections are encrypted with TLS.
    #[serde(default = "default_false")]
    pub enforce_tls: bool,

    /// Optional owner/BYPASSRLS maintenance connection URL for cross-tenant
    /// operations that must run outside Row Level Security (e.g. Qdrant
    /// tenant-metadata backfill).
    ///
    /// When set, a separate small connection pool (max 2) is created at
    /// startup using this URL.  The pool is NOT used for any regular query;
    /// it is only consumed by the backfill endpoint.
    ///
    /// **Default is `None` (disabled).**  The backfill endpoint returns an
    /// explicit error until an operator deliberately configures this.
    #[serde(default)]
    pub admin_url: Option<String>,

    /// Whether an empty [`Self::url`] may silently fall back to a local SQLite
    /// database.
    ///
    /// **Defaults to `false`, and that default is a security control.** SQLite
    /// has no Row Level Security, so the fallback drops *all* database-layer
    /// tenant isolation — the multi-tenant guarantee then rests entirely on
    /// every application query passing the right `tenant_id`. A production
    /// deployment that loses its `DATABASE_URL` (empty env var, stripped
    /// config, bad secret injection) must **fail to boot**, not quietly come up
    /// with weaker isolation than the operator believes it has.
    ///
    /// Set to `true` (or `APP_DB_ALLOW_SQLITE_FALLBACK=true`) only for local
    /// development. Doing so prints a startup banner, mirroring `jwt.disabled`.
    #[serde(default = "default_false")]
    pub allow_sqlite_fallback: bool,
}

fn default_helper_threads() -> usize {
    10
}
fn default_db_pool_size() -> u32 {
    10
}
fn default_tcp_timeout() -> u64 {
    10000
}
fn default_connection_timeout() -> u64 {
    30000
}
fn default_statement_timeout() -> u64 {
    30000
}
