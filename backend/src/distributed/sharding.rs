//! Sharding Manager
//!
//! This module provides sharding for distributed memory.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::distributed::node::NodeId;

/// Shard key for memory distribution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShardKey {
    pub memory_id: String,
    pub tenant_id: Option<String>,
}

impl ShardKey {
    pub fn new(memory_id: impl Into<String>) -> Self {
        Self {
            memory_id: memory_id.into(),
            tenant_id: None,
        }
    }

    pub fn with_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }
}

/// Shard placement information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardPlacement {
    pub shard_id: u32,
    pub primary_node: NodeId,
    pub replica_nodes: Vec<NodeId>,
}

/// Shard manager for distributing memory across nodes.
pub struct ShardManager {
    shard_count: u32,
    placements: HashMap<u32, ShardPlacement>,
}

impl ShardManager {
    pub fn new(shard_count: u32) -> Self {
        Self {
            shard_count,
            placements: HashMap::new(),
        }
    }

    /// Get shard ID for a key.
    pub fn get_shard(&self, key: &ShardKey) -> u32 {
        // Simple hash-based sharding
        let hash = key.memory_id.hash();
        (hash % self.shard_count as u64) as u32
    }

    /// Get shard placement.
    pub fn get_placement(&self, shard_id: u32) -> Option<&ShardPlacement> {
        self.placements.get(&shard_id)
    }

    /// Set shard placement.
    pub fn set_placement(&mut self, placement: ShardPlacement) {
        self.placements.insert(placement.shard_id, placement);
    }

    /// Get primary node for a key.
    pub fn get_primary_node(&self, key: &ShardKey) -> Option<NodeId> {
        let shard_id = self.get_shard(key);
        self.placements.get(&shard_id).map(|p| p.primary_node.clone())
    }

    /// Get all replica nodes for a key.
    pub fn get_replica_nodes(&self, key: &ShardKey) -> Vec<NodeId> {
        let shard_id = self.get_shard(key);
        self.placements.get(&shard_id)
            .map(|p| p.replica_nodes.clone())
            .unwrap_or_default()
    }
}
