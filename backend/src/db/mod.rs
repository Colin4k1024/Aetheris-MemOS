//! Database layer: PostgreSQL and SQLite support.

use sqlx::migrate::Migrator;
use sqlx::{PgPool, SqlitePool};
use std::path::Path;
use std::sync::OnceLock;
use tracing::{error, info};

use crate::config::{DatabaseBackend, DbConfig, StorageConfig};

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
pub mod event_store;
pub mod evidence_graph;
pub mod kg;
#[allow(dead_code)]
pub mod ltm;
pub mod memory;
pub mod memory_feedback;
#[allow(dead_code)]
pub mod mm;
#[allow(dead_code)]
pub mod neo4j;
#[allow(dead_code)]
pub mod performance;
pub mod stm;
#[allow(dead_code)]
pub mod weights;
pub mod workflow_lifecycle;
pub use kg::KGRepository;
pub use neo4j::{init_neo4j, init_neo4j_indexes};
pub use stm::{SessionListResponse, SessionMessage};

/// Database pool - either PostgreSQL or SQLite
pub enum DatabasePool {
    Postgres(PgPool),
    Sqlite(SqlitePool),
}

pub static DATABASE_POOL: OnceLock<DatabasePool> = OnceLock::new();

/// Initialize the database based on configuration
pub async fn init(config: &DbConfig) -> Result<(), DbInitError> {
    match config.backend {
        DatabaseBackend::Postgres => init_postgres(config).await,
        DatabaseBackend::Sqlite => init_sqlite(config).await,
    }
}

async fn init_postgres(config: &DbConfig) -> Result<(), DbInitError> {
    info!(
        "Connecting to PostgreSQL: {} (redacted)",
        config.url.split('@').last().unwrap_or("")
    );

    let pool_options = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.pool_size)
        .min_connections(config.min_idle.unwrap_or(2))
        .acquire_timeout(std::time::Duration::from_secs(config.connection_timeout))
        .idle_timeout(Some(std::time::Duration::from_secs(600)))
        .max_lifetime(Some(std::time::Duration::from_secs(1800)));

    let sqlx_pool = pool_options.connect(&config.url).await.map_err(|e| {
        error!("PostgreSQL connection failed: {}", e);
        DbInitError(format!("PostgreSQL connection failed: {}", e))
    })?;
    info!("PostgreSQL connected, pool initialized");

    sqlx_pool.acquire().await.map_err(|e| {
        error!("PostgreSQL pool health check failed: {}", e);
        DbInitError(format!("PostgreSQL pool health check failed: {}", e))
    })?;
    info!("PostgreSQL pool health check passed");

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

    DATABASE_POOL
        .set(DatabasePool::Postgres(sqlx_pool))
        .map_err(|_| DbInitError("database pool already set".to_string()))?;
    info!("PostgreSQL initialization complete");
    Ok(())
}

async fn init_sqlite(config: &DbConfig) -> Result<(), DbInitError> {
    // Determine SQLite database path
    let db_path = if let Some(ref path) = config.path {
        path.clone()
    } else {
        // Extract path from URL or use default
        if config.url.starts_with("sqlite:") {
            config.url.trim_start_matches("sqlite:")
        } else {
            &config.url
        }
        .to_string()
    };

    info!("Connecting to SQLite: {}", db_path);

    // Create parent directory if it doesn't exist
    if let Some(parent) = Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            error!("Failed to create SQLite directory: {}", e);
            DbInitError(format!("Failed to create SQLite directory: {}", e))
        })?;
    }

    let pool_options = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(config.pool_size)
        .min_connections(config.min_idle.unwrap_or(1))
        .acquire_timeout(std::time::Duration::from_secs(config.connection_timeout))
        .idle_timeout(Some(std::time::Duration::from_secs(600)))
        // Issue #57: Serialize writes through a single connection to eliminate lock contention.
        // SQLite WAL mode allows concurrent reads but only one writer at a time; having a
        // max writer pool size of 1 eliminates "database is locked" errors under concurrent load.
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                use sqlx::Connection;
                // Enable WAL mode for better read/write concurrency
                sqlx::query("PRAGMA journal_mode=WAL")
                    .execute(&mut *conn)
                    .await?;
                // Reduce fsync frequency — safe with WAL (no data loss on OS crash)
                sqlx::query("PRAGMA synchronous=NORMAL")
                    .execute(&mut *conn)
                    .await?;
                // 64 MB page cache — reduces I/O for repeated reads
                sqlx::query("PRAGMA cache_size=-65536")
                    .execute(&mut *conn)
                    .await?;
                // Store temp tables in memory to avoid disk I/O
                sqlx::query("PRAGMA temp_store=MEMORY")
                    .execute(&mut *conn)
                    .await?;
                // 5 s busy timeout — avoids immediate "database is locked" on contention
                sqlx::query("PRAGMA busy_timeout=5000")
                    .execute(&mut *conn)
                    .await?;
                // WAL auto-checkpoint at 1000 pages to bound WAL file growth
                sqlx::query("PRAGMA wal_autocheckpoint=1000")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        });

    let sqlite_url = format!("sqlite:{}", db_path);
    let sqlx_pool = pool_options.connect(&sqlite_url).await.map_err(|e| {
        error!("SQLite connection failed: {}", e);
        DbInitError(format!("SQLite connection failed: {}", e))
    })?;
    info!("SQLite connected, pool initialized");

    sqlx_pool.acquire().await.map_err(|e| {
        error!("SQLite pool health check failed: {}", e);
        DbInitError(format!("SQLite pool health check failed: {}", e))
    })?;
    info!("SQLite pool health check passed");

    info!("Running migrations...");
    let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations_sqlite");
    info!("Migrations path: {:?}", migrations_path);

    // Check if SQLite migrations exist, otherwise use PostgreSQL migrations
    if !migrations_path.exists() {
        info!("SQLite migrations not found, using PostgreSQL migrations");
        let pg_migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        if pg_migrations_path.exists() {
            let migrator = Migrator::new(pg_migrations_path).await.map_err(|e| {
                error!("Failed to create migrator: {}", e);
                DbInitError(format!("Failed to create migrator: {}", e))
            })?;

            // SQLite uses slightly different syntax, we'll run basic migrations
            // Note: Some PostgreSQL-specific migrations may need SQLite versions
            migrator.run(&sqlx_pool).await.map_err(|e| {
                error!("Migrations failed: {}", e);
                DbInitError(format!("Failed to run migrations: {}", e))
            })?;
        }
    } else {
        let migrator = Migrator::new(migrations_path).await.map_err(|e| {
            error!("Failed to create migrator: {}", e);
            DbInitError(format!("Failed to create migrator: {}", e))
        })?;

        migrator.run(&sqlx_pool).await.map_err(|e| {
            error!("Migrations failed: {}", e);
            DbInitError(format!("Failed to run migrations: {}", e))
        })?;
    }
    info!("Migrations completed");

    DATABASE_POOL
        .set(DatabasePool::Sqlite(sqlx_pool))
        .map_err(|_| DbInitError("database pool already set".to_string()))?;
    info!("SQLite initialization complete");
    Ok(())
}

/// Get PostgreSQL pool (panics if not PostgreSQL)
pub fn pool() -> &'static PgPool {
    match DATABASE_POOL.get() {
        Some(DatabasePool::Postgres(p)) => p,
        Some(DatabasePool::Sqlite(_)) => panic!("Expected PostgreSQL pool, got SQLite"),
        None => panic!("Database pool not initialized"),
    }
}

/// Get SQLite pool (panics if not SQLite)
pub fn sqlite_pool() -> &'static SqlitePool {
    match DATABASE_POOL.get() {
        Some(DatabasePool::Sqlite(p)) => p,
        Some(DatabasePool::Postgres(_)) => panic!("Expected SQLite pool, got PostgreSQL"),
        None => panic!("Database pool not initialized"),
    }
}

/// Check which database backend is being used
pub fn is_sqlite() -> bool {
    matches!(DATABASE_POOL.get(), Some(DatabasePool::Sqlite(_)))
}

/// Check which database backend is being used
pub fn is_postgres() -> bool {
    matches!(DATABASE_POOL.get(), Some(DatabasePool::Postgres(_)))
}

/// Initialize database based on storage config
pub async fn init_storage(config: &StorageConfig) -> Result<(), DbInitError> {
    let db_config = DbConfig {
        backend: match config.backend {
            crate::config::StorageBackend::Postgres => DatabaseBackend::Postgres,
            crate::config::StorageBackend::Sqlite => DatabaseBackend::Sqlite,
        },
        url: config.url.clone(),
        path: None,
        pool_size: if matches!(config.backend, crate::config::StorageBackend::Sqlite) {
            5
        } else {
            10
        },
        min_idle: Some(1),
        tcp_timeout: 10000,
        connection_timeout: 30,
        statement_timeout: 30,
        helper_threads: 4,
        enforce_tls: false,
    };
    init(&db_config).await
}
