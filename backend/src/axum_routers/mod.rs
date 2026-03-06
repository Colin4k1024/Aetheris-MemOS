//! Axum Router Module
//!
//! This module provides Axum-based API routes to replace Salvo.

pub mod auth;
pub mod demo;
pub mod memory;
pub mod memory_search;
pub mod memory_storage;
pub mod knowledge_graph;
pub mod multimodal;
pub mod user;

use axum::{
    routing::{get, post, put, delete},
    Router,
};

use tower_http::cors::CorsLayer;

use crate::web::cors::cors_layer;

/// Create the main Axum router
pub fn create_router() -> Router {
    let cors = cors_layer();

    Router::new()
        .merge(demo::router())
        .merge(auth::router())
        .merge(user::router())
        .merge(memory::router())
        .merge(memory_storage::router())
        .merge(memory_search::router())
        .merge(knowledge_graph::router())
        .merge(multimodal::router())
        .layer(cors)
}
