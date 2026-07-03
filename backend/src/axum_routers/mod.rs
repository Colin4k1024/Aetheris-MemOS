//! Axum Router Module
//!
//! This module provides Axum-based API routes to replace Salvo.

pub mod agent;
pub mod auth;
pub mod demo;
pub mod distributed;

use axum::Router;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::web::cors_layer;

/// Create the main Axum router.
///
/// ## Public routes (no auth)
/// - `/api-doc/openapi.json` - OpenAPI spec
/// - `/scalar`, `/scalar/` - API docs UI
/// - `/login`, `/register` - Auth page handlers
/// - `/api/login` - Login API endpoint
/// - `/` - Demo hello
///
/// ## Protected routes (auth required via httpOnly cookie or Bearer header)
/// All other routes require a valid JWT. The auth middleware is applied
/// in `protected::protected_router()`.
///
/// ## Observability layers (outermost → innermost)
/// 1. `TraceLayer` — creates a per-request span with method/path/status/latency
/// 2. CORS hoop
pub fn create_router() -> Router {
    let cors = cors_layer();

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(
            DefaultMakeSpan::new()
                .level(Level::INFO)
                .include_headers(false),
        )
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    crate::routers::root()
        .layer(trace_layer)
        .layer(cors)
}
