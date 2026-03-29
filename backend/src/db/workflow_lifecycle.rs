//! Workflow Lifecycle Management
//!
//! Tracks workflow state transitions and enforces lifecycle constraints,
//! such as ensuring child workflows complete before parents.

use crate::kernel::error::{MemoryError, MemoryResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Terminal states that indicate a workflow has finished.
const TERMINAL_STATES: &[&str] = &["completed", "failed", "terminated", "cancelled"];

/// Represents the state of a workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowState {
    /// Workflow is pending execution.
    Pending,
    /// Workflow is currently running.
    Running,
    /// Workflow is suspended (waiting for input).
    Suspended,
    /// Workflow has completed successfully.
    Completed,
    /// Workflow has failed.
    Failed,
    /// Workflow was terminated.
    Terminated,
    /// Workflow was cancelled.
    Cancelled,
}

impl WorkflowState {
    /// Check if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            WorkflowState::Completed
                | WorkflowState::Failed
                | WorkflowState::Terminated
                | WorkflowState::Cancelled
        )
    }
}

/// A workflow lifecycle record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowLifecycleRecord {
    /// Unique workflow identifier.
    pub workflow_id: String,
    /// Parent workflow ID, if any.
    pub parent_workflow_id: Option<String>,
    /// Current state.
    pub state: WorkflowState,
    /// Child workflow IDs.
    pub child_workflow_ids: Vec<String>,
    /// When the workflow was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When the workflow last transitioned state.
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Optional metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Manages workflow lifecycle state.
pub struct WorkflowLifecycle {
    /// In-memory store of workflow states.
    states: Arc<RwLock<HashMap<String, WorkflowLifecycleRecord>>>,
}

impl WorkflowLifecycle {
    /// Create a new workflow lifecycle manager.
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new workflow.
    pub async fn register(
        &self,
        workflow_id: String,
        parent_workflow_id: Option<String>,
    ) -> MemoryResult<()> {
        let mut states = self.states.write().await;

        if states.contains_key(&workflow_id) {
            return Err(MemoryError::AlreadyExists(format!(
                "Workflow {} already exists",
                workflow_id
            )));
        }

        let now = chrono::Utc::now();
        let record = WorkflowLifecycleRecord {
            workflow_id: workflow_id.clone(),
            parent_workflow_id: parent_workflow_id.clone(),
            state: WorkflowState::Pending,
            child_workflow_ids: Vec::new(),
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        };

        // Add to parent's children list
        if let Some(parent_id) = &parent_workflow_id {
            if let Some(parent) = states.get_mut(parent_id) {
                parent.child_workflow_ids.push(workflow_id.clone());
            }
        }

        states.insert(workflow_id, record);
        Ok(())
    }

    /// Transition a workflow to a new state.
    pub async fn transition(
        &self,
        workflow_id: &str,
        new_state: WorkflowState,
    ) -> MemoryResult<()> {
        let mut states = self.states.write().await;

        let record = states
            .get_mut(workflow_id)
            .ok_or_else(|| MemoryError::NotFound(format!("Workflow {} not found", workflow_id)))?;

        // Validate transition
        if record.state.is_terminal() {
            return Err(MemoryError::InvalidOperation(format!(
                "Cannot transition from terminal state {:?}",
                record.state
            )));
        }

        record.state = new_state;
        record.updated_at = chrono::Utc::now();
        Ok(())
    }

    /// Get the current state of a workflow.
    pub async fn get_state(&self, workflow_id: &str) -> MemoryResult<WorkflowState> {
        let states = self.states.read().await;
        let record = states
            .get(workflow_id)
            .ok_or_else(|| MemoryError::NotFound(format!("Workflow {} not found", workflow_id)))?;
        Ok(record.state)
    }

    /// Get the lifecycle record for a workflow.
    pub async fn get_record(&self, workflow_id: &str) -> MemoryResult<WorkflowLifecycleRecord> {
        let states = self.states.read().await;
        states
            .get(workflow_id)
            .cloned()
            .ok_or_else(|| MemoryError::NotFound(format!("Workflow {} not found", workflow_id)))
    }

    /// Check if a child workflow is in a terminal state.
    pub async fn is_child_terminal(&self, child_id: &str) -> MemoryResult<bool> {
        let state = self.get_state(child_id).await?;
        Ok(state.is_terminal())
    }

    /// Ensure all child workflows of a parent have reached terminal states.
    ///
    /// This is used to enforce that a parent workflow cannot complete
    /// until all its children have completed.
    pub async fn ensure_child_complete(
        &self,
        parent_id: &str,
        child_id: &str,
    ) -> MemoryResult<()> {
        let states = self.states.read().await;

        // Verify parent exists
        let parent = states
            .get(parent_id)
            .ok_or_else(|| MemoryError::NotFound(format!("Parent workflow {} not found", parent_id)))?;

        // Verify child exists and is in parent's children list
        if !parent.child_workflow_ids.contains(&child_id.to_string()) {
            return Err(MemoryError::NotFound(format!(
                "Child {} is not a child of parent {}",
                child_id, parent_id
            )));
        }

        // Get child's state
        let child = states
            .get(child_id)
            .ok_or_else(|| MemoryError::NotFound(format!("Child workflow {} not found", child_id)))?;

        // Check if child is in terminal state
        if !child.state.is_terminal() {
            return Err(MemoryError::InvalidOperation(format!(
                "Child workflow {} is not in terminal state (current: {:?})",
                child_id, child.state
            )));
        }

        Ok(())
    }

    /// Get all child workflow IDs for a parent.
    pub async fn get_children(&self, parent_id: &str) -> MemoryResult<Vec<String>> {
        let states = self.states.read().await;
        let parent = states
            .get(parent_id)
            .ok_or_else(|| MemoryError::NotFound(format!("Workflow {} not found", parent_id)))?;
        Ok(parent.child_workflow_ids.clone())
    }

    /// Check if all children of a workflow are terminal.
    pub async fn all_children_terminal(&self, parent_id: &str) -> MemoryResult<bool> {
        let children = self.get_children(parent_id).await?;
        for child_id in &children {
            if !self.is_child_terminal(child_id).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Remove a workflow record.
    pub async fn remove(&self, workflow_id: &str) -> MemoryResult<()> {
        let mut states = self.states.write().await;

        // First check if workflow exists and get its children
        let child_ids = {
            let record = states
                .get(workflow_id)
                .ok_or_else(|| MemoryError::NotFound(format!("Workflow {} not found", workflow_id)))?;

            // Cannot remove a workflow with active children
            if !record.child_workflow_ids.is_empty() {
                let any_active = record
                    .child_workflow_ids
                    .iter()
                    .any(|child_id| {
                        states
                            .get(child_id)
                            .map(|r| !r.state.is_terminal())
                            .unwrap_or(false)
                    });

                if any_active {
                    return Err(MemoryError::InvalidOperation(
                        "Cannot remove workflow with active children".to_string(),
                    ));
                }
            }

            record.child_workflow_ids.clone()
        };

        // Remove from parent's children list
        if let Some(parent_id) = states.get(workflow_id).and_then(|r| r.parent_workflow_id.clone()) {
            if let Some(parent) = states.get_mut(&parent_id) {
                parent.child_workflow_ids.retain(|id| id != workflow_id);
            }
        }

        states.remove(workflow_id);
        Ok(())
    }
}

impl Default for WorkflowLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_workflow() {
        let lifecycle = WorkflowLifecycle::new();

        lifecycle
            .register("wf-1".to_string(), None)
            .await
            .unwrap();

        let state = lifecycle.get_state("wf-1").await.unwrap();
        assert_eq!(state, WorkflowState::Pending);
    }

    #[tokio::test]
    async fn test_register_with_parent() {
        let lifecycle = WorkflowLifecycle::new();

        lifecycle
            .register("parent".to_string(), None)
            .await
            .unwrap();
        lifecycle
            .register("child".to_string(), Some("parent".to_string()))
            .await
            .unwrap();

        let children = lifecycle.get_children("parent").await.unwrap();
        assert_eq!(children, vec!["child"]);
    }

    #[tokio::test]
    async fn test_ensure_child_complete() {
        let lifecycle = WorkflowLifecycle::new();

        // Register parent and child
        lifecycle
            .register("parent".to_string(), None)
            .await
            .unwrap();
        lifecycle
            .register("child".to_string(), Some("parent".to_string()))
            .await
            .unwrap();

        // Child not terminal yet
        assert!(lifecycle.ensure_child_complete("parent", "child").await.is_err());

        // Complete the child
        lifecycle
            .transition("child", WorkflowState::Completed)
            .await
            .unwrap();

        // Now it should pass
        assert!(lifecycle.ensure_child_complete("parent", "child").await.is_ok());
    }

    #[tokio::test]
    async fn test_transition_to_terminal() {
        let lifecycle = WorkflowLifecycle::new();

        lifecycle
            .register("wf-1".to_string(), None)
            .await
            .unwrap();

        lifecycle
            .transition("wf-1", WorkflowState::Running)
            .await
            .unwrap();

        lifecycle
            .transition("wf-1", WorkflowState::Completed)
            .await
            .unwrap();

        let state = lifecycle.get_state("wf-1").await.unwrap();
        assert_eq!(state, WorkflowState::Completed);

        // Cannot transition from terminal
        assert!(lifecycle
            .transition("wf-1", WorkflowState::Failed)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_all_children_terminal() {
        let lifecycle = WorkflowLifecycle::new();

        lifecycle
            .register("parent".to_string(), None)
            .await
            .unwrap();
        lifecycle
            .register("child1".to_string(), Some("parent".to_string()))
            .await
            .unwrap();
        lifecycle
            .register("child2".to_string(), Some("parent".to_string()))
            .await
            .unwrap();

        // Not all terminal yet
        assert!(!lifecycle.all_children_terminal("parent").await.unwrap());

        // Complete both children
        lifecycle
            .transition("child1", WorkflowState::Completed)
            .await
            .unwrap();
        assert!(!lifecycle.all_children_terminal("parent").await.unwrap());

        lifecycle
            .transition("child2", WorkflowState::Completed)
            .await
            .unwrap();
        assert!(lifecycle.all_children_terminal("parent").await.unwrap());
    }
}
