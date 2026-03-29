//! Enterprise Cloud Platform - Distributed System Support
//!
//! This module provides enterprise-grade distributed system capabilities:
//! - High availability election
//! - Data sharding
//! - Distributed configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use ulid::Ulid;

/// Node role in the cluster
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterNodeRole {
    Leader,
    Follower,
    Candidate,
}

/// Cluster node status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Active,
    Inactive,
    Recovering,
}

/// Cluster node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    pub node_id: String,
    pub host: String,
    pub port: u16,
    pub role: ClusterNodeRole,
    pub status: NodeStatus,
    pub leader_id: Option<String>,
    pub term: u64,
    pub last_heartbeat: i64,
    pub added_at: i64,
}

/// Distributed cluster manager
pub struct ClusterManager {
    nodes: Arc<RwLock<HashMap<String, ClusterNode>>>,
    current_node_id: String,
    leader_id: Arc<RwLock<Option<String>>>,
}

impl ClusterManager {
    /// Create a new cluster manager
    pub fn new(node_id: &str) -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            current_node_id: node_id.to_string(),
            leader_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Register a node in the cluster
    pub async fn register_node(&self, node_id: &str, host: &str, port: u16) -> ClusterNode {
        let node = ClusterNode {
            node_id: node_id.to_string(),
            host: host.to_string(),
            port,
            role: ClusterNodeRole::Follower,
            status: NodeStatus::Active,
            leader_id: None,
            term: 0,
            last_heartbeat: chrono::Utc::now().timestamp(),
            added_at: chrono::Utc::now().timestamp(),
        };

        self.nodes
            .write()
            .await
            .insert(node_id.to_string(), node.clone());

        info!("Registered node {} in cluster", node_id);
        node
    }

    /// Get cluster nodes
    pub async fn get_nodes(&self) -> Vec<ClusterNode> {
        let nodes = self.nodes.read().await;
        nodes.values().cloned().collect()
    }

    /// Get active nodes
    pub async fn get_active_nodes(&self) -> Vec<ClusterNode> {
        let nodes = self.nodes.read().await;
        nodes
            .values()
            .filter(|n| n.status == NodeStatus::Active)
            .cloned()
            .collect()
    }

    /// Get current leader
    pub async fn get_leader(&self) -> Option<ClusterNode> {
        let leader_id = self.leader_id.read().await;
        if let Some(ref id) = *leader_id {
            let nodes = self.nodes.read().await;
            nodes.get(id).cloned()
        } else {
            None
        }
    }

    /// Become leader (simplified election)
    pub async fn become_leader(&self) -> Result<ClusterNode, crate::AppError> {
        let mut nodes = self.nodes.write().await;
        let mut leader = nodes
            .get_mut(&self.current_node_id)
            .ok_or_else(|| crate::AppError::NotFound("Current node not registered".to_string()))?;

        leader.role = ClusterNodeRole::Leader;
        leader.term += 1;
        leader.last_heartbeat = chrono::Utc::now().timestamp();

        // Update all followers
        for node in nodes.values_mut() {
            if node.node_id != self.current_node_id {
                node.role = ClusterNodeRole::Follower;
                node.leader_id = Some(self.current_node_id.clone());
            }
        }

        drop(nodes);

        // Update leader id
        *self.leader_id.write().await = Some(self.current_node_id.clone());

        let nodes = self.nodes.read().await;
        let result = nodes.get(&self.current_node_id).cloned();

        info!("Node {} became leader", self.current_node_id);
        result.ok_or_else(|| crate::AppError::Internal("Failed to get leader".to_string()))
    }

    /// Send heartbeat to leader
    pub async fn send_heartbeat(&self) -> Result<(), crate::AppError> {
        let leader_id = self.leader_id.read().await;
        if let Some(ref id) = *leader_id {
            let mut nodes = self.nodes.write().await;
            if let Some(node) = nodes.get_mut(id) {
                node.last_heartbeat = chrono::Utc::now().timestamp();
                return Ok(());
            }
        }
        Err(crate::AppError::Internal("No leader available".to_string()))
    }

    /// Check if current node is leader
    pub async fn is_leader(&self) -> bool {
        let leader_id = self.leader_id.read().await;
        if let Some(ref id) = *leader_id {
            id == &self.current_node_id
        } else {
            false
        }
    }

    /// Remove dead nodes
    pub async fn remove_dead_nodes(&self, timeout_seconds: i64) -> Vec<String> {
        let now = chrono::Utc::now().timestamp();
        let mut nodes = self.nodes.write().await;
        let mut removed = Vec::new();

        nodes.retain(|id, node| {
            if node.status == NodeStatus::Active && now - node.last_heartbeat > timeout_seconds {
                removed.push(id.clone());
                false
            } else {
                true
            }
        });

        if !removed.is_empty() {
            info!("Removed dead nodes: {:?}", removed);
        }

        removed
    }
}

/// Data sharding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardConfig {
    pub shard_id: u32,
    pub primary_node: String,
    pub replica_nodes: Vec<String>,
    pub key_range_start: u64,
    pub key_range_end: u64,
}

/// Shard manager
pub struct EnterpriseShardManager {
    shards: Arc<RwLock<HashMap<u32, ShardConfig>>>,
}

impl EnterpriseShardManager {
    /// Create a new shard manager
    pub fn new() -> Self {
        Self {
            shards: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a shard
    pub async fn create_shard(
        &self,
        shard_id: u32,
        primary_node: &str,
        replica_nodes: Vec<String>,
        key_range_start: u64,
        key_range_end: u64,
    ) -> ShardConfig {
        let config = ShardConfig {
            shard_id,
            primary_node: primary_node.to_string(),
            replica_nodes,
            key_range_start,
            key_range_end,
        };

        self.shards.write().await.insert(shard_id, config.clone());

        info!("Created shard {} with primary {}", shard_id, primary_node);
        config
    }

    /// Get shard for a key
    pub async fn get_shard_for_key(&self, key: &str) -> Option<ShardConfig> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish() % 100;

        let shards = self.shards.read().await;
        for shard in shards.values() {
            if hash >= shard.key_range_start && hash < shard.key_range_end {
                return Some(shard.clone());
            }
        }

        // Default to first shard
        shards.values().next().cloned()
    }

    /// Get all shards
    pub async fn get_shards(&self) -> Vec<ShardConfig> {
        let shards = self.shards.read().await;
        shards.values().cloned().collect()
    }
}

impl Default for EnterpriseShardManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_node() {
        let manager = ClusterManager::new("node_1");
        let node = manager.register_node("node_2", "localhost", 8080).await;
        assert_eq!(node.node_id, "node_2");
    }

    #[tokio::test]
    async fn test_become_leader() {
        let manager = ClusterManager::new("node_1");
        manager.register_node("node_1", "localhost", 8080).await;

        let leader = manager.become_leader().await.unwrap();
        assert_eq!(leader.role, ClusterNodeRole::Leader);
    }

    #[tokio::test]
    async fn test_shard_manager() {
        let manager = EnterpriseShardManager::new();
        manager
            .create_shard(0, "node_1", vec!["node_2".to_string()], 0, 50)
            .await;

        let shard = manager.get_shard_for_key("key_1").await;
        assert!(shard.is_some());
    }
}
