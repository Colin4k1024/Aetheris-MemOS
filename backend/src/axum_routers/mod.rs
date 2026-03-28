//! Axum Router Module
//!
//! This module provides Axum-based API routes to replace Salvo.

pub mod agent;
pub mod auth;
pub mod demo;
pub mod memory;
pub mod memory_search;
pub mod memory_storage;
pub mod knowledge_graph;
pub mod multimodal;
pub mod user;

use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};

use utoipa::OpenApi;

use crate::web::cors_layer;

#[derive(OpenApi, Default)]
#[openapi(
    info(title = "Adaptive Memory System API", version = "0.0.1"),
    paths(
        demo::hello,
        auth::register,
        auth::post_login,
        auth::get_current_user,
        user::list_users,
        user::create_user,
        user::update_user,
        user::delete_user,
        agent::create_agent,
        agent::list_agents,
        agent::get_agent,
        agent::update_agent,
        agent::delete_agent,
        agent::get_self_model,
        agent::update_self_model,
        agent::trigger_reflection,
        agent::add_capability,
        agent::list_capabilities,
        agent::update_capability,
        agent::delete_capability,
        agent::record_episode,
        agent::list_episodes,
        agent::update_episode,
        agent::record_behavior,
        agent::list_behaviors,
        agent::get_agent_complete,
        memory::health_check,
        memory::get_memory_status,
        memory::select_memory_config,
        memory::get_decision_traces,
        memory::get_workflow_evidence,
        memory::get_memory_config,
        memory::list_memory_configs,
        memory::create_memory_config,
        memory::update_memory_config,
        memory::delete_memory_config,
        memory::get_resources,
        memory::get_weight_history,
        memory::get_config,
        memory::analyze_task_characteristics,
        memory::batch_analyze_characteristics,
        memory::predict_performance,
        memory::get_baselines,
        memory::calculate_cost_benefit,
        memory::optimize,
        memory::adjust_weights,
        memory::select_memory_config_trace,
        memory_storage::list_sessions,
        memory_storage::store_stm,
        memory_storage::get_session_messages,
        memory_storage::store_ltm,
        memory_storage::transfer_stm_to_ltm,
        memory_storage::batch_store_ltm,
        memory_search::list_ltm_entries,
        memory_search::search_ltm,
        memory_search::get_ltm_entry,
        memory_search::search_stm,
        memory_search::hybrid_search,
        memory_search::search_by_entity,
        knowledge_graph::list_entities,
        knowledge_graph::create_entity,
        knowledge_graph::get_entity_by_name,
        knowledge_graph::get_related_entities,
        knowledge_graph::create_relation,
        knowledge_graph::search_by_entity,
        multimodal::store_mm,
        multimodal::get_mm,
        multimodal::get_session_mm,
        multimodal::get_by_modality,
        multimodal::list_mm,
    )
)]
struct ApiDoc;

async fn openapi_json() -> impl IntoResponse {
    let openapi = ApiDoc::openapi();
    let json = serde_json::to_string(&openapi).unwrap_or_default();
    json
}

async fn scalar_ui() -> Html<String> {
    Html(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width,initial-scale=1" />
    <title>API Scalar</title>
  </head>
  <body style="margin:0">
    <script id="api-reference" data-url="/api-doc/openapi.json"></script>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
  </body>
</html>"#
        .to_string(),
    )
}

async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Html("Page not found".to_string()))
}

/// Create the main Axum router
pub fn create_router() -> Router {
    let cors = cors_layer();

    Router::new()
        .route("/api-doc/openapi.json", get(|| async { openapi_json().await }))
        .route("/scalar", get(scalar_ui))
        .route("/scalar/", get(scalar_ui))
        .merge(demo::router())
        .merge(auth::router())
        .merge(user::router())
        .merge(agent::router())
        .merge(memory::router())
        .merge(memory_storage::router())
        .merge(memory_search::router())
        .merge(knowledge_graph::router())
        .merge(multimodal::router())
        .layer(cors)
        .fallback(not_found)
}
