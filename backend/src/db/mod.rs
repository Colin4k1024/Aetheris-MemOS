//! Database layer: PostgreSQL for Docker/production.

use sqlx::migrate::Migrator;
use sqlx::PgPool;
use std::path::Path;
use std::sync::OnceLock;
use tracing::{error, info};

use crate::config::DbConfig;

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

pub mod decision_trace;
pub mod kg;
pub mod ltm;
pub mod memory;
pub mod mm;
pub mod neo4j;
pub mod performance;
pub mod stm;
pub mod weights;
pub use kg::KGRepository;
pub use neo4j::{init_neo4j, init_neo4j_indexes};
pub use stm::{SessionMessage, SessionListResponse};

pub static SQLX_POOL: OnceLock<PgPool> = OnceLock::new();

pub async fn init(config: &DbConfig) -> Result<(), DbInitError> {
    info!("Connecting to database: {} (redacted)", config.url.split('@').last().unwrap_or(""));

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
