//! Memory Node
//!
//! This module provides the memory node abstraction for distributed systems.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Node identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new() -> Self {
        Self(ulid::Ulid::new().to_string())
    }

    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Node role in the cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    Leader,
    Follower,
    Candidate,
    Learner,
}

/// Node information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub role: NodeRole,
    pub address: String,
    pub port: u16,
    pub is_healthy: bool,
    pub last_heartbeat: i64,
    pub resources: NodeResources,
}

/// Node resources.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeResources {
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub storage_mb: u64,
}

/// Memory node in the distributed system.
pub struct MemoryNode {
    id: NodeId,
    info: RwLock<NodeInfo>,
    peers: RwLock<HashMap<NodeId, NodeInfo>>,
}

impl MemoryNode {
    pub fn new(id: NodeId, address: String, port: u16) -> Self {
        let info = NodeInfo {
            id: id.clone(),
            role: NodeRole::Follower,
            address,
            port,
            is_healthy: true,
            last_heartbeat: chrono::Utc::now().timestamp(),
            resources: NodeResources::default(),
        };

        Self {
            id,
            info: RwLock::new(info),
            peers: RwLock::new(HashMap::new()),
        }
    }

    pub fn id(&self) -> &NodeId {
        &self.id
    }

    pub async fn info(&self) -> NodeInfo {
        self.info.read().await.clone()
    }

    pub async fn set_role(&self, role: NodeRole) {
        let mut info = self.info.write().await;
        info.role = role;
        info.last_heartbeat = chrono::Utc::now().timestamp();
    }

    pub async fn add_peer(&self, peer: NodeInfo) {
        let mut peers = self.peers.write().await;
        peers.insert(peer.id.clone(), peer);
    }

    pub async fn remove_peer(&self, node_id: &NodeId) {
        let mut peers = self.peers.write().await;
        peers.remove(node_id);
    }

    pub async fn get_peers(&self) -> Vec<NodeInfo> {
        let peers = self.peers.read().await;
        peers.values().cloned().collect()
    }

    pub async fn leader(&self) -> Option<NodeInfo> {
        let peers = self.peers.read().await;
        peers.values().find(|p| p.role == NodeRole::Leader).cloned()
    }

    pub async fn is_leader(&self) -> bool {
        let info = self.info.read().await;
        info.role == NodeRole::Leader
    }

    pub async fn heartbeat(&self) {
        let mut info = self.info.write().await;
        info.last_heartbeat = chrono::Utc::now().timestamp();
    }
}
