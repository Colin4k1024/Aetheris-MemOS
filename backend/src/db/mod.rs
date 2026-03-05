//! Database layer: SQLite is the default adapter for local & demo.
//! Adapter abstraction (e.g. PostgreSQL/MySQL) is planned for production; see docs/ROADMAP.md.

use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
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

/// Adapter namespace: current implementation uses SQLite directly in this crate.
/// Future: move SQLite behind `adapters::sqlite`, add `adapters::postgres` etc.
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
pub use kg::{Entity, KGRepository, Relation};
pub use ltm::KnowledgeEntry;
pub use mm::{MMRepository, ModalityRelation, MultimodalEntry};
pub use neo4j::{create_session, driver, init as init_neo4j, init_neo4j_indexes};
pub use stm::{Session, SessionMessage};

pub static SQLX_POOL: OnceLock<SqlitePool> = OnceLock::new();

pub async fn init(config: &DbConfig) -> Result<(), DbInitError> {
    // 连接数据库
    info!("正在连接数据库: {}", config.url);

    // 配置连接池选项
    let pool_options = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(10)
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(Some(std::time::Duration::from_secs(600)))
        .max_lifetime(Some(std::time::Duration::from_secs(1800)));

    let sqlx_pool = pool_options.connect(&config.url).await.map_err(|e| {
        error!("Database connection failed: {}", e);
        DbInitError(format!("Database connection failed: {}", e))
    })?;
    info!("数据库连接成功，连接池已初始化");

    // 健康检查
    sqlx_pool.acquire().await.map_err(|e| {
        error!("数据库连接池健康检查失败: {}", e);
        DbInitError(format!("Database pool health check failed: {}", e))
    })?;
    info!("数据库连接池健康检查通过");

    // 执行迁移
    info!("开始执行数据库迁移...");
    let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    info!("迁移文件目录: {:?}", migrations_path);

    let migrator = Migrator::new(migrations_path).await.map_err(|e| {
        error!("创建迁移器失败: {}", e);
        DbInitError(format!("Failed to create migrator: {}", e))
    })?;

    migrator.run(&sqlx_pool).await.map_err(|e| {
        error!("数据库迁移执行失败: {}", e);
        DbInitError(format!("Failed to run migrations: {}", e))
    })?;
    info!("数据库迁移执行成功");

    // 设置连接池
    crate::db::SQLX_POOL.set(sqlx_pool).map_err(|_| {
        DbInitError("sqlx pool already set".to_string())
    })?;
    info!("数据库初始化完成");
    Ok(())
}

pub fn pool() -> &'static SqlitePool {
    SQLX_POOL.get().expect("sqlx pool should be set")
}
