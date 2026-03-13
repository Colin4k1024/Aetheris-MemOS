//! Knowledge graph routes - simplified

use axum::{
    extract::Path,
    routing::{get, post},
    Router,
};

/// List entities
async fn list_entities() -> impl axum::response::IntoResponse {
    "[]"
}

/// Create entity
async fn create_entity() -> impl axum::response::IntoResponse {
    "{}"
}

/// Get entity by name
async fn get_entity_by_name(Path(_name): Path<String>) -> impl axum::response::IntoResponse {
    "{}"
}

/// Get related entities
async fn get_related_entities(Path(_entity_id): Path<String>) -> impl axum::response::IntoResponse {
    "[]"
}

/// Create relation
async fn create_relation() -> impl axum::response::IntoResponse {
    "{}"
}

/// Search by entity
async fn search_by_entity() -> impl axum::response::IntoResponse {
    "[]"
}

/// Create knowledge graph routes
pub fn router() -> Router {
    Router::new()
        .route("/api/kg/entities", get(list_entities))
        .route("/api/kg/entities", post(create_entity))
        .route("/api/kg/entities/by-name/{name}", get(get_entity_by_name))
        .route("/api/kg/entities/{entity_id}/related", get(get_related_entities))
        .route("/api/kg/relations", post(create_relation))
        .route("/api/kg/search", post(search_by_entity))
}
