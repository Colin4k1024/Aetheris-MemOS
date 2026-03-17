//! Memory Pool Router
//!
//! API endpoints for multi-agent collaborative memory pool.

use axum::extract::Path;
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::info;
use validator::Validate;

use crate::services::memory_pool::{
    MemoryPool, MemoryVisibility, NetworkAgent, NetworkStatus, ShareRequest,
};
use crate::{json_ok, JsonResult};

// Global memory pool instance
static MEMORY_POOL: std::sync::OnceLock<MemoryPool> = std::sync::OnceLock::new();

fn get_memory_pool() -> &'static MemoryPool {
    MEMORY_POOL.get_or_init(MemoryPool::new)
}

/// Register agent request
#[derive(Deserialize, Serialize, Validate)]
pub struct RegisterAgentRequest {
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub name: String,
}

/// Share memory request
#[derive(Deserialize, Serialize, Validate)]
pub struct ShareMemoryRequest {
    #[serde(rename = "memoryId")]
    pub memory_id: String,
    #[serde(rename = "targetAgentIds")]
    pub target_agent_ids: Vec<String>,
    pub visibility: MemoryVisibility,
}

/// Add correlation request
#[derive(Deserialize, Serialize, Validate)]
pub struct AddCorrelationRequest {
    #[serde(rename = "sourceMemoryId")]
    pub source_memory_id: String,
    #[serde(rename = "targetMemoryId")]
    pub target_memory_id: String,
    #[serde(rename = "targetAgentId")]
    pub target_agent_id: String,
    #[serde(rename = "correlationType")]
    pub correlation_type: String,
    pub strength: f64,
}

/// Register agent in network
pub async fn register_agent(
    Json(req): Json<RegisterAgentRequest>,
) -> JsonResult<NetworkAgent> {
    req.validate()?;
    info!("Registering agent {} in memory pool", req.agent_id);

    let agent = get_memory_pool()
        .register_agent(&req.agent_id, &req.name)
        .await;

    json_ok(agent)
}

/// Unregister agent
pub async fn unregister_agent(
    Path(agent_id): Path<String>,
) -> JsonResult<serde_json::Value> {
    info!("Unregistering agent {} from memory pool", agent_id);

    let result = get_memory_pool().unregister_agent(&agent_id).await;

    json_ok(serde_json::json!({
        "success": result,
        "agent_id": agent_id
    }))
}

/// Share memory
pub async fn share_memory(
    Path(owner_agent_id): Path<String>,
    Json(req): Json<ShareMemoryRequest>,
) -> JsonResult<crate::services::memory_pool::SharedMemory> {
    req.validate()?;
    info!(
        "Sharing memory {} from agent {}",
        req.memory_id, owner_agent_id
    );

    let share_request = ShareRequest {
        memory_id: req.memory_id,
        target_agent_ids: req.target_agent_ids,
        visibility: req.visibility,
    };

    let result = get_memory_pool()
        .share_memory(&owner_agent_id, share_request)
        .await?;

    json_ok(result)
}

/// Revoke memory sharing
pub async fn revoke_memory(
    Path((owner_agent_id, memory_id)): Path<(String, String)>,
) -> JsonResult<serde_json::Value> {
    info!(
        "Revoking memory {} from agent {}",
        memory_id, owner_agent_id
    );

    get_memory_pool()
        .revoke_memory(&owner_agent_id, &memory_id)
        .await?;

    json_ok(serde_json::json!({
        "success": true,
        "memory_id": memory_id
    }))
}

/// Get visible memories for agent
pub async fn get_visible_memories(
    Path(agent_id): Path<String>,
) -> JsonResult<Vec<crate::services::memory_pool::SharedMemory>> {
    info!("Getting visible memories for agent {}", agent_id);

    let memories = get_memory_pool()
        .get_visible_memories(&agent_id)
        .await?;

    json_ok(memories)
}

/// Add memory correlation
pub async fn add_correlation(
    Json(req): Json<AddCorrelationRequest>,
) -> JsonResult<crate::services::memory_pool::MemoryCorrelation> {
    req.validate()?;
    info!(
        "Adding correlation from {} to {}",
        req.source_memory_id, req.target_memory_id
    );

    let result = get_memory_pool()
        .add_correlation(
            &req.source_memory_id,
            &req.target_memory_id,
            &req.target_agent_id,
            &req.correlation_type,
            req.strength,
        )
        .await?;

    json_ok(result)
}

/// Get correlated memories
pub async fn get_correlations(
    Path(memory_id): Path<String>,
) -> JsonResult<Vec<crate::services::memory_pool::MemoryCorrelation>> {
    info!("Getting correlations for memory {}", memory_id);

    let correlations = get_memory_pool()
        .find_correlated_memories(&memory_id)
        .await?;

    json_ok(correlations)
}

/// Get network status
pub async fn get_network_status() -> JsonResult<NetworkStatus> {
    info!("Getting memory pool network status");

    let status = get_memory_pool().get_network_status().await;

    json_ok(status)
}

/// List agents in network
pub async fn list_agents() -> JsonResult<Vec<NetworkAgent>> {
    info!("Listing agents in memory pool network");

    let agents = get_memory_pool().list_agents().await;

    json_ok(agents)
}
