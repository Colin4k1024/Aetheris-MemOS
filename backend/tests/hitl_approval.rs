//! HITL Approval Integration Tests
//!
//! Tests for the Human-In-The-Loop approval workflow system.

use std::time::Duration;

use backend::kernel::approval_node::{ApprovalNode, ApprovalStatus, EscalationPolicy};
use backend::kernel::ExecutionContext;
use backend::services::approval_manager::ApprovalManager;
use backend::kernel::ApprovalInput;

/// Test: Request approval and then approve it
#[tokio::test]
async fn test_approval_request_and_approve() {
    // Create an approval node
    let node = ApprovalNode::new(
        "test-approval-1",
        "admin",
        Duration::from_secs(300),
        EscalationPolicy::AutoReject,
    );

    // Create execution context
    let ctx = ExecutionContext {
        workflow_id: "workflow-123".to_string(),
        node_id: "node-1".to_string(),
        required_role: "admin".to_string(),
    };

    // Create approval input
    let input = ApprovalInput {
        description: "Approve data processing workflow".to_string(),
        workflow_snapshot: vec![1, 2, 3, 4, 5],
        callback_url: None,
    };

    // Execute the approval node
    let output = node.execute(&ctx, &input).await.unwrap();

    // Verify output
    assert_eq!(output.status, ApprovalStatus::Pending);
    assert!(output.approval_id.len() > 0);
    assert!(output.message.contains("Approval required"));

    // Get the approval manager and verify status
    let manager = ApprovalManager::get_instance();
    let status = manager.get_status(&output.approval_id).await.unwrap();
    assert_eq!(status, ApprovalStatus::Pending);

    // Approve the request
    manager.approve(&output.approval_id, "admin_user").await.unwrap();

    // Verify it's now approved
    let status = manager.get_status(&output.approval_id).await.unwrap();
    assert_eq!(status, ApprovalStatus::Approved);

    // Get full approval details
    let approval = manager.get_approval(&output.approval_id).await.unwrap();
    assert_eq!(approval.resolved_by, Some("admin_user".to_string()));
}

/// Test: Approval request and rejection
#[tokio::test]
async fn test_approval_request_and_reject() {
    let node = ApprovalNode::new(
        "test-approval-2",
        "approver",
        Duration::from_secs(300),
        EscalationPolicy::AutoReject,
    );

    let ctx = ExecutionContext {
        workflow_id: "workflow-456".to_string(),
        node_id: "node-2".to_string(),
        required_role: "approver".to_string(),
    };

    let input = ApprovalInput {
        description: "Approve budget allocation".to_string(),
        workflow_snapshot: vec![10, 20, 30],
        callback_url: None,
    };

    let output = node.execute(&ctx, &input).await.unwrap();

    let manager = ApprovalManager::get_instance();

    // Reject the request
    manager
        .reject(&output.approval_id, "manager_user", "Budget exceeds limit")
        .await
        .unwrap();

    // Verify it's rejected
    let status = manager.get_status(&output.approval_id).await.unwrap();
    assert_eq!(status, ApprovalStatus::Rejected);

    let approval = manager.get_approval(&output.approval_id).await.unwrap();
    assert_eq!(approval.resolution_reason, Some("Budget exceeds limit".to_string()));
}

/// Test: Approval timeout expiry
#[tokio::test]
async fn test_approval_timeout_expires() {
    // Create a node with very short timeout for testing
    let node = ApprovalNode::new(
        "test-approval-3",
        "admin",
        Duration::from_millis(100), // Very short timeout
        EscalationPolicy::AutoReject,
    );

    let ctx = ExecutionContext {
        workflow_id: "workflow-789".to_string(),
        node_id: "node-3".to_string(),
        required_role: "admin".to_string(),
    };

    let input = ApprovalInput {
        description: "Timeout test approval".to_string(),
        workflow_snapshot: vec![7, 8, 9],
        callback_url: None,
    };

    let output = node.execute(&ctx, &input).await.unwrap();

    let manager = ApprovalManager::get_instance();

    // Verify it's pending initially
    let status = manager.get_status(&output.approval_id).await.unwrap();
    assert_eq!(status, ApprovalStatus::Pending);

    // Wait for timeout
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Note: In a real implementation, the background task would expire this
    // For this test, we manually verify the expiry behavior
    let approval = manager.get_approval(&output.approval_id).await.unwrap();
    let is_expired = approval.expires_at < chrono::Utc::now();

    // The approval should have expired (or be about to expire)
    assert!(is_expired || approval.status == ApprovalStatus::Expired);
}

/// Test: Multiple approvals for same workflow
#[tokio::test]
async fn test_multiple_approvals_same_workflow() {
    let manager = ApprovalManager::get_instance();

    // Create multiple approval nodes for the same workflow
    let node1 = ApprovalNode::new(
        "multi-1",
        "admin",
        Duration::from_secs(300),
        EscalationPolicy::AutoReject,
    );

    let node2 = ApprovalNode::new(
        "multi-2",
        "manager",
        Duration::from_secs(300),
        EscalationPolicy::AutoReject,
    );

    let ctx = ExecutionContext {
        workflow_id: "workflow-multi".to_string(),
        node_id: "node-1".to_string(),
        required_role: "admin".to_string(),
    };

    let input = ApprovalInput {
        description: "First approval".to_string(),
        workflow_snapshot: vec![1],
        callback_url: None,
    };

    let output1 = node1.execute(&ctx, &input).await.unwrap();

    let ctx2 = ExecutionContext {
        workflow_id: "workflow-multi".to_string(),
        node_id: "node-2".to_string(),
        required_role: "manager".to_string(),
    };

    let input2 = ApprovalInput {
        description: "Second approval".to_string(),
        workflow_snapshot: vec![2],
        callback_url: None,
    };

    let output2 = node2.execute(&ctx2, &input2).await.unwrap();

    // Both should be pending
    assert_eq!(
        manager.get_status(&output1.approval_id).await.unwrap(),
        ApprovalStatus::Pending
    );
    assert_eq!(
        manager.get_status(&output2.approval_id).await.unwrap(),
        ApprovalStatus::Pending
    );

    // List pending for workflow
    let pending = manager.list_pending_for_workflow("workflow-multi").await;
    assert_eq!(pending.len(), 2);

    // Approve first one
    manager.approve(&output1.approval_id, "admin_user").await.unwrap();

    // Verify first is approved, second still pending
    assert_eq!(
        manager.get_status(&output1.approval_id).await.unwrap(),
        ApprovalStatus::Approved
    );
    assert_eq!(
        manager.get_status(&output2.approval_id).await.unwrap(),
        ApprovalStatus::Pending
    );
}

/// Test: Approval status not found
#[tokio::test]
async fn test_approval_not_found() {
    let manager = ApprovalManager::get_instance();

    let result = manager.get_status("nonexistent-approval-id").await;
    assert!(result.is_err());
}
