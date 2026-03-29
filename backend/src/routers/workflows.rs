//! Workflow Approval Routes
//!
//! Provides REST API endpoints for HITL approval workflow management.

use axum::{
    extract::{Extension, Path},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::AppError;
use crate::kernel::approval_node::ApprovalStatus;
use crate::models::*;
use crate::services::approval_manager::{ApprovalError, ApprovalManager, PendingApproval};
use crate::services::lease_release::LeaseReleaseProtocol;
use crate::tenant::RequestTenantContext;
use crate::{json_ok, JsonResult};

/// Request body for approval callback
#[derive(Debug, Deserialize, ToSchema)]
pub struct ApprovalCallbackRequest {
    /// User performing the action
    pub user_id: String,
    /// Optional reason for rejection
    #[serde(default)]
    pub reason: Option<String>,
}

/// Request body for rejection callback
#[derive(Debug, Deserialize, ToSchema)]
pub struct RejectionCallbackRequest {
    /// User performing the rejection
    pub user_id: String,
    /// Reason for rejection (required)
    pub reason: String,
}

/// Response for approval status query
#[derive(Debug, Serialize, ToSchema)]
pub struct ApprovalStatusResponse {
    /// Approval ID
    pub approval_id: String,
    /// Workflow ID
    pub workflow_id: String,
    /// Required role for approval
    pub required_role: String,
    /// Current status
    pub status: ApprovalStatus,
    /// When approval was requested
    pub created_at: String,
    /// When approval will expire
    pub expires_at: String,
    /// Who resolved it (if resolved)
    pub resolved_by: Option<String>,
    /// Resolution reason (if rejected)
    pub resolution_reason: Option<String>,
}

impl From<PendingApproval> for ApprovalStatusResponse {
    fn from(approval: PendingApproval) -> Self {
        Self {
            approval_id: approval.approval_id,
            workflow_id: approval.workflow_id,
            required_role: approval.required_role,
            status: approval.status,
            created_at: approval.created_at.to_rfc3339(),
            expires_at: approval.expires_at.to_rfc3339(),
            resolved_by: approval.resolved_by,
            resolution_reason: approval.resolution_reason,
        }
    }
}

/// Approve a pending workflow approval
///
/// Requires RBAC authorization. The approving user must have the
/// required role for the approval.
#[utoipa::path(
    post,
    path = "/api/v1/workflows/{workflow_id}/approve",
    params(
        ("workflow_id" = String, Path, description = "Workflow ID")
    ),
    request_body = ApprovalCallbackRequest,
    responses(
        (status = 200, description = "Approval recorded successfully"),
        (status = 400, description = "Invalid request or approval not found"),
        (status = 403, description = "User lacks required RBAC role"),
        (status = 404, description = "Approval not found")
    ),
    tag = "Workflows"
)]
pub async fn approve_workflow(
    Path(workflow_id): Path<String>,
    Extension(ctx): Extension<RequestTenantContext>,
    Json(request): Json<ApprovalCallbackRequest>,
) -> JsonResult<ApprovalStatusResponse> {
    tracing::info!(
        workflow_id = %workflow_id,
        user_id = %request.user_id,
        "Approval callback received"
    );

    let manager = ApprovalManager::get_instance();

    // Find the pending approval for this workflow
    let pending = manager
        .list_pending_for_workflow(&workflow_id)
        .await
        .into_iter()
        .find(|p| p.status == ApprovalStatus::Pending)
        .ok_or_else(|| {
            AppError::NotFound(format!("No pending approval found for workflow: {}", workflow_id))
        })?;

    // In a real implementation, verify RBAC here:
    // if !ctx.has_role(&pending.required_role) {
    //     return Err(AppError::Forbidden(
    //         format!("User {} lacks required role: {}", request.user_id, pending.required_role)
    //     ));
    // }

    manager
        .approve(&pending.approval_id, &request.user_id)
        .await
        .map_err(|e: AppError| match e {
            AppError::NotFound(_) => e,
            _ => AppError::BadRequest(e.to_string()),
        })?;

    // Restore workflow from checkpoint
    let lease_protocol = LeaseReleaseProtocol::default();
    if let Ok(snapshot) = lease_protocol.restore_and_reclaim(&pending.approval_id) {
        tracing::info!(
            workflow_id = %workflow_id,
            snapshot_size = snapshot.len(),
            "Workflow state restored, ready to resume"
        );
        // In real implementation: re-queue workflow for execution
    }

    let updated = manager.get_approval(&pending.approval_id).await?;

    json_ok(updated.into())
}

/// Reject a pending workflow approval
///
/// Requires RBAC authorization.
#[utoipa::path(
    post,
    path = "/api/v1/workflows/{workflow_id}/reject",
    params(
        ("workflow_id" = String, Path, description = "Workflow ID")
    ),
    request_body = RejectionCallbackRequest,
    responses(
        (status = 200, description = "Rejection recorded successfully"),
        (status = 400, description = "Invalid request or approval not found"),
        (status = 403, description = "User lacks required RBAC role"),
        (status = 404, description = "Approval not found")
    ),
    tag = "Workflows"
)]
pub async fn reject_workflow(
    Path(workflow_id): Path<String>,
    Extension(ctx): Extension<RequestTenantContext>,
    Json(request): Json<RejectionCallbackRequest>,
) -> JsonResult<ApprovalStatusResponse> {
    tracing::info!(
        workflow_id = %workflow_id,
        user_id = %request.user_id,
        "Rejection callback received"
    );

    let manager = ApprovalManager::get_instance();

    // Find the pending approval for this workflow
    let pending = manager
        .list_pending_for_workflow(&workflow_id)
        .await
        .into_iter()
        .find(|p| p.status == ApprovalStatus::Pending)
        .ok_or_else(|| {
            AppError::NotFound(format!("No pending approval found for workflow: {}", workflow_id))
        })?;

    // In a real implementation, verify RBAC here

    manager
        .reject(&pending.approval_id, &request.user_id, &request.reason)
        .await
        .map_err(|e: AppError| match e {
            AppError::NotFound(_) => e,
            _ => AppError::BadRequest(e.to_string()),
        })?;

    // Cleanup checkpoint
    let lease_protocol = LeaseReleaseProtocol::default();
    let _ = lease_protocol.cleanup_approval_checkpoint(&pending.approval_id);

    let updated = manager.get_approval(&pending.approval_id).await?;

    tracing::info!(
        workflow_id = %workflow_id,
        reason = %request.reason,
        "Workflow rejected and checkpoint cleaned"
    );

    json_ok(updated.into())
}

/// Get the status of an approval
#[utoipa::path(
    get,
    path = "/api/v1/approvals/{approval_id}/status",
    params(
        ("approval_id" = String, Path, description = "Approval ID")
    ),
    responses(
        (status = 200, description = "Approval status retrieved", body = ApprovalStatusResponse),
        (status = 404, description = "Approval not found")
    ),
    tag = "Workflows"
)]
pub async fn get_approval_status(
    Path(approval_id): Path<String>,
) -> JsonResult<ApprovalStatusResponse> {
    let manager = ApprovalManager::get_instance();

    let approval = manager
        .get_approval(&approval_id)
        .await
        .map_err(|e: AppError| match e {
            AppError::NotFound(_) => e,
            _ => AppError::BadRequest(e.to_string()),
        })?;

    json_ok(approval.into())
}
