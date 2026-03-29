//! Knowledge graph routes - simplified

use axum::{
    extract::Path,
    routing::{get, post},
    Router,
};

/// List entities
#[utoipa::path(get, path = "/api/kg/entities", tag = "KnowledgeGraph")]
async fn list_entities() -> impl axum::response::IntoResponse {
    "[]"
}

/// Create entity
#[utoipa::path(post, path = "/api/kg/entities", tag = "KnowledgeGraph")]
async fn create_entity() -> impl axum::response::IntoResponse {
    "{}"
}

/// Get entity by name
#[utoipa::path(get, path = "/api/kg/entities/by-name/{name}", tag = "KnowledgeGraph")]
async fn get_entity_by_name(Path(_name): Path<String>) -> impl axum::response::IntoResponse {
    "{}"
}

/// Get related entities
#[utoipa::path(
    get,
    path = "/api/kg/entities/{entity_id}/related",
    tag = "KnowledgeGraph"
)]
async fn get_related_entities(Path(_entity_id): Path<String>) -> impl axum::response::IntoResponse {
    "[]"
}

/// Create relation
#[utoipa::path(post, path = "/api/kg/relations", tag = "KnowledgeGraph")]
async fn create_relation() -> impl axum::response::IntoResponse {
    "{}"
}

/// Search by entity
#[utoipa::path(post, path = "/api/kg/search", tag = "KnowledgeGraph")]
async fn search_by_entity() -> impl axum::response::IntoResponse {
    "[]"
}

/// Create knowledge graph routes
pub fn router() -> Router {
    Router::new()
        .route("/api/kg/entities", get(list_entities))
        .route("/api/kg/entities", post(create_entity))
        .route("/api/kg/entities/by-name/{name}", get(get_entity_by_name))
        .route(
            "/api/kg/entities/{entity_id}/related",
            get(get_related_entities),
        )
        .route("/api/kg/relations", post(create_relation))
        .route("/api/kg/search", post(search_by_entity))
}
