use axum::{
    routing::get,
    Router,
};
use tokio::signal;
use tracing::info;

mod config;
mod db;
mod hoops;
mod models;
// mod routers; // TODO: 迁移后删除
mod services;
mod utils;

mod error;
pub use error::AppError;

// Axum web module
mod web;
pub use web::{json_ok as axum_json_ok, AppError as AxumAppError};

// Axum routers module
mod axum_routers;

pub type AppResult<T> = Result<T, AppError>;

/// 初始化数据库连接
async fn init_database() {
    let config = crate::config::get();
    match crate::db::init(&config.db).await {
        Ok(_) => tracing::info!("Database connected successfully"),
        Err(e) => tracing::warn!("Database connection failed: {}. Server will run without database.", e),
    }
}

/// 初始化 Neo4j 连接
async fn init_neo4j() {
    let config = crate::config::get();

    tracing::info!("Initializing Neo4j connection");
    let _ = crate::db::init_neo4j(&config.neo4j).await;

    tracing::info!("Initializing Neo4j indexes and constraints");
    if let Err(e) = crate::db::init_neo4j_indexes().await {
        tracing::warn!("Neo4j indexes initialization failed: {}", e);
    } else {
        tracing::info!("Neo4j connection initialized successfully");
    }
}

/// 初始化记忆转移服务
async fn init_memory_transfer() {
    let config = crate::config::get();

    tracing::info!("Initializing memory transfer service");
    match crate::services::memory_transfer::init_transfer_service(
        config.memory_transfer.check_interval,
        config.memory_transfer.message_count_threshold,
        config.memory_transfer.session_time_threshold,
    ).await {
        Ok(_) => tracing::info!("Memory transfer service initialized successfully"),
        Err(e) => tracing::warn!("Memory transfer service initialization failed: {}", e),
    }
}

#[tokio::main]
async fn main() {
    // 初始化配置
    crate::config::init();
    let config = crate::config::get();

    // 初始化日志
    let _guard = config.log.guard();
    tracing::info!("log level: {}", &config.log.filter_level);

    // 初始化数据库（可选）
    init_database().await;

    // 初始化 Neo4j（可选）
    init_neo4j().await;

    // 初始化记忆转移服务（可选）
    init_memory_transfer().await;

    // 创建 Axum 路由
    let app = axum_routers::create_router();

    let listen_addr = &config.listen_addr;
    println!("🔄 在以下位置监听 {}", listen_addr);

    // 启动服务器
    let listener = tokio::net::TcpListener::bind(listen_addr).await.unwrap();
    println!(
        "📖 Open API 页面: http://{}/docs",
        listen_addr.replace("0.0.0.0", "127.0.0.1")
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("ctrl_c signal received"),
        _ = terminate => info!("terminate signal received"),
    }
}
