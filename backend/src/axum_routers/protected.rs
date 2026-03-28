//! Protected routes module
//!
//! Defines which routes require authentication vs which are public.
//!
//! ## Public routes (no auth required)
//! - `/api-doc/openapi.json` - OpenAPI spec endpoint
//! - `/scalar`, `/scalar/` - API documentation UI
//! - `/login`, `/register` - Auth page handlers
//! - `/api/login` - Login API endpoint
//! - `/` - Demo hello
//!
//! ## Protected routes (auth required)
//! All other routes require a valid JWT token via httpOnly cookie
//! or Authorization header. The auth middleware is applied in
//! `create_router()` via `protected_router()`.

use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};

use super::auth;
use super::user;
use super::agent;
use super::memory;
use super::memory_storage;
use super::memory_search;
use super::knowledge_graph;
use super::multimodal;
use crate::hoops::jwt::auth_middleware;

/// Create a router with all protected (auth-required) routes.
///
/// This router is merged into the main router with `auth_middleware` applied.
/// All routes here require a valid JWT token.
pub fn protected_router() -> Router {
    Router::new()
        // Auth - current user endpoint
        .merge(auth::protected_router())
        // User management
        .merge(user::router())
        // Agent management
        .merge(agent::router())
        // Memory management
        .merge(memory::router())
        // Memory storage
        .merge(memory_storage::router())
        // Memory search
        .merge(memory_search::router())
        // Knowledge graph
        .merge(knowledge_graph::router())
        // Multimodal
        .merge(multimodal::router())
        .layer(middleware::from_fn(auth_middleware))
}
