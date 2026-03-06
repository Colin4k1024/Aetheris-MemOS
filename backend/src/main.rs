use salvo::catcher::Catcher;
use salvo::conn::rustls::{Keycert, RustlsConfig};
use salvo::prelude::*;
use salvo::server::ServerHandle;
use serde::Serialize;
use tokio::signal;
use tracing::info;

mod config;
mod db;
mod hoops;
mod models;
mod routers;
mod services;
mod utils;

mod error;
pub use error::AppError;

pub type AppResult<T> = Result<T, AppError>;
pub type JsonResult<T> = Result<Json<T>, AppError>;
pub type EmptyResult = Result<Json<Empty>, AppError>;

pub fn json_ok<T>(data: T) -> JsonResult<T> {
    Ok(Json(data))
}

#[derive(Serialize, ToSchema, Clone, Copy, Debug)]
pub struct Empty {}

pub fn empty_ok() -> JsonResult<Empty> {
    Ok(Json(Empty {}))
}

/// 初始化数据库连接
async fn init_database() -> AppResult<()> {
    let config = crate::config::get();
    crate::db::init(&config.db)
        .await
        .map_err(|e| AppError::DatabaseConnection(format!("Database initialization failed: {}", e)))?;
    Ok(())
}

/// 初始化 Neo4j 连接
async fn init_neo4j() -> AppResult<()> {
    let config = crate::config::get();
    
    tracing::info!("Initializing Neo4j connection");
    let _ = crate::db::init_neo4j(&config.neo4j).await;
    
    tracing::info!("Initializing Neo4j indexes and constraints");
    crate::db::init_neo4j_indexes()
        .await
        .map_err(|e| AppError::DatabaseConnection(format!("Neo4j indexes initialization failed: {}", e)))?;
    
    tracing::info!("Neo4j connection initialized successfully");
    Ok(())
}

/// 初始化记忆转移服务
async fn init_memory_transfer() -> AppResult<()> {
    let config = crate::config::get();
    
    tracing::info!("Initializing memory transfer service");
    crate::services::memory_transfer::init_transfer_service(
        config.memory_transfer.check_interval,
        config.memory_transfer.message_count_threshold,
        config.memory_transfer.session_time_threshold,
    )
    .await
    .map_err(|e| AppError::Internal(format!("Memory transfer service initialization failed: {}", e)))?;
    
    tracing::info!("Memory transfer service initialized successfully");
    Ok(())
}

#[tokio::main]
async fn main() {
    // 初始化配置
    crate::config::init();
    let config = crate::config::get();

    // 初始化数据库
    if let Err(e) = init_database().await {
        eprintln!("❌ Database initialization failed: {}", e);
        std::process::exit(1);
    }

    // 初始化 Neo4j
    if let Err(e) = init_neo4j().await {
        eprintln!("❌ Neo4j initialization failed: {}", e);
        std::process::exit(1);
    }

    // 初始化日志
    let _guard = config.log.guard();
    tracing::info!("log level: {}", &config.log.filter_level);

    // 初始化记忆转移服务
    if let Err(e) = init_memory_transfer().await {
        eprintln!("❌ Memory transfer service initialization failed: {}", e);
        std::process::exit(1);
    }

    let service = Service::new(routers::root())
        .catcher(Catcher::default().hoop(hoops::error_404))
        .hoop(hoops::cors_hoop());
    
    println!("🔄 在以下位置监听 {}", &config.listen_addr);

    // TLS 支持
    if let Some(tls) = &config.tls {
        let listen_addr = &config.listen_addr;
        println!(
            "📖 Open API Page: https://{}/scalar",
            listen_addr.replace("0.0.0.0", "127.0.0.1")
        );
        println!(
            "🔑 Login Page: https://{}/login",
            listen_addr.replace("0.0.0.0", "127.0.0.1")
        );
        
        let tls_config = RustlsConfig::new(
            Keycert::new()
                .cert(tls.cert.clone())
                .key(tls.key.clone())
        );

        let acceptor = TcpListener::new(listen_addr).rustls(tls_config).bind().await;
        let server = Server::new(acceptor);
        tokio::spawn(shutdown_signal(server.handle()));
        server.serve(service).await;
    } else {
        println!(
            "📖 Open API 页面: http://{}/scalar",
            config.listen_addr.replace("0.0.0.0", "127.0.0.1")
        );
        println!(
            "🔑 Login Page: http://{}/login",
            config.listen_addr.replace("0.0.0.0", "127.0.0.1")
        );
        
        let acceptor = TcpListener::new(&config.listen_addr).bind().await;
        let server = Server::new(acceptor);
        tokio::spawn(shutdown_signal(server.handle()));
        server.serve(service).await;
    }
}

async fn shutdown_signal(handle: ServerHandle) {
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
    handle.stop_graceful(std::time::Duration::from_secs(60));
}

#[cfg(test)]
mod tests {
    use salvo::prelude::*;
    use salvo::test::{ResponseExt, TestClient};

    use crate::config;

    #[tokio::test]
    async fn test_hello_world() {
        config::init();

        let service = Service::new(crate::routers::root());

        let content = TestClient::get(format!(
            "http://{}",
            config::get().listen_addr.replace("0.0.0.0", "127.0.0.1")
        ))
        .send(&service)
        .await
        .take_string()
        .await
        .expect("test response body");
        assert_eq!(content, "Hello World from salvo");
    }
}
