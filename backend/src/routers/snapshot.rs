//! Snapshot Router
//!
//! API endpoints for context snapshot management (Oris Integration).

use axum::extract::Path;
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;
use validator::Validate;

use crate::integrations::oris::{Checkpoint, OrisTaskState};
use crate::services::context_snapshot::ContextSnapshotService;
use crate::{json_ok, JsonResult};

/// Create task request
#[derive(Deserialize, ToSchema, Validate)]
pub struct CreateTaskRequest {
    #[serde(rename = "agentId")]
    pub agent_id: String,
}

/// Create snapshot request
#[derive(Deserialize, ToSchema, Validate)]
pub struct CreateSnapshotRequest {
    #[serde(rename = "taskId")]
    pub task_id: String,
    #[serde(rename = "workingMemory")]
    pub working_memory: Option<Vec<String>>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// Restore snapshot request
#[derive(Deserialize, ToSchema, Validate)]
pub struct RestoreSnapshotRequest {
    #[serde(rename = "taskId")]
    pub task_id: String,
    #[serde(rename = "snapshotId")]
    pub snapshot_id: Option<String>,
}

/// Create checkpoint request
#[derive(Deserialize, ToSchema, Validate)]
pub struct CreateCheckpointRequest {
    #[serde(rename = "taskId")]
    pub task_id: String,
    pub description: Option<String>,
}

/// Rollback request
#[derive(Deserialize, ToSchema, Validate)]
pub struct RollbackRequest {
    #[serde(rename = "taskId")]
    pub task_id: String,
    #[serde(rename = "checkpointId")]
    pub checkpoint_id: String,
}

/// Create a new task
pub async fn create_task(
    Json(req): Json<CreateTaskRequest>,
) -> JsonResult<OrisTaskState> {
    req.validate()?;
    info!("Creating task for agent: {}", req.agent_id);

    let service = ContextSnapshotService::new();
    let task = service.create_task(&req.agent_id).await?;

    json_ok(task)
}

/// Create a snapshot
pub async fn create_snapshot(
    Json(req): Json<CreateSnapshotRequest>,
) -> JsonResult<crate::integrations::oris::ContextSnapshot> {
    req.validate()?;
    info!("Creating snapshot for task: {}", req.task_id);

    let service = ContextSnapshotService::new();
    let working_memory = req
        .working_memory
        .map(|ids| ids.into_iter().map(|s| crate::kernel::types::MemoryId(s)).collect());

    let snapshot = service
        .create_snapshot(&req.task_id, working_memory.unwrap_or_default(), req.metadata)
        .await?;

    json_ok(snapshot)
}

/// Restore a snapshot
pub async fn restore_snapshot(
    Json(req): Json<RestoreSnapshotRequest>,
) -> JsonResult<OrisTaskState> {
    req.validate()?;
    info!("Restoring snapshot for task: {}", req.task_id);

    let service = ContextSnapshotService::new();
    let task = service
        .restore_snapshot(&req.task_id, req.snapshot_id)
        .await?;

    json_ok(task)
}

/// Create a checkpoint
pub async fn create_checkpoint(
    Json(req): Json<CreateCheckpointRequest>,
) -> JsonResult<Checkpoint> {
    req.validate()?;
    info!("Creating checkpoint for task: {}", req.task_id);

    let service = ContextSnapshotService::new();
    let checkpoint = service
        .create_checkpoint(&req.task_id, req.description)
        .await?;

    json_ok(checkpoint)
}

/// Rollback to checkpoint
pub async fn rollback_to_checkpoint(
    Json(req): Json<RollbackRequest>,
) -> JsonResult<OrisTaskState> {
    req.validate()?;
    info!(
        "Rolling back task {} to checkpoint {}",
        req.task_id, req.checkpoint_id
    );

    let service = ContextSnapshotService::new();
    let task = service
        .rollback_to_checkpoint(&req.task_id, &req.checkpoint_id)
        .await?;

    json_ok(task)
}

/// Get task state
pub async fn get_task(
    Path(task_id): Path<String>,
) -> JsonResult<Option<OrisTaskState>> {
    info!("Getting task: {}", task_id);

    let service = ContextSnapshotService::new();
    let task = service.get_task(&task_id).await?;

    json_ok(task)
}

/// List checkpoints for a task
pub async fn list_checkpoints(
    Path(task_id): Path<String>,
) -> JsonResult<Vec<Checkpoint>> {
    info!("Listing checkpoints for task: {}", task_id);

    let service = ContextSnapshotService::new();
    let checkpoints = service.list_checkpoints(&task_id).await?;

    json_ok(checkpoints)
}
