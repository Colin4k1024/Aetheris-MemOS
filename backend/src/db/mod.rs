//! Database layer: PostgreSQL and SQLite support.

use sqlx::migrate::Migrator;
use sqlx::PgPool;
use std::path::Path;
use std::sync::OnceLock;
use tracing::{error, info};

use crate::config::{DbConfig, StorageConfig};

/// Database initialization error.
#[derive(Debug)]
pub struct DbInitError(pub String);

impl std::fmt::Display for DbInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for DbInitError {}

pub mod adapters;
pub mod agent;

pub mod decision_trace;
pub mod kg;
#[allow(dead_code)]
pub mod ltm;
pub mod memory;
#[allow(dead_code)]
pub mod mm;
#[allow(dead_code)]
pub mod neo4j;
#[allow(dead_code)]
pub mod performance;
pub mod stm;
#[allow(dead_code)]
pub mod weights;
pub use kg::KGRepository;
pub use neo4j::{init_neo4j, init_neo4j_indexes};
pub use stm::{SessionListResponse, SessionMessage};

pub static SQLX_POOL: OnceLock<PgPool> = OnceLock::new();
pub static SQLITE_POOL: OnceLock<sqlx::SqlitePool> = OnceLock::new();

pub async fn init(config: &DbConfig) -> Result<(), DbInitError> {
    info!(
        "Connecting to database: {} (redacted)",
        config.url.split('@').last().unwrap_or("")
    );

    let pool_options = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.pool_size)
        .min_connections(config.min_idle.unwrap_or(2))
        .acquire_timeout(std::time::Duration::from_secs(config.connection_timeout))
        .idle_timeout(Some(std::time::Duration::from_secs(600)))
        .max_lifetime(Some(std::time::Duration::from_secs(1800)));

    let sqlx_pool = pool_options.connect(&config.url).await.map_err(|e| {
        error!("Database connection failed: {}", e);
        DbInitError(format!("Database connection failed: {}", e))
    })?;
    info!("Database connected, pool initialized");

    sqlx_pool.acquire().await.map_err(|e| {
        error!("Database pool health check failed: {}", e);
        DbInitError(format!("Database pool health check failed: {}", e))
    })?;
    info!("Database pool health check passed");

    info!("Running migrations...");
    let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    info!("Migrations path: {:?}", migrations_path);

    let migrator = Migrator::new(migrations_path).await.map_err(|e| {
        error!("Failed to create migrator: {}", e);
        DbInitError(format!("Failed to create migrator: {}", e))
    })?;

    migrator.run(&sqlx_pool).await.map_err(|e| {
        error!("Migrations failed: {}", e);
        DbInitError(format!("Failed to run migrations: {}", e))
    })?;
    info!("Migrations completed");

    crate::db::SQLX_POOL
        .set(sqlx_pool)
        .map_err(|_| DbInitError("sqlx pool already set".to_string()))?;
    info!("Database initialization complete");
    Ok(())
}

pub fn pool() -> &'static PgPool {
    SQLX_POOL.get().expect("sqlx pool should be set")
}

pub fn sqlite_pool() -> &'static sqlx::SqlitePool {
    SQLITE_POOL.get().expect("sqlite pool should be set")
}

/// Initialize SQLite database
pub async fn init_sqlite(config: &StorageConfig) -> Result<(), DbInitError> {
    info!("Initializing SQLite database: {}", config.url);

    // Parse URL to handle special SQLite cases
    let sqlite_url = if config.url.starts_with("sqlite://") {
        config.url.replace("sqlite://", "")
    } else {
        config.url.clone()
    };

    // Create directory if it doesn't exist and URL is a file path
    if !config.in_memory {
        if let Some(parent) = Path::new(&sqlite_url).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    DbInitError(format!("Failed to create database directory: {}", e))
                })?;
            }
        }
    }

    let pool_options = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(30));

    let sqlite_pool = pool_options.connect(&config.url).await.map_err(|e| {
        error!("SQLite connection failed: {}", e);
        DbInitError(format!("SQLite connection failed: {}", e))
    })?;

    info!("SQLite connected, running migrations...");

    let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    info!("SQLite migrations path: {:?}", migrations_path);

    // Note: SQLx migrations need to be compatible with SQLite
    // Some PostgreSQL-specific migrations may not work
    let migrator = Migrator::new(migrations_path).await.map_err(|e| {
        error!("Failed to create SQLite migrator: {}", e);
        DbInitError(format!("Failed to create SQLite migrator: {}", e))
    })?;

    // Run migrations on SQLite pool
    migrator.run(&sqlite_pool).await.map_err(|e| {
        error!("SQLite migrations failed: {}", e);
        DbInitError(format!("Failed to run SQLite migrations: {}", e))
    })?;

    info!("SQLite migrations completed");

    crate::db::SQLITE_POOL
        .set(sqlite_pool)
        .map_err(|_| DbInitError("sqlite pool already set".to_string()))?;

    info!("SQLite database initialization complete");
    Ok(())
}

/// Initialize database based on storage config
pub async fn init_storage(config: &StorageConfig) -> Result<(), DbInitError> {
    match config.backend {
        crate::config::StorageBackend::Postgres => {
            // For PostgreSQL, use the existing init with a DbConfig
            let db_config = DbConfig {
                url: config.url.clone(),
                pool_size: 10,
                min_idle: Some(2),
                tcp_timeout: 10000,
                connection_timeout: 30000,
                statement_timeout: 30000,
                helper_threads: 10,
                enforce_tls: false,
            };
            init(&db_config).await
        }
        crate::config::StorageBackend::Sqlite => init_sqlite(config).await,
    }
}
