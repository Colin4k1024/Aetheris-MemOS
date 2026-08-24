#![recursion_limit = "256"]

use std::net::SocketAddr;

use axum::Json;
use serde::Serialize;
use tokio::signal;
use tracing::info;
use utoipa::ToSchema;

#[cfg(feature = "a2a")]
mod a2a;
mod agent;
mod axum_routers;
mod config;
mod db;
mod distributed;
mod error;
mod hoops;
mod integrations;
mod kernel;
mod layers;
mod mcp;
mod models;
mod otel;
mod protocol;
mod providers;
mod routers;
mod runtime;
mod services;
mod tenant;
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

    // Fail-fast if auth is enabled but the JWT secret is a placeholder/too weak.
    // Skipped when jwt.disabled is set (explicit local/dev mode).
    if let Err(e) = crate::config::validate_jwt_security(config) {
        eprintln!("[startup] {e}");
        std::process::exit(1);
    }

    // Initialize tracing subscriber (fmt + optional OTLP) before any tracing:: calls.
    // The guard must be held for the process lifetime — dropping it flushes spans.
    let _tracing_guard = otel::init_tracing(&config.log, &config.otel);

    if config.jwt.disabled {
        tracing::warn!(
            "\n\
             ╔══════════════════════════════════════════════════════════════╗\n\
             ║  AUTH IS DISABLED — GOVERNANCE & RBAC ARE SKIPPED           ║\n\
             ║  Every request runs unauthenticated as a single shared      ║\n\
             ║  anonymous tenant. This configuration must NEVER be used    ║\n\
             ║  outside local development.                                 ║\n\
             ╚══════════════════════════════════════════════════════════════╝"
        );
    }

    crate::db::init(&config.db)
        .await
        .expect("Database initialization failed");

    // Issue #57: start the SQLite write-queue to coalesce concurrent writes
    if crate::db::is_sqlite() {
        crate::services::write_queue::init_write_queue();
    }

    // Issue #49: detect hardware capabilities (CUDA / Metal / CPU) once at startup
    crate::services::hardware_detector::init();

    // Issue #59: validate vector-space signature (abort if dimension mismatch)
    if let Err(e) = crate::services::vector_guard::init() {
        eprintln!("[startup] Vector guard error: {}", e);
        std::process::exit(1);
    }

    // Issue #50: start the proactive layered memory ingestion / reflection daemon
    crate::services::memory_ingestion::init_reflection_daemon(
        crate::services::memory_ingestion::IngestionConfig::default(),
    );

    // Issue #58: initialise the write journal and integrity scanner
    crate::services::information_guard::init_write_journal();
    crate::services::information_guard::init_integrity_scanner();

    // P1 governance: start the best-effort async audit writer (PostgreSQL only).
    // Mirrors the is_sqlite() gating of the write_queue above.
    if crate::db::is_postgres() {
        crate::services::audit_writer::init_audit_writer();
        // ADR-0002: durable LTM↔Qdrant outbox consumer (PG only).
        crate::services::outbox_worker::init_outbox_worker();
        // W1.1: periodic PG↔Qdrant drift scanner — the backstop for outbox
        // events that never reach Qdrant. Read-only (dry_run) unless
        // reconciliation.mode is explicitly set to "repair".
        crate::services::vector_reconciliation::init_reconciliation_scanner(&config.reconciliation);
    }

    // P1 governance: register the enterprise hooks (RBAC/quota/audit) singleton that
    // hoops::governance::governance_middleware consumes. Not gated on is_postgres —
    // governance is authorization and applies to every backend; audit events on a
    // non-PG backend are dropped+counted by the writer, which is harmless.
    hoops::init_enterprise_hooks(hoops::create_enterprise_hook_set());

    // Issue #55: adaptive strategy mutation daemon — DISABLED (P0 cleanup).
    // The mutator runs a heuristic hill-climb but its output
    // (strategy_mutator::current_hyperparams / BEST_PARAMS) is consumed by nothing —
    // scheduler/predictor never read it — so running it only emits misleading
    // "self-optimizing" logs. Re-enable in P3 once the scheduler actually consumes
    // learned hyperparameters. Manual run_mutation_cycle() remains available.
    // if crate::services::strategy_mutator::MutationConfig::default().auto_mutate {
    //     crate::services::strategy_mutator::StrategyMutator::init_mutation_daemon(
    //         crate::services::strategy_mutator::MutationConfig::default(),
    //     );
    // }

    // Issue #61: initialize distributed epoch manager and interrupt propagator
    crate::axum_routers::distributed::init_distributed();

    // Neo4j is an OPTIONAL dependency. Its connect path can block for up to ~60s
    // per query under neo4rs' internal backoff (a wrong password authenticates
    // never), so it must not sit on the startup critical path. spawn_neo4j_init
    // returns immediately and runs connect + index creation in a bounded
    // background task; the HTTP server starts listening regardless. Real status
    // is observable via crate::db::neo4j_status(), not an optimistic log line.
    crate::db::spawn_neo4j_init(&config.neo4j);

    // Issue #86: WebSocket connection manager singleton — used by the
    // /api/v1/ws upgrade handler and by memory write/delete event emitters.
    crate::protocol::websocket::init_ws_manager();
    tracing::info!("WebSocket connection manager initialized");

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
    tracing::info!("🔄 Listening on {}", &config.listen_addr);

    if let Some(tls) = &config.tls {
        let listen_addr = &config.listen_addr;
        tracing::info!(
            "📖 Open API Page: https://{}/scalar",
            listen_addr.replace("0.0.0.0", "127.0.0.1")
        );
        tracing::info!(
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
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .expect("axum tls server failed");
    } else {
        tracing::info!(
            "📖 Open API Page: http://{}/scalar",
            config.listen_addr.replace("0.0.0.0", "127.0.0.1")
        );
        tracing::info!(
            "🔑 Login Page: http://{}/login",
            config.listen_addr.replace("0.0.0.0", "127.0.0.1")
        );
        // gRPC server on port 50051 (separate from the HTTP surface).
        // Uses the same auth interceptor as the REST middleware — ADR-0007 transport parity.
        tokio::spawn(async {
            let grpc_addr: SocketAddr = "0.0.0.0:50051".parse().unwrap();
            let svc = tonic::transport::Server::builder()
                .add_service(
                    crate::protocol::grpc_service::pb::memory_service_server::MemoryServiceServer::with_interceptor(
                        crate::protocol::grpc_service::MemoryServiceImpl,
                        crate::protocol::grpc::grpc_auth_interceptor,
                    ),
                )
                .serve(grpc_addr);
            info!("🔌 gRPC listening on {grpc_addr}");
            if let Err(e) = svc.await {
                tracing::error!("gRPC server error: {e}");
            }
        });

        let listener = tokio::net::TcpListener::bind(&config.listen_addr)
            .await
            .expect("failed to bind listener");
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
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
