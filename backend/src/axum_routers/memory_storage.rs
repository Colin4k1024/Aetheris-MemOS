//! Memory storage routes - simplified

use axum::{
    routing::{get, post},
    Router,
};

/// List sessions
async fn list_sessions() -> impl axum::response::IntoResponse {
    "[]"
}

/// Store STM
async fn store_stm() -> impl axum::response::IntoResponse {
    "{}"
}

/// Get session messages
async fn get_session_messages() -> impl axum::response::IntoResponse {
    "[]"
}

/// Store LTM
async fn store_ltm() -> impl axum::response::IntoResponse {
    "{}"
}

/// Transfer STM to LTM
async fn transfer_stm_to_ltm() -> impl axum::response::IntoResponse {
    "{}"
}

/// Batch store LTM
async fn batch_store_ltm() -> impl axum::response::IntoResponse {
    "{}"
}

/// Create memory storage routes
pub fn router() -> Router {
    Router::new()
        .route("/api/v1/memory/storage/sessions", get(list_sessions))
        .route("/api/v1/memory/storage/stm", post(store_stm))
        .route("/api/v1/memory/storage/stm/{session_id}", get(get_session_messages))
        .route("/api/v1/memory/storage/ltm", post(store_ltm))
        .route("/api/v1/memory/storage/transfer", post(transfer_stm_to_ltm))
        .route("/api/v1/memory/storage/batch-ltm", post(batch_store_ltm))
}
