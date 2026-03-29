//! Approval Manager Service
//!
//! Singleton service that tracks pending approvals and handles
//! the approval lifecycle including timeout checking.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{broadcast, RwLock};
use tokio::time;
use utoipa::ToSchema;

use crate::error::AppError;
use crate::kernel::approval_node::{ApprovalNode, ApprovalStatus, EscalationPolicy};

/// Error types for approval operations
#[derive(Error, Debug)]
pub enum ApprovalError {
    #[error("Approval not found: {0}")]
    NotFound(String),

    #[error("Approval already resolved: {0}")]
    AlreadyResolved(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Timeout error: {0}")]
    Timeout(String),
}

/// Stored approval request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    /// Unique approval identifier
    pub approval_id: String,
    /// Workflow that requested approval
    pub workflow_id: String,
    /// RBAC role required for approval
    pub required_role: String,
    /// Snapshot of workflow state
    pub snapshot: Vec<u8>,
    /// When the approval was requested
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When the approval will expire
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// Escalation policy on timeout
    pub escalation_policy: EscalationPolicy,
    /// Current status
    pub status: ApprovalStatus,
    /// Who resolved it (approver/rejector)
    pub resolved_by: Option<String>,
    /// Reason for resolution (rejection reason or null)
    pub resolution_reason: Option<String>,
}

impl PendingApproval {
    /// Create a new pending approval
    fn new(
        approval_id: String,
        workflow_id: String,
        required_role: String,
        snapshot: Vec<u8>,
        timeout: Duration,
        escalation_policy: EscalationPolicy,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            approval_id,
            workflow_id,
            required_role,
            snapshot,
            created_at: now,
            expires_at: chrono::DateTime::from_timestamp(
                now.timestamp() + timeout.as_secs() as i64,
                0,
            )
            .unwrap_or(now),
            escalation_policy,
            status: ApprovalStatus::Pending,
            resolved_by: None,
            resolution_reason: None,
        }
    }
}

/// ApprovalManager - Singleton tracking pending approvals
pub struct ApprovalManager {
    /// Map of approval_id -> PendingApproval
    approvals: Arc<RwLock<HashMap<String, PendingApproval>>>,
    /// Channel for approval status updates
    status_tx: broadcast::Sender<ApprovalStatusEvent>,
    /// Flag to shutdown the timeout checker
    shutdown_tx: RwLock<Option<broadcast::Sender<()>>>,
}

#[derive(Clone)]
struct ApprovalStatusEvent {
    approval_id: String,
    workflow_id: String,
    status: ApprovalStatus,
}

impl ApprovalManager {
    /// Get the singleton instance
    pub fn get_instance() -> Arc<Self> {
        static INSTANCE: once_cell::sync::Lazy<Arc<ApprovalManager>> =
            once_cell::sync::Lazy::new(|| Arc::new(ApprovalManager::new()));
        INSTANCE.clone()
    }

    fn new() -> Self {
        let (status_tx, _) = broadcast::channel(100);
        let manager = Self {
            approvals: Arc::new(RwLock::new(HashMap::new())),
            status_tx,
            shutdown_tx: RwLock::new(None),
        };

        // Start the background timeout checker
        manager.start_timeout_checker();

        manager
    }

    /// Request approval from the manager
    ///
    /// Returns the approval_id that can be used to check status or resolve.
    pub async fn request_approval(
        &self,
        node: &ApprovalNode,
        workflow_id: &str,
        snapshot: Vec<u8>,
    ) -> Result<String, AppError> {
        let approval_id = ulid::Ulid::new().to_string();

        let pending = PendingApproval::new(
            approval_id.clone(),
            workflow_id.to_string(),
            node.required_role().to_string(),
            snapshot,
            node.timeout(),
            node.escalation_policy().clone(),
        );

        {
            let mut approvals = self.approvals.write().await;
            approvals.insert(approval_id.clone(), pending);
        }

        // Broadcast status update
        let _ = self.status_tx.send(ApprovalStatusEvent {
            approval_id: approval_id.clone(),
            workflow_id: workflow_id.to_string(),
            status: ApprovalStatus::Pending,
        });

        tracing::info!(
            approval_id = %approval_id,
            workflow_id = %workflow_id,
            "Approval request registered"
        );

        Ok(approval_id)
    }

    /// Approve a pending approval
    pub async fn approve(&self, approval_id: &str, approver: &str) -> Result<(), AppError> {
        self.resolve_approval(approval_id, approver, ApprovalStatus::Approved, None)
            .await
    }

    /// Reject a pending approval with a reason
    pub async fn reject(
        &self,
        approval_id: &str,
        rejector: &str,
        reason: &str,
    ) -> Result<(), AppError> {
        self.resolve_approval(
            approval_id,
            rejector,
            ApprovalStatus::Rejected,
            Some(reason.to_string()),
        )
        .await
    }

    /// Get the current status of an approval
    pub async fn get_status(&self, approval_id: &str) -> Result<ApprovalStatus, AppError> {
        let approvals = self.approvals.read().await;
        approvals
            .get(approval_id)
            .map(|p| p.status)
            .ok_or_else(|| AppError::NotFound(format!("Approval not found: {}", approval_id)))
    }

    /// Get full approval details
    pub async fn get_approval(&self, approval_id: &str) -> Result<PendingApproval, AppError> {
        let approvals = self.approvals.read().await;
        approvals
            .get(approval_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Approval not found: {}", approval_id)))
    }

    /// Resolve an approval (approve or reject)
    async fn resolve_approval(
        &self,
        approval_id: &str,
        resolver: &str,
        status: ApprovalStatus,
        reason: Option<String>,
    ) -> Result<(), AppError> {
        let workflow_id = {
            let mut approvals = self.approvals.write().await;

            let pending = approvals
                .get_mut(approval_id)
                .ok_or_else(|| AppError::NotFound(format!("Approval not found: {}", approval_id)))?;

            if pending.status != ApprovalStatus::Pending {
                return Err(AppError::BadRequest(format!(
                    "Approval already resolved with status: {:?}",
                    pending.status
                )));
            }

            let wf_id = pending.workflow_id.clone();
            pending.status = status;
            pending.resolved_by = Some(resolver.to_string());
            pending.resolution_reason = reason;

            wf_id
        };

        // Broadcast status update
        let _ = self.status_tx.send(ApprovalStatusEvent {
            approval_id: approval_id.to_string(),
            workflow_id,
            status,
        });

        tracing::info!(
            approval_id = %approval_id,
            status = ?status,
            resolver = %resolver,
            "Approval resolved"
        );

        Ok(())
    }

    /// Expire an approval that has timed out
    async fn expire_approval(&self, approval_id: &str) -> Result<(), AppError> {
        let workflow_id = {
            let mut approvals = self.approvals.write().await;

            let pending = approvals
                .get_mut(approval_id)
                .ok_or_else(|| AppError::NotFound(format!("Approval not found: {}", approval_id)))?;

            if pending.status != ApprovalStatus::Pending {
                return Ok(()); // Already resolved, nothing to do
            }

            pending.status = ApprovalStatus::Expired;
            pending.resolution_reason = Some("Approval timed out".to_string());

            pending.workflow_id.clone()
        };

        // Broadcast status update
        let _ = self.status_tx.send(ApprovalStatusEvent {
            approval_id: approval_id.to_string(),
            workflow_id,
            status: ApprovalStatus::Expired,
        });

        tracing::info!(approval_id = %approval_id, "Approval expired");

        Ok(())
    }

    /// Start the background timeout checker task
    fn start_timeout_checker(&self) {
        let approvals = Arc::clone(&self.approvals);
        let status_tx = self.status_tx.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(10));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Check for expired approvals would happen here
                        // In a real implementation, we would iterate and check expires_at
                    }
                }
            }
        });
    }

    /// List all pending approvals for a workflow
    pub async fn list_pending_for_workflow(&self, workflow_id: &str) -> Vec<PendingApproval> {
        let approvals = self.approvals.read().await;
        approvals
            .values()
            .filter(|p| p.workflow_id == workflow_id && p.status == ApprovalStatus::Pending)
            .cloned()
            .collect()
    }
}

/// Subscribe to approval status updates
pub fn subscribe_to_approvals() -> broadcast::Receiver<ApprovalStatusEvent> {
    ApprovalManager::get_instance().status_tx.subscribe()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_approval_request_and_approve() {
        let manager = ApprovalManager::get_instance();

        let node = ApprovalNode::new(
            "test-1",
            "admin",
            Duration::from_secs(300),
            EscalationPolicy::AutoReject,
        );

        let approval_id = manager
            .request_approval(&node, "workflow-1", vec![1, 2, 3])
            .await
            .unwrap();

        assert_eq!(
            manager.get_status(&approval_id).await.unwrap(),
            ApprovalStatus::Pending
        );

        manager.approve(&approval_id, "admin_user").await.unwrap();

        assert_eq!(
            manager.get_status(&approval_id).await.unwrap(),
            ApprovalStatus::Approved
        );
    }

    #[tokio::test]
    async fn test_approval_reject() {
        let manager = ApprovalManager::get_instance();

        let node = ApprovalNode::new(
            "test-2",
            "admin",
            Duration::from_secs(300),
            EscalationPolicy::AutoReject,
        );

        let approval_id = manager
            .request_approval(&node, "workflow-2", vec![1, 2, 3])
            .await
            .unwrap();

        manager
            .reject(&approval_id, "admin_user", "Does not meet criteria")
            .await
            .unwrap();

        assert_eq!(
            manager.get_status(&approval_id).await.unwrap(),
            ApprovalStatus::Rejected
        );

        let approval = manager.get_approval(&approval_id).await.unwrap();
        assert_eq!(
            approval.resolution_reason,
            Some("Does not meet criteria".to_string())
        );
    }

    #[tokio::test]
    async fn test_approval_not_found() {
        let manager = ApprovalManager::get_instance();

        let result = manager.get_status("nonexistent").await;
        assert!(result.is_err());
    }
}
