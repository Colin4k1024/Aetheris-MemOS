use axum::Json;
use serde::Serialize;
use tokio::signal;
use tracing::info;
use utoipa::ToSchema;

mod config;
mod db;
mod error;
mod hoops;
mod kernel;
mod models;
mod protocol;
mod routers;
mod axum_routers;
mod services;
mod utils;
mod web;

pub use error::AppError;

pub type AppResult<T> = Result<T, AppError>;
pub type JsonResult<T> = Result<Json<T>, AppError>;
pub type EmptyResult = JsonResult<Empty>;

pub fn json_ok<T>(data: T) -> JsonResult<T> {
    Ok(Json(data))
}

#[derive(Serialize, ToSchema, Clone, Copy, Debug)]
pub struct Empty {}

pub fn empty_ok() -> JsonResult<Empty> {
    Ok(Json(Empty {}))
}

#[tokio::main]
async fn main() {
    crate::config::init();
    let config = crate::config::get();
    crate::db::init(&config.db)
        .await
        .expect("Database initialization failed");

    tracing::info!("Initializing Neo4j connection");
    let _ = crate::db::init_neo4j(&config.neo4j).await;
    tracing::info!("Neo4j connection initialized successfully");

    tracing::info!("Initializing Neo4j indexes and constraints");
    crate::db::init_neo4j_indexes()
        .await
        .expect("Failed to initialize Neo4j indexes");
    tracing::info!("Neo4j indexes and constraints initialized successfully");

    let _guard = config.log.guard();
    tracing::info!("log level: {}", &config.log.filter_level);

    tracing::info!("Initializing memory transfer service");
    crate::services::memory_transfer::init_transfer_service(
        config.memory_transfer.check_interval,
        config.memory_transfer.message_count_threshold,
        config.memory_transfer.session_time_threshold,
    )
    .await
    .expect("Failed to initialize memory transfer service");
    tracing::info!("Memory transfer service initialized successfully");

    let app = axum_routers::create_router().layer(hoops::cors_hoop());
    println!("🔄 在以下位置监听 {}", &config.listen_addr);

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

        let addr: std::net::SocketAddr = listen_addr.parse().expect("invalid listen address");
        let rustls_config =
            axum_server::tls_rustls::RustlsConfig::from_pem_file(tls.cert.clone(), tls.key.clone())
                .await
                .expect("failed to load tls cert/key");
        let handle = axum_server::Handle::new();
        tokio::spawn(shutdown_signal_with_handle(handle.clone()));
        axum_server::bind_rustls(addr, rustls_config)
            .handle(handle)
            .serve(app.into_make_service())
            .await
            .expect("axum tls server failed");
    } else {
        println!(
            "📖 Open API 页面: http://{}/scalar",
            config.listen_addr.replace("0.0.0.0", "127.0.0.1")
        );
        println!(
            "🔑 Login Page: http://{}/login",
            config.listen_addr.replace("0.0.0.0", "127.0.0.1")
        );
        let listener = tokio::net::TcpListener::bind(&config.listen_addr)
            .await
            .expect("failed to bind listener");
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("axum server failed");
    }
}

async fn wait_shutdown_signal() {
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

async fn shutdown_signal() {
    wait_shutdown_signal().await;
}

async fn shutdown_signal_with_handle(handle: axum_server::Handle) {
    wait_shutdown_signal().await;
    handle.graceful_shutdown(Some(std::time::Duration::from_secs(60)));
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::config;

    #[tokio::test]
    async fn test_hello_world() {
        config::init();

        let app = crate::routers::root();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(axum::body::Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("request success");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let content = String::from_utf8(body.to_vec()).expect("utf8 body");
        assert_eq!(content, "Hello World from axum");
    }
}
