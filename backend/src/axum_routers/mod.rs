//! Axum Router Module
//!
//! This module provides Axum-based API routes to replace Salvo.

pub mod agent;
pub mod auth;
pub mod demo;
pub mod distributed;

use axum::Router;

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
pub fn create_router() -> Router {
    let cors = cors_layer();

    crate::routers::root().layer(cors)
}
