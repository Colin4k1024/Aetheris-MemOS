//! Memory search routes - simplified

use axum::{
    extract::Path,
    routing::{get, post},
    Router,
};

/// List LTM entries
async fn list_ltm_entries() -> impl axum::response::IntoResponse {
    "[]"
}

/// Search LTM
async fn search_ltm() -> impl axum::response::IntoResponse {
    "[]"
}

/// Get LTM entry
async fn get_ltm_entry(Path(_entry_id): Path<String>) -> impl axum::response::IntoResponse {
    "{}"
}

/// Search STM
async fn search_stm() -> impl axum::response::IntoResponse {
    "[]"
}

/// Hybrid search
async fn hybrid_search() -> impl axum::response::IntoResponse {
    "[]"
}

/// Search by entity
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
