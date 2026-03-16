//! Multimodal routes - simplified

use axum::{
    extract::Path,
    routing::{get, post},
    Router,
};

/// Store MM
#[utoipa::path(post, path = "/api/mm/store", tag = "Multimodal")]
async fn store_mm() -> impl axum::response::IntoResponse {
    "{}"
}

/// Get MM entry
#[utoipa::path(get, path = "/api/mm/entry/{entry_id}", tag = "Multimodal")]
async fn get_mm(Path(_entry_id): Path<String>) -> impl axum::response::IntoResponse {
    "{}"
}

/// Get session MM
#[utoipa::path(get, path = "/api/mm/session/{session_id}", tag = "Multimodal")]
async fn get_session_mm(Path(_session_id): Path<String>) -> impl axum::response::IntoResponse {
    "[]"
}

/// Get by modality
#[utoipa::path(get, path = "/api/mm/modality/{modality_type}", tag = "Multimodal")]
async fn get_by_modality(Path(_modality_type): Path<String>) -> impl axum::response::IntoResponse {
    "[]"
}

/// List MM entries
#[utoipa::path(get, path = "/api/mm/list", tag = "Multimodal")]
async fn list_mm() -> impl axum::response::IntoResponse {
    "[]"
}

/// Create multimodal routes
pub fn router() -> Router {
    Router::new()
        .route("/api/mm/store", post(store_mm))
        .route("/api/mm/entry/{entry_id}", get(get_mm))
        .route("/api/mm/session/{session_id}", get(get_session_mm))
        .route("/api/mm/modality/{modality_type}", get(get_by_modality))
        .route("/api/mm/list", get(list_mm))
}
