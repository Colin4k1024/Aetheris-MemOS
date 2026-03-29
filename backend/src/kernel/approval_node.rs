//! HITL Approval Node Abstraction
//!
//! This module provides the ApprovalNode for native HITL orchestration,
//! allowing workflows to pause and wait for human approval before proceeding.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::AppError;
use crate::services::approval_manager::ApprovalManager;

/// Escalation policy when approval times out
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum EscalationPolicy {
    /// Notify a specific role or user
    Notify(String),
    /// Automatically reject the pending approval
    AutoReject,
    /// Automatically approve (use with caution)
    AutoApprove,
}

/// Current status of an approval request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    /// Waiting for human approval
    Pending,
    /// Approved by an authorized user
    Approved,
    /// Rejected by an authorized user
    Rejected,
    /// Approval timed out before resolution
    Expired,
}

/// Output produced by the ApprovalNode after execution
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NodeOutput {
    /// The approval ID for tracking this request
    pub approval_id: String,
    /// Current status of the approval
    pub status: ApprovalStatus,
    /// Human-readable message describing the output
    pub message: String,
}

/// Execution context passed to ApprovalNode
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecutionContext {
    /// Unique identifier for the current workflow execution
    pub workflow_id: String,
    /// Unique identifier for the node within the workflow
    pub node_id: String,
    /// RBAC role required to approve this node
    pub required_role: String,
}

/// Input to the ApprovalNode
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApprovalInput {
    /// Description of what needs approval
    pub description: String,
    /// Snapshot of workflow state at approval point
    pub workflow_snapshot: Vec<u8>,
    /// Optional callback URL for async notification
    pub callback_url: Option<String>,
}

/// ApprovalNode for HITL orchestration
///
/// This node pauses workflow execution and waits for human approval
/// before transitioning to the next state.
#[derive(Debug, Clone)]
pub struct ApprovalNode {
    /// Unique approval ID
    approval_id: String,
    /// RBAC role required to approve
    required_rbac_role: String,
    /// Duration to wait before timeout
    timeout_duration: Duration,
    /// Policy to apply on timeout
    escalation_policy: EscalationPolicy,
}

impl ApprovalNode {
    /// Create a new ApprovalNode
    pub fn new(
        approval_id: impl Into<String>,
        required_rbac_role: impl Into<String>,
        timeout_duration: Duration,
        escalation_policy: EscalationPolicy,
    ) -> Self {
        Self {
            approval_id: approval_id.into(),
            required_rbac_role: required_rbac_role.into(),
            timeout_duration,
            escalation_policy,
        }
    }

    /// Execute the approval node, transitioning to job_waiting state
    ///
    /// Returns immediately with the approval request details.
    /// The caller should await the approval asynchronously.
    pub async fn execute(
        &self,
        _ctx: &ExecutionContext,
        input: &ApprovalInput,
    ) -> Result<NodeOutput, AppError> {
        let approval_manager = ApprovalManager::get_instance();

        // Request approval from the manager
        let approval_id = approval_manager
            .request_approval(self, &_ctx.workflow_id, input.workflow_snapshot.clone())
            .await?;

        tracing::info!(
            approval_id = %approval_id,
            workflow_id = %_ctx.workflow_id,
            node_id = %_ctx.node_id,
            required_role = %self.required_rbac_role,
            "Approval requested, workflow paused"
        );

        Ok(NodeOutput {
            approval_id,
            status: ApprovalStatus::Pending,
            message: format!(
                "Approval required for: {}. Waiting for {} approval.",
                input.description, self.required_rbac_role
            ),
        })
    }

    /// Get the required RBAC role for this approval
    pub fn required_role(&self) -> &str {
        &self.required_rbac_role
    }

    /// Get the timeout duration
    pub fn timeout(&self) -> Duration {
        self.timeout_duration
    }

    /// Get the escalation policy
    pub fn escalation_policy(&self) -> &EscalationPolicy {
        &self.escalation_policy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_approval_node_creation() {
        let node = ApprovalNode::new(
            "test-approval-1",
            "admin",
            Duration::from_secs(300),
            EscalationPolicy::Notify("admin@example.com".to_string()),
        );

        assert_eq!(node.required_role(), "admin");
        assert_eq!(node.timeout(), Duration::from_secs(300));
    }
}
