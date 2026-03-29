//! Oris Execution Engine Integration
//!
//! This module provides integration with Oris persistent task runtime for
//! durable agent task state management.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use ulid::Ulid;

use crate::kernel::types::{MemoryEntry, MemoryId};

/// Memory access record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAccess {
    pub memory_id: String,
    pub access_type: String,
    pub timestamp: i64,
}

/// Oris task state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrisTaskState {
    pub task_id: String,
    pub agent_id: String,
    pub status: OrisTaskStatus,
    pub context_snapshot: ContextSnapshot,
    pub checkpoints: Vec<Checkpoint>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Oris task status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OrisTaskStatus {
    Running,
    Suspended,
    Completed,
    Failed,
    RolledBack,
}

/// Context snapshot for task suspension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub snapshot_id: String,
    pub task_id: String,
    pub memory_state: MemoryState,
    pub decision_state: DecisionState,
    pub working_memory: Vec<MemoryId>,
    pub metadata: HashMap<String, String>,
    pub created_at: i64,
}

/// Memory state at snapshot time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryState {
    pub stm_entries: Vec<MemoryEntry>,
    pub active_context: Vec<MemoryId>,
    pub recent_accesses: Vec<MemoryAccess>,
}

/// Decision state at snapshot time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionState {
    pub decision_history: Vec<DecisionRecord>,
    pub pending_decisions: Vec<PendingDecision>,
    pub checkpoint_id: Option<String>,
}

/// Decision record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub decision_id: String,
    pub decision_type: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub confidence: f64,
    pub timestamp: i64,
}

/// Pending decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDecision {
    pub decision_id: String,
    pub decision_type: String,
    pub context: serde_json::Value,
    pub waiting_on: Option<String>,
    pub created_at: i64,
}

/// Checkpoint for rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub checkpoint_id: String,
    pub task_id: String,
    pub snapshot: ContextSnapshot,
    pub memory_snapshot: Vec<MemoryEntry>,
    pub decision_snapshot: Vec<DecisionRecord>,
    pub created_at: i64,
    pub description: Option<String>,
}

/// Oris integration service
pub struct OrisIntegration {
    tasks: Arc<RwLock<HashMap<String, OrisTaskState>>>,
    snapshots: Arc<RwLock<HashMap<String, ContextSnapshot>>>,
    checkpoints: Arc<RwLock<HashMap<String, Checkpoint>>>,
}

impl OrisIntegration {
    /// Create a new Oris integration service
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new task state
    pub async fn create_task(&self, agent_id: &str) -> Result<OrisTaskState, crate::AppError> {
        let task_id = Ulid::new().to_string();
        let now = chrono::Utc::now().timestamp();

        let task = OrisTaskState {
            task_id: task_id.clone(),
            agent_id: agent_id.to_string(),
            status: OrisTaskStatus::Running,
            context_snapshot: ContextSnapshot {
                snapshot_id: Ulid::new().to_string(),
                task_id: task_id.clone(),
                memory_state: MemoryState {
                    stm_entries: Vec::new(),
                    active_context: Vec::new(),
                    recent_accesses: Vec::new(),
                },
                decision_state: DecisionState {
                    decision_history: Vec::new(),
                    pending_decisions: Vec::new(),
                    checkpoint_id: None,
                },
                working_memory: Vec::new(),
                metadata: HashMap::new(),
                created_at: now,
            },
            checkpoints: Vec::new(),
            created_at: now,
            updated_at: now,
        };

        self.tasks
            .write()
            .await
            .insert(task_id.clone(), task.clone());
        info!("Created Oris task: {}", task_id);

        Ok(task)
    }

    /// Suspend task and save context snapshot
    pub async fn suspend_task(
        &self,
        task_id: &str,
        context: ContextSnapshot,
    ) -> Result<ContextSnapshot, crate::AppError> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| crate::AppError::NotFound(format!("Task {} not found", task_id)))?;

        task.status = OrisTaskStatus::Suspended;
        task.context_snapshot = context.clone();
        task.updated_at = chrono::Utc::now().timestamp();

        // Store snapshot
        self.snapshots
            .write()
            .await
            .insert(context.snapshot_id.clone(), context.clone());
        info!(
            "Suspended task {} with snapshot {}",
            task_id, context.snapshot_id
        );

        Ok(context)
    }

    /// Resume task and restore context
    pub async fn resume_task(
        &self,
        task_id: &str,
        snapshot_id: Option<String>,
    ) -> Result<OrisTaskState, crate::AppError> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| crate::AppError::NotFound(format!("Task {} not found", task_id)))?;

        if task.status != OrisTaskStatus::Suspended {
            return Err(crate::AppError::BadRequest(
                "Task is not in suspended state".to_string(),
            ));
        }

        // Restore from snapshot if provided
        if let Some(sid) = snapshot_id {
            let snapshots = self.snapshots.read().await;
            if let Some(snapshot) = snapshots.get(&sid) {
                task.context_snapshot = snapshot.clone();
            }
        }

        task.status = OrisTaskStatus::Running;
        task.updated_at = chrono::Utc::now().timestamp();

        info!("Resumed task {}", task_id);
        Ok(task.clone())
    }

    /// Create a checkpoint
    pub async fn create_checkpoint(
        &self,
        task_id: &str,
        description: Option<String>,
    ) -> Result<Checkpoint, crate::AppError> {
        let tasks = self.tasks.read().await;
        let task = tasks
            .get(task_id)
            .ok_or_else(|| crate::AppError::NotFound(format!("Task {} not found", task_id)))?;

        let checkpoint = Checkpoint {
            checkpoint_id: Ulid::new().to_string(),
            task_id: task_id.to_string(),
            snapshot: task.context_snapshot.clone(),
            memory_snapshot: task.context_snapshot.memory_state.stm_entries.clone(),
            decision_snapshot: task
                .context_snapshot
                .decision_state
                .decision_history
                .clone(),
            created_at: chrono::Utc::now().timestamp(),
            description,
        };

        drop(tasks);

        self.checkpoints
            .write()
            .await
            .insert(checkpoint.checkpoint_id.clone(), checkpoint.clone());

        // Also add to task
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.checkpoints.push(checkpoint.clone());
            task.context_snapshot.decision_state.checkpoint_id =
                Some(checkpoint.checkpoint_id.clone());
        }

        info!("Created checkpoint for task {}", task_id);
        Ok(checkpoint)
    }

    /// Rollback to a checkpoint
    pub async fn rollback_to_checkpoint(
        &self,
        task_id: &str,
        checkpoint_id: &str,
    ) -> Result<OrisTaskState, crate::AppError> {
        // First get checkpoint data
        let checkpoint_data = {
            let checkpoints = self.checkpoints.read().await;
            let checkpoint = checkpoints.get(checkpoint_id).ok_or_else(|| {
                crate::AppError::NotFound(format!("Checkpoint {} not found", checkpoint_id))
            })?;

            if checkpoint.task_id != task_id {
                return Err(crate::AppError::BadRequest(
                    "Checkpoint does not belong to this task".to_string(),
                ));
            }

            checkpoint.snapshot.clone()
        };

        // Now update the task
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| crate::AppError::NotFound(format!("Task {} not found", task_id)))?;

        task.context_snapshot = checkpoint_data;
        task.status = OrisTaskStatus::RolledBack;
        task.updated_at = chrono::Utc::now().timestamp();

        info!(
            "Rolled back task {} to checkpoint {}",
            task_id, checkpoint_id
        );
        Ok(task.clone())
    }

    /// Get task state
    pub async fn get_task(&self, task_id: &str) -> Result<Option<OrisTaskState>, crate::AppError> {
        Ok(self.tasks.read().await.get(task_id).cloned())
    }

    /// Get checkpoint
    pub async fn get_checkpoint(
        &self,
        checkpoint_id: &str,
    ) -> Result<Option<Checkpoint>, crate::AppError> {
        Ok(self.checkpoints.read().await.get(checkpoint_id).cloned())
    }

    /// List task checkpoints
    pub async fn list_checkpoints(
        &self,
        task_id: &str,
    ) -> Result<Vec<Checkpoint>, crate::AppError> {
        let tasks = self.tasks.read().await;
        let task = tasks
            .get(task_id)
            .ok_or_else(|| crate::AppError::NotFound(format!("Task {} not found", task_id)))?;

        Ok(task.checkpoints.clone())
    }
}

impl Default for OrisIntegration {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_task() {
        let oris = OrisIntegration::new();
        let task = oris.create_task("agent_1").await.unwrap();
        assert_eq!(task.agent_id, "agent_1");
        assert!(matches!(task.status, OrisTaskStatus::Running));
    }

    #[tokio::test]
    async fn test_suspend_resume_task() {
        let oris = OrisIntegration::new();
        let task = oris.create_task("agent_1").await.unwrap();

        let context = ContextSnapshot {
            snapshot_id: Ulid::new().to_string(),
            task_id: task.task_id.clone(),
            memory_state: MemoryState {
                stm_entries: Vec::new(),
                active_context: Vec::new(),
                recent_accesses: Vec::new(),
            },
            decision_state: DecisionState {
                decision_history: Vec::new(),
                pending_decisions: Vec::new(),
                checkpoint_id: None,
            },
            working_memory: Vec::new(),
            metadata: HashMap::new(),
            created_at: chrono::Utc::now().timestamp(),
        };

        oris.suspend_task(&task.task_id, context).await.unwrap();

        let resumed = oris.resume_task(&task.task_id, None).await.unwrap();
        assert!(matches!(resumed.status, OrisTaskStatus::Running));
    }

    #[tokio::test]
    async fn test_checkpoint_rollback() {
        let oris = OrisIntegration::new();
        let task = oris.create_task("agent_1").await.unwrap();

        oris.create_checkpoint(&task.task_id, Some("Initial checkpoint".to_string()))
            .await
            .unwrap();

        let checkpoints = oris.list_checkpoints(&task.task_id).await.unwrap();
        assert_eq!(checkpoints.len(), 1);

        let checkpoint_id = &checkpoints[0].checkpoint_id;
        oris.rollback_to_checkpoint(&task.task_id, checkpoint_id)
            .await
            .unwrap();

        let task = oris.get_task(&task.task_id).await.unwrap().unwrap();
        assert!(matches!(task.status, OrisTaskStatus::RolledBack));
    }
}
