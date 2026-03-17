//! Enterprise Router
//!
//! API endpoints for enterprise cloud platform features.

use axum::extract::Path;
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::info;
use validator::Validate;

use crate::services::enterprise::{ClusterManager, ClusterNode, EnterpriseShardManager, ShardConfig};
use crate::{json_ok, JsonResult};

// Global cluster manager
static CLUSTER_MANAGER: std::sync::OnceLock<ClusterManager> =
    std::sync::OnceLock::new();

fn get_cluster_manager() -> &'static ClusterManager {
    CLUSTER_MANAGER.get_or_init(|| ClusterManager::new("node_1"))
}

// Global shard manager
static SHARD_MANAGER: std::sync::OnceLock<EnterpriseShardManager> =
    std::sync::OnceLock::new();

fn get_shard_manager() -> &'static EnterpriseShardManager {
    SHARD_MANAGER.get_or_init(EnterpriseShardManager::new)
}

/// Register node request
#[derive(Deserialize, Serialize, Validate)]
pub struct RegisterNodeRequest {
    #[serde(rename = "nodeId")]
    pub node_id: String,
    pub host: String,
    pub port: u16,
}

/// Create shard request
#[derive(Deserialize, Serialize, Validate)]
pub struct CreateShardRequest {
    #[serde(rename = "shardId")]
    pub shard_id: u32,
    #[serde(rename = "primaryNode")]
    pub primary_node: String,
    #[serde(rename = "replicaNodes")]
    pub replica_nodes: Vec<String>,
    #[serde(rename = "keyRangeStart")]
    pub key_range_start: u64,
    #[serde(rename = "keyRangeEnd")]
    pub key_range_end: u64,
}

/// Register a node in the cluster
pub async fn register_node(
    Json(req): Json<RegisterNodeRequest>,
) -> JsonResult<ClusterNode> {
    req.validate()?;
    info!("Registering node {} in cluster", req.node_id);

    let node = get_cluster_manager()
        .register_node(&req.node_id, &req.host, req.port)
        .await;

    json_ok(node)
}

/// Get cluster nodes
pub async fn get_cluster_nodes() -> JsonResult<Vec<ClusterNode>> {
    info!("Getting cluster nodes");

    let nodes = get_cluster_manager().get_nodes().await;

    json_ok(nodes)
}

/// Get active nodes
pub async fn get_active_nodes() -> JsonResult<Vec<ClusterNode>> {
    info!("Getting active cluster nodes");

    let nodes = get_cluster_manager().get_active_nodes().await;

    json_ok(nodes)
}

/// Get current leader
pub async fn get_leader() -> JsonResult<Option<ClusterNode>> {
    info!("Getting cluster leader");

    let leader = get_cluster_manager().get_leader().await;

    json_ok(leader)
}

/// Become leader
pub async fn become_leader() -> JsonResult<ClusterNode> {
    info!("Requesting leadership");

    let leader = get_cluster_manager().become_leader().await?;

    json_ok(leader)
}

/// Check if current node is leader
pub async fn is_leader() -> JsonResult<serde_json::Value> {
    let is_leader = get_cluster_manager().is_leader().await;

    json_ok(serde_json::json!({
        "is_leader": is_leader
    }))
}

/// Create a shard
pub async fn create_shard(
    Json(req): Json<CreateShardRequest>,
) -> JsonResult<ShardConfig> {
    req.validate()?;
    info!("Creating shard {}", req.shard_id);

    let shard = get_shard_manager()
        .create_shard(
            req.shard_id,
            &req.primary_node,
            req.replica_nodes,
            req.key_range_start,
            req.key_range_end,
        )
        .await;

    json_ok(shard)
}

/// Get shard for key
pub async fn get_shard(
    Path(key): Path<String>,
) -> JsonResult<Option<ShardConfig>> {
    info!("Getting shard for key: {}", key);

    let shard = get_shard_manager().get_shard_for_key(&key).await;

    json_ok(shard)
}

/// Get all shards
pub async fn get_shards() -> JsonResult<Vec<ShardConfig>> {
    info!("Getting all shards");

    let shards = get_shard_manager().get_shards().await;

    json_ok(shards)
}
