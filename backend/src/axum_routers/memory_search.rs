//! Memory search routes - simplified

use axum::{
    extract::Path,
    routing::{get, post},
    Router,
};

/// List LTM entries
#[utoipa::path(get, path = "/api/v1/memory/search/ltm", tag = "Search")]
async fn list_ltm_entries() -> impl axum::response::IntoResponse {
    "[]"
}

/// Search LTM
#[utoipa::path(post, path = "/api/v1/memory/search/ltm", tag = "Search")]
async fn search_ltm() -> impl axum::response::IntoResponse {
    "[]"
}

/// Get LTM entry
#[utoipa::path(get, path = "/api/v1/memory/search/ltm/{entry_id}", tag = "Search")]
async fn get_ltm_entry(Path(_entry_id): Path<String>) -> impl axum::response::IntoResponse {
    "{}"
}

/// Search STM
#[utoipa::path(post, path = "/api/v1/memory/search/stm", tag = "Search")]
async fn search_stm() -> impl axum::response::IntoResponse {
    "[]"
}

/// Hybrid search
#[utoipa::path(post, path = "/api/v1/memory/search/hybrid", tag = "Search")]
async fn hybrid_search() -> impl axum::response::IntoResponse {
    "[]"
}

/// Search by entity
#[utoipa::path(post, path = "/api/v1/memory/search/entity", tag = "Search")]
async fn search_by_entity() -> impl axum::response::IntoResponse {
    "[]"
}

/// Create memory search routes
pub fn router() -> Router {
    Router::new()
        .route("/api/v1/memory/search/ltm", get(list_ltm_entries))
        .route("/api/v1/memory/search/ltm", post(search_ltm))
        .route("/api/v1/memory/search/ltm/{entry_id}", get(get_ltm_entry))
        .route("/api/v1/memory/search/stm", post(search_stm))
        .route("/api/v1/memory/search/hybrid", post(hybrid_search))
        .route("/api/v1/memory/search/entity", post(search_by_entity))
}
