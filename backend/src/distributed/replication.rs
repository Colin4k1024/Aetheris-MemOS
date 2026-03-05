//! Replication Manager
//!
//! This module provides replication management for distributed memory.

use serde::{Deserialize, Serialize};
use crate::distributed::node::NodeId;

/// Replication state for a memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplicaState {
    Primary,
    Replica,
    Syncing,
    Outdated,
}

/// Replication configuration.
#[derive(Debug, Clone)]
pub struct ReplicationConfig {
    pub replication_factor: usize,
    pub consistency_level: ConsistencyLevel,
    pub sync_timeout_ms: u64,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            replication_factor: 3,
            consistency_level: ConsistencyLevel::Eventual,
            sync_timeout_ms: 5000,
        }
    }
}

/// Consistency level for replication.
#[derive(Debug, Clone, Copy)]
pub enum ConsistencyLevel {
    Strong,
    Sequential,
    Eventual,
}

/// Replication manager.
pub struct ReplicationManager {
    config: ReplicationConfig,
    replicas: std::sync::RwLock<std::collections::HashMap<String, Vec<ReplicaInfo>>>,
}

#[derive(Debug, Clone)]
pub struct ReplicaInfo {
    pub node_id: NodeId,
    pub state: ReplicaState,
    pub offset: u64,
}

impl ReplicationManager {
    pub fn new(config: ReplicationConfig) -> Self {
        Self {
            config,
            replicas: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Get replica nodes for a memory entry.
    pub fn get_replicas(&self, memory_id: &str) -> Vec<NodeId> {
        let replicas = self.replicas.read().unwrap();
        replicas
            .get(memory_id)
            .map(|r| r.iter().map(|ri| ri.node_id.clone()).collect())
            .unwrap_or_default()
    }

    /// Add replica for a memory entry.
    pub fn add_replica(&self, memory_id: &str, node_id: NodeId) {
        let mut replicas = self.replicas.write().unwrap();
        let entry = replicas.entry(memory_id.to_string()).or_insert_with(Vec::new);
        entry.push(ReplicaInfo {
            node_id,
            state: ReplicaState::Syncing,
            offset: 0,
        });
    }

    /// Remove replica for a memory entry.
    pub fn remove_replica(&self, memory_id: &str, node_id: &NodeId) {
        let mut replicas = self.replicas.write().unwrap();
        if let Some(entry) = replicas.get_mut(memory_id) {
            entry.retain(|r| &r.node_id != node_id);
        }
    }
}
