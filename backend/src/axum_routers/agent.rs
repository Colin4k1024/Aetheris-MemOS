//! Agent routes for axum

use axum::{
    extract::{Path, Query},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;

use crate::db;
use crate::error::AppError;
use crate::models::agent::*;
use crate::services::agent_identity::AgentService;

/// Pagination parameters
#[derive(Deserialize)]
pub struct PaginationParams {
    limit: Option<i64>,
    offset: Option<i64>,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            limit: Some(20),
            offset: Some(0),
        }
    }
}

/// Create agent router
pub fn router() -> Router {
    Router::new()
        // Agent Identity
        .route("/api/v1/agents", post(create_agent).get(list_agents))
        .route(
            "/api/v1/agents/{agent_id}",
            get(get_agent).put(update_agent).delete(delete_agent),
        )
        // Self-Model
        .route(
            "/api/v1/agents/{agent_id}/self-model",
            get(get_self_model).put(update_self_model),
        )
        .route(
            "/api/v1/agents/{agent_id}/self-model/reflect",
            post(trigger_reflection),
        )
        // Capabilities
        .route(
            "/api/v1/agents/{agent_id}/capabilities",
            get(list_capabilities).post(add_capability),
        )
        .route(
            "/api/v1/agents/{agent_id}/capabilities/{capability_id}",
            put(update_capability).delete(delete_capability),
        )
        // Episodes
        .route(
            "/api/v1/agents/{agent_id}/episodes",
            get(list_episodes).post(record_episode),
        )
        .route(
            "/api/v1/agents/{agent_id}/episodes/{episode_id}",
            put(update_episode),
        )
        // Behavior Profiles
        .route(
            "/api/v1/agents/{agent_id}/behaviors",
            get(list_behaviors).post(record_behavior),
        )
        // Complete agent info
        .route(
            "/api/v1/agents/{agent_id}/complete",
            get(get_agent_complete),
        )
}

// ============================================================================
// Agent Identity Handlers
// ============================================================================

#[utoipa::path(post, path = "/api/v1/agents", tag = "Agent")]
async fn create_agent(
    Json(payload): Json<CreateAgentIdentity>,
) -> Result<Json<AgentIdentity>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let agent = service.create_agent(payload).await?;
    Ok(Json(agent))
}

#[utoipa::path(get, path = "/api/v1/agents", tag = "Agent")]
async fn list_agents(
    Query(params): Query<PaginationParams>,
) -> Result<Json<AgentListResponse>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let limit = params.limit.unwrap_or(20);
    let offset = params.offset.unwrap_or(0);

    let agents = service.list_agents(limit, offset).await?;
    Ok(Json(agents))
}

#[utoipa::path(get, path = "/api/v1/agents/{agent_id}", tag = "Agent")]
async fn get_agent(
    Path(agent_id): Path<String>,
) -> Result<Json<AgentIdentity>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let agent = service
        .get_agent(&agent_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Agent {} not found", agent_id)))?;

    Ok(Json(agent))
}

#[utoipa::path(put, path = "/api/v1/agents/{agent_id}", tag = "Agent")]
async fn update_agent(
    Path(agent_id): Path<String>,
    Json(payload): Json<UpdateAgentIdentity>,
) -> Result<Json<AgentIdentity>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let agent = service.update_agent(&agent_id, payload).await?;
    Ok(Json(agent))
}

#[utoipa::path(delete, path = "/api/v1/agents/{agent_id}", tag = "Agent")]
async fn delete_agent(
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    service.delete_agent(&agent_id).await?;
    Ok(Json(serde_json::json!({ "message": "Agent deleted successfully" })))
}

// ============================================================================
// Self-Model Handlers
// ============================================================================

#[utoipa::path(get, path = "/api/v1/agents/{agent_id}/self-model", tag = "Agent")]
async fn get_self_model(
    Path(agent_id): Path<String>,
) -> Result<Json<AgentSelfModel>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let model = service
        .get_self_model(&agent_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Self-model for agent {} not found", agent_id)))?;

    Ok(Json(model))
}

#[utoipa::path(put, path = "/api/v1/agents/{agent_id}/self-model", tag = "Agent")]
async fn update_self_model(
    Path(agent_id): Path<String>,
    Json(payload): Json<UpdateAgentSelfModel>,
) -> Result<Json<AgentSelfModel>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let model = service.update_self_model(&agent_id, payload).await?;
    Ok(Json(model))
}

#[utoipa::path(post, path = "/api/v1/agents/{agent_id}/self-model/reflect", tag = "Agent")]
async fn trigger_reflection(
    Path(agent_id): Path<String>,
) -> Result<Json<AgentSelfModel>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let model = service.trigger_reflection(&agent_id).await?;
    Ok(Json(model))
}

// ============================================================================
// Capability Handlers
// ============================================================================

#[utoipa::path(post, path = "/api/v1/agents/{agent_id}/capabilities", tag = "Agent")]
async fn add_capability(
    Path(agent_id): Path<String>,
    Json(payload): Json<CreateAgentCapability>,
) -> Result<Json<AgentCapability>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let capability = service.add_capability(&agent_id, payload).await?;
    Ok(Json(capability))
}

#[utoipa::path(get, path = "/api/v1/agents/{agent_id}/capabilities", tag = "Agent")]
async fn list_capabilities(
    Path(agent_id): Path<String>,
) -> Result<Json<Vec<AgentCapability>>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let capabilities = service.list_capabilities(&agent_id).await?;
    Ok(Json(capabilities))
}

#[utoipa::path(put, path = "/api/v1/agents/{agent_id}/capabilities/{capability_id}", tag = "Agent")]
async fn update_capability(
    Path((_agent_id, capability_id)): Path<(String, String)>,
    Json(payload): Json<UpdateAgentCapability>,
) -> Result<Json<AgentCapability>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let capability = service.update_capability(&capability_id, payload).await?;
    Ok(Json(capability))
}

#[utoipa::path(delete, path = "/api/v1/agents/{agent_id}/capabilities/{capability_id}", tag = "Agent")]
async fn delete_capability(
    Path((_agent_id, capability_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    service.delete_capability(&capability_id).await?;
    Ok(Json(serde_json::json!({ "message": "Capability deleted successfully" })))
}

// ============================================================================
// Episode Handlers
// ============================================================================

#[utoipa::path(post, path = "/api/v1/agents/{agent_id}/episodes", tag = "Agent")]
async fn record_episode(
    Path(agent_id): Path<String>,
    Json(payload): Json<CreateAgentEpisode>,
) -> Result<Json<AgentEpisode>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let episode = service.record_episode(&agent_id, payload).await?;
    Ok(Json(episode))
}

#[utoipa::path(get, path = "/api/v1/agents/{agent_id}/episodes", tag = "Agent")]
async fn list_episodes(
    Path(agent_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<EpisodeListResponse>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let limit = params.limit.unwrap_or(20);
    let offset = params.offset.unwrap_or(0);

    let episodes = service.list_episodes(&agent_id, limit, offset).await?;
    Ok(Json(episodes))
}

#[utoipa::path(put, path = "/api/v1/agents/{agent_id}/episodes/{episode_id}", tag = "Agent")]
async fn update_episode(
    Path((_agent_id, episode_id)): Path<(String, String)>,
    Json(payload): Json<UpdateAgentEpisode>,
) -> Result<Json<AgentEpisode>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let episode = service.update_episode(&episode_id, payload).await?;
    Ok(Json(episode))
}

// ============================================================================
// Behavior Profile Handlers
// ============================================================================

#[utoipa::path(post, path = "/api/v1/agents/{agent_id}/behaviors", tag = "Agent")]
async fn record_behavior(
    Path(agent_id): Path<String>,
    Json(payload): Json<CreateAgentBehaviorProfile>,
) -> Result<Json<AgentBehaviorProfile>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let profile = service.record_behavior(&agent_id, payload).await?;
    Ok(Json(profile))
}

#[utoipa::path(get, path = "/api/v1/agents/{agent_id}/behaviors", tag = "Agent")]
async fn list_behaviors(
    Path(agent_id): Path<String>,
) -> Result<Json<Vec<AgentBehaviorProfile>>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let profiles = service.list_behaviors(&agent_id).await?;
    Ok(Json(profiles))
}

// ============================================================================
// Complete Agent Info
// ============================================================================

#[utoipa::path(get, path = "/api/v1/agents/{agent_id}/complete", tag = "Agent")]
async fn get_agent_complete(
    Path(agent_id): Path<String>,
) -> Result<Json<AgentWithSelfModel>, AppError> {
    let pool = db::pool();
    let service = AgentService::new(pool.clone());

    let agent = service
        .get_agent_with_self_model(&agent_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Agent {} not found", agent_id)))?;

    Ok(Json(agent))
}
