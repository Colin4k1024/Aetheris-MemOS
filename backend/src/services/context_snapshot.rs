//! Context Snapshot Service
//!
//! This service manages context snapshots for Oris task state persistence.

use crate::integrations::oris::{
    Checkpoint, ContextSnapshot, MemoryState, OrisIntegration, OrisTaskState,
};
use crate::kernel::types::MemoryId;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Global Oris integration instance
static ORIS_INTEGRATION: std::sync::OnceLock<Arc<OrisIntegration>> = std::sync::OnceLock::new();

/// Get the global Oris integration instance
pub fn get_oris_integration() -> &'static Arc<OrisIntegration> {
    ORIS_INTEGRATION
        .get_or_init(|| Arc::new(OrisIntegration::new()))
}

/// Initialize Oris integration
pub fn init_oris() -> &'static Arc<OrisIntegration> {
    get_oris_integration()
}

/// Snapshot service for creating and managing context snapshots
pub struct ContextSnapshotService {
    oris: Arc<OrisIntegration>,
}

impl ContextSnapshotService {
    /// Create a new snapshot service
    pub fn new() -> Self {
        Self {
            oris: get_oris_integration().clone(),
        }
    }

    /// Create a task and return task state
    pub async fn create_task(&self, agent_id: &str) -> Result<OrisTaskState, crate::AppError> {
        self.oris.create_task(agent_id).await
    }

    /// Create a snapshot for a task
    pub async fn create_snapshot(
        &self,
        task_id: &str,
        working_memory: Vec<MemoryId>,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<ContextSnapshot, crate::AppError> {
        let snapshot = ContextSnapshot {
            snapshot_id: ulid::Ulid::new().to_string(),
            task_id: task_id.to_string(),
            memory_state: MemoryState {
                stm_entries: Vec::new(),
                active_context: working_memory,
                recent_accesses: Vec::new(),
            },
            decision_state: crate::integrations::oris::DecisionState {
                decision_history: Vec::new(),
                pending_decisions: Vec::new(),
                checkpoint_id: None,
            },
            working_memory: Vec::new(),
            metadata: metadata.unwrap_or_default(),
            created_at: chrono::Utc::now().timestamp(),
        };

        self.oris.suspend_task(task_id, snapshot).await
    }

    /// Restore a task from a snapshot
    pub async fn restore_snapshot(
        &self,
        task_id: &str,
        snapshot_id: Option<String>,
    ) -> Result<OrisTaskState, crate::AppError> {
        self.oris.resume_task(task_id, snapshot_id).await
    }

    /// Create a checkpoint
    pub async fn create_checkpoint(
        &self,
        task_id: &str,
        description: Option<String>,
    ) -> Result<Checkpoint, crate::AppError> {
        self.oris.create_checkpoint(task_id, description).await
    }

    /// Rollback to a checkpoint
    pub async fn rollback_to_checkpoint(
        &self,
        task_id: &str,
        checkpoint_id: &str,
    ) -> Result<OrisTaskState, crate::AppError> {
        self.oris
            .rollback_to_checkpoint(task_id, checkpoint_id)
            .await
    }

    /// Get task state
    pub async fn get_task(&self, task_id: &str) -> Result<Option<OrisTaskState>, crate::AppError> {
        self.oris.get_task(task_id).await
    }

    /// List checkpoints for a task
    pub async fn list_checkpoints(
        &self,
        task_id: &str,
    ) -> Result<Vec<Checkpoint>, crate::AppError> {
        self.oris.list_checkpoints(task_id).await
    }
}

impl Default for ContextSnapshotService {
    fn default() -> Self {
        Self::new()
    }
}
