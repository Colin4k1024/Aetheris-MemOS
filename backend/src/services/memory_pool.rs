//! Multi-agent Collaborative Memory Pool
//!
//! This module provides distributed agent memory sharing capabilities.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use ulid::Ulid;

/// Memory visibility level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryVisibility {
    Private,
    Shared,
    Public,
}

/// Agent in the memory network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkAgent {
    pub agent_id: String,
    pub name: String,
    pub status: AgentStatus,
    pub visible_memories: HashSet<String>, // Memory IDs
    pub shared_with: HashSet<String>,      // Agent IDs
    pub connected_at: i64,
}

/// Agent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Online,
    Offline,
    Busy,
}

/// Shared memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedMemory {
    pub memory_id: String,
    pub owner_agent_id: String,
    pub visibility: MemoryVisibility,
    pub shared_with: HashSet<String>,
    pub sync_status: SyncStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Sync status for shared memories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Synced,
    Pending,
    Conflict,
}

/// Memory share request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareRequest {
    pub memory_id: String,
    pub target_agent_ids: Vec<String>,
    pub visibility: MemoryVisibility,
}

/// Memory correlation (for graph-based retrieval)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCorrelation {
    pub correlation_id: String,
    pub source_memory_id: String,
    pub target_memory_id: String,
    pub target_agent_id: String,
    pub correlation_type: String,
    pub strength: f64,
}

/// Multi-agent memory pool service
pub struct MemoryPool {
    agents: Arc<RwLock<HashMap<String, NetworkAgent>>>,
    shared_memories: Arc<RwLock<HashMap<String, SharedMemory>>>,
    correlations: Arc<RwLock<HashMap<String, Vec<MemoryCorrelation>>>>,
}

impl MemoryPool {
    /// Create a new memory pool
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            shared_memories: Arc::new(RwLock::new(HashMap::new())),
            correlations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an agent in the network
    pub async fn register_agent(&self, agent_id: &str, name: &str) -> NetworkAgent {
        let agent = NetworkAgent {
            agent_id: agent_id.to_string(),
            name: name.to_string(),
            status: AgentStatus::Online,
            visible_memories: HashSet::new(),
            shared_with: HashSet::new(),
            connected_at: chrono::Utc::now().timestamp(),
        };

        self.agents
            .write()
            .await
            .insert(agent_id.to_string(), agent.clone());

        info!("Registered agent {} in memory pool", agent_id);
        agent
    }

    /// Unregister an agent
    pub async fn unregister_agent(&self, agent_id: &str) -> bool {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.remove(agent_id) {
            // Remove shared memories owned by this agent
            let mut shared = self.shared_memories.write().await;
            shared.retain(|_, m| m.owner_agent_id != agent_id);
            info!("Unregistered agent {} from memory pool", agent_id);
            true
        } else {
            false
        }
    }

    /// Share memory with agents
    pub async fn share_memory(
        &self,
        owner_agent_id: &str,
        request: ShareRequest,
    ) -> Result<SharedMemory, crate::AppError> {
        // Verify owner exists
        let agents = self.agents.read().await;
        if !agents.contains_key(owner_agent_id) {
            return Err(crate::AppError::NotFound(format!(
                "Agent {} not found",
                owner_agent_id
            )));
        }
        drop(agents);

        let now = chrono::Utc::now().timestamp();
        let shared = SharedMemory {
            memory_id: request.memory_id.clone(),
            owner_agent_id: owner_agent_id.to_string(),
            visibility: request.visibility,
            shared_with: request.target_agent_ids.into_iter().collect(),
            sync_status: SyncStatus::Pending,
            created_at: now,
            updated_at: now,
        };

        // Store shared memory
        self.shared_memories
            .write()
            .await
            .insert(request.memory_id.clone(), shared.clone());

        // Update agent's visible memories
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(owner_agent_id) {
            agent.visible_memories.insert(request.memory_id.clone());
        }

        // Update target agents' shared_with
        for target_id in &shared.shared_with {
            if let Some(agent) = agents.get_mut(target_id) {
                agent.shared_with.insert(owner_agent_id.to_string());
            }
        }

        info!(
            "Shared memory {} with {} agents",
            request.memory_id,
            shared.shared_with.len()
        );
        Ok(shared)
    }

    /// Revoke memory sharing
    pub async fn revoke_memory(
        &self,
        owner_agent_id: &str,
        memory_id: &str,
    ) -> Result<(), crate::AppError> {
        let mut shared = self.shared_memories.write().await;

        if let Some(memory) = shared.get_mut(memory_id) {
            if memory.owner_agent_id != owner_agent_id {
                return Err(crate::AppError::Unauthorized(
                    "Not the owner of this memory".to_string(),
                ));
            }

            // Update target agents
            let targets: Vec<String> = memory.shared_with.drain().collect();
            drop(shared);

            let mut agents = self.agents.write().await;
            for target_id in targets {
                if let Some(agent) = agents.get_mut(&target_id) {
                    agent.shared_with.remove(owner_agent_id);
                }
            }

            info!("Revoked sharing of memory {}", memory_id);
            Ok(())
        } else {
            Err(crate::AppError::NotFound(format!(
                "Memory {} not found",
                memory_id
            )))
        }
    }

    /// Get memories visible to an agent
    pub async fn get_visible_memories(
        &self,
        agent_id: &str,
    ) -> Result<Vec<SharedMemory>, crate::AppError> {
        let agents = self.agents.read().await;
        let agent = agents
            .get(agent_id)
            .ok_or_else(|| crate::AppError::NotFound(format!("Agent {} not found", agent_id)))?;

        let shared = self.shared_memories.read().await;
        let mut result = Vec::new();

        // Get memories owned by this agent
        for memory_id in &agent.visible_memories {
            if let Some(memory) = shared.get(memory_id) {
                result.push(memory.clone());
            }
        }

        // Get memories shared with this agent
        for owner_id in &agent.shared_with {
            for (memory_id, memory) in shared.iter() {
                if memory.owner_agent_id == *owner_id
                    && (memory.visibility == MemoryVisibility::Public
                        || memory.shared_with.contains(agent_id))
                {
                    result.push(memory.clone());
                }
            }
        }

        Ok(result)
    }

    /// Add correlation between memories (cross-agent entity linking)
    pub async fn add_correlation(
        &self,
        source_memory_id: &str,
        target_memory_id: &str,
        target_agent_id: &str,
        correlation_type: &str,
        strength: f64,
    ) -> Result<MemoryCorrelation, crate::AppError> {
        let correlation = MemoryCorrelation {
            correlation_id: Ulid::new().to_string(),
            source_memory_id: source_memory_id.to_string(),
            target_memory_id: target_memory_id.to_string(),
            target_agent_id: target_agent_id.to_string(),
            correlation_type: correlation_type.to_string(),
            strength,
        };

        let mut correlations = self.correlations.write().await;
        correlations
            .entry(source_memory_id.to_string())
            .or_default()
            .push(correlation.clone());

        info!(
            "Added correlation from {} to {} (agent: {})",
            source_memory_id, target_memory_id, target_agent_id
        );
        Ok(correlation)
    }

    /// Find correlated memories (shared knowledge discovery)
    pub async fn find_correlated_memories(
        &self,
        memory_id: &str,
    ) -> Result<Vec<MemoryCorrelation>, crate::AppError> {
        let correlations = self.correlations.read().await;
        Ok(correlations.get(memory_id).cloned().unwrap_or_default())
    }

    /// Get network status
    pub async fn get_network_status(&self) -> NetworkStatus {
        let agents = self.agents.read().await;
        let shared = self.shared_memories.read().await;

        NetworkStatus {
            total_agents: agents.len(),
            online_agents: agents
                .values()
                .filter(|a| a.status == AgentStatus::Online)
                .count(),
            total_shared_memories: shared.len(),
            public_memories: shared
                .values()
                .filter(|m| m.visibility == MemoryVisibility::Public)
                .count(),
        }
    }

    /// List all agents in network
    pub async fn list_agents(&self) -> Vec<NetworkAgent> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }
}

impl Default for MemoryPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Network status response
#[derive(Serialize)]
pub struct NetworkStatus {
    pub total_agents: usize,
    pub online_agents: usize,
    pub total_shared_memories: usize,
    pub public_memories: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_agent() {
        let pool = MemoryPool::new();
        let agent = pool.register_agent("agent_1", "Test Agent").await;
        assert_eq!(agent.agent_id, "agent_1");
        assert_eq!(agent.status, AgentStatus::Online);
    }

    #[tokio::test]
    async fn test_share_memory() {
        let pool = MemoryPool::new();
        pool.register_agent("agent_1", "Owner").await;
        pool.register_agent("agent_2", "Target").await;

        let request = ShareRequest {
            memory_id: "memory_1".to_string(),
            target_agent_ids: vec!["agent_2".to_string()],
            visibility: MemoryVisibility::Shared,
        };

        let result = pool.share_memory("agent_1", request).await.unwrap();
        assert_eq!(result.owner_agent_id, "agent_1");
    }

    #[tokio::test]
    async fn test_get_visible_memories() {
        let pool = MemoryPool::new();
        pool.register_agent("agent_1", "Owner").await;
        pool.register_agent("agent_2", "Target").await;

        // Share memory
        let request = ShareRequest {
            memory_id: "memory_1".to_string(),
            target_agent_ids: vec!["agent_2".to_string()],
            visibility: MemoryVisibility::Shared,
        };
        pool.share_memory("agent_1", request).await.unwrap();

        // Get visible memories for agent_2
        let memories = pool.get_visible_memories("agent_2").await.unwrap();
        assert!(!memories.is_empty());
    }
}
