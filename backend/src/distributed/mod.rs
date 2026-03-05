//! Distributed Memory Node
//!
//! This module provides distributed memory node capabilities:
//! - Node abstraction
//! - Replication
//! - Sharding
//! - Consensus

pub mod node;
pub mod replication;
pub mod sharding;
pub mod consensus;

pub use node::{MemoryNode, NodeId, NodeInfo, NodeRole};
pub use replication::{ReplicationManager, ReplicaState, ReplicationConfig};
pub use sharding::{ShardManager, ShardKey, ShardPlacement};
pub use consensus::ConsensusModule;

/// Distributed system configuration.
#[derive(Debug, Clone)]
pub struct DistributedConfig {
    pub node_id: NodeId,
    pub cluster_id: String,
    pub replication_factor: usize,
    pub shard_count: usize,
    pub consensus_method: ConsensusMethod,
}

impl Default for DistributedConfig {
    fn default() -> Self {
        Self {
            node_id: NodeId::new(),
            cluster_id: "default".to_string(),
            replication_factor: 3,
            shard_count: 16,
            consensus_method: ConsensusMethod::Raft,
        }
    }
}

/// Consensus method for distributed coordination.
#[derive(Debug, Clone, Copy)]
pub enum ConsensusMethod {
    Raft,
    Paxos,
    MultiPaxos,
}
