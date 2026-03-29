//! Distributed System API Routes
//!
//! Provides endpoints for managing the sub-agent pool and
//! querying workflow signals.

use axum::extract::Path;
use axum::Json;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::distributed::signaling_bus::{SignalingBus, WorkflowSignal};
use crate::runtime::subagent_pool::SubagentPool;
use crate::{json_ok, JsonResult};

/// Pool status response.
#[derive(Serialize, ToSchema)]
pub struct PoolStatusResponse {
    pub total_slots: usize,
    pub idle_slots: usize,
    pub busy_slots: usize,
    pub detached_slots: usize,
    pub slots: Vec<SlotInfo>,
}

/// Slot information in response.
#[derive(Serialize, ToSchema)]
pub struct SlotInfo {
    pub slot_id: usize,
    pub workflow_id: Option<String>,
    pub status: String,
}

/// Signal response for a workflow.
#[derive(Serialize, ToSchema)]
pub struct WorkflowSignalResponse {
    pub workflow_id: String,
    pub signals: Vec<SignalInfo>,
}

/// Signal information.
#[derive(Serialize, ToSchema)]
pub struct SignalInfo {
    pub signal_type: String,
    pub target_workflow_id: String,
    pub timestamp: String,
    pub parent_workflow_id: String,
}

// Global instances for the distributed system
static SIGNALING_BUS: Lazy<Arc<SignalingBus>> = Lazy::new(|| Arc::new(SignalingBus::new()));
static SUBAGENT_POOL: Lazy<Arc<SubagentPool>> = Lazy::new(|| Arc::new(SubagentPool::new(16)));

/// Get the current pool status.
#[utoipa::path(
    get,
    path = "/api/v1/distributed/pool/status",
    responses(
        (status = 200, description = "Pool status retrieved", body = PoolStatusResponse)
    )
)]
pub async fn get_pool_status() -> JsonResult<PoolStatusResponse> {
    let status = SUBAGENT_POOL.status().await;

    let slots = status
        .slots
        .iter()
        .map(|s| SlotInfo {
            slot_id: s.slot_id,
            workflow_id: s.workflow_id.clone(),
            status: format!("{:?}", s.status).to_lowercase(),
        })
        .collect();

    json_ok(PoolStatusResponse {
        total_slots: status.total_slots,
        idle_slots: status.idle_slots,
        busy_slots: status.busy_slots,
        detached_slots: status.detached_slots,
        slots,
    })
}

/// Get signals for a specific workflow.
#[utoipa::path(
    get,
    path = "/api/v1/distributed/signals/{workflow_id}",
    params(
        ("workflow_id" = String, Path, description = "Workflow ID to get signals for")
    ),
    responses(
        (status = 200, description = "Signals retrieved", body = WorkflowSignalResponse),
        (status = 404, description = "Workflow not found")
    )
)]
pub async fn get_signals(
    Path(workflow_id): Path<String>,
) -> JsonResult<WorkflowSignalResponse> {
    let signals = SIGNALING_BUS.get_parent_signals(&workflow_id);

    let signal_infos = signals
        .into_iter()
        .map(|(signal, metadata)| {
            let target_id = signal.workflow_id().unwrap_or("unknown").to_string();
            SignalInfo {
                signal_type: metadata.signal_type,
                target_workflow_id: target_id,
                timestamp: metadata.timestamp.to_rfc3339(),
                parent_workflow_id: metadata.parent_workflow_id,
            }
        })
        .collect();

    json_ok(WorkflowSignalResponse {
        workflow_id,
        signals: signal_infos,
    })
}

/// Allocate slots in the pool.
#[derive(Deserialize, ToSchema)]
pub struct AllocateRequest {
    pub count: usize,
}

/// Allocate response.
#[derive(Serialize, ToSchema)]
pub struct AllocateResponse {
    pub slot_ids: Vec<usize>,
    pub allocated_count: usize,
}

/// Allocate slots for sub-agents.
pub async fn allocate_slots(
    Json(request): Json<AllocateRequest>,
) -> JsonResult<AllocateResponse> {
    let slot_ids = SUBAGENT_POOL.allocate(request.count).await;
    json_ok(AllocateResponse {
        allocated_count: slot_ids.len(),
        slot_ids,
    })
}

/// Release slots back to the pool.
#[derive(Deserialize, ToSchema)]
pub struct ReleaseRequest {
    pub slot_ids: Vec<usize>,
}

/// Release response.
#[derive(Serialize, ToSchema)]
pub struct ReleaseResponse {
    pub released_count: usize,
}

/// Release previously allocated slots.
pub async fn release_slots(
    Json(request): Json<ReleaseRequest>,
) -> JsonResult<ReleaseResponse> {
    SUBAGENT_POOL.release(&request.slot_ids).await;
    json_ok(ReleaseResponse {
        released_count: request.slot_ids.len(),
    })
}

/// Publish a signal to the bus.
#[derive(Deserialize)]
pub struct PublishSignalRequest {
    pub signal: WorkflowSignal,
    pub parent_workflow_id: String,
}

/// Publish response.
#[derive(Serialize, ToSchema)]
pub struct PublishSignalResponse {
    pub published: bool,
}

/// Publish a workflow signal.
pub async fn publish_signal(
    Json(request): Json<PublishSignalRequest>,
) -> JsonResult<PublishSignalResponse> {
    SIGNALING_BUS.publish(request.signal, &request.parent_workflow_id);
    json_ok(PublishSignalResponse { published: true })
}

// Re-export the global instances for use in tests or other modules
pub fn signaling_bus() -> Arc<SignalingBus> {
    SIGNALING_BUS.clone()
}

pub fn subagent_pool() -> Arc<SubagentPool> {
    SUBAGENT_POOL.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_pool_status() {
        let response = get_pool_status().await.unwrap();
        assert!(response.total_slots >= 1);
        assert!(response.idle_slots <= response.total_slots);
        assert_eq!(response.total_slots, response.idle_slots + response.busy_slots + response.detached_slots);
    }

    #[tokio::test]
    async fn test_allocate_and_release() {
        // Allocate 3 slots
        let alloc_request = AllocateRequest { count: 3 };
        let alloc_response = allocate_slots(Json(alloc_request)).await.unwrap();
        assert_eq!(alloc_response.allocated_count, 3);

        // Release them
        let release_request = ReleaseRequest {
            slot_ids: alloc_response.slot_ids.clone(),
        };
        let release_response = release_slots(Json(release_request)).await.unwrap();
        assert_eq!(release_response.released_count, 3);
    }

    #[tokio::test]
    async fn test_publish_and_get_signals() {
        let signal = WorkflowSignal::SubagentSpawn {
            child_workflow_id: "child-1".to_string(),
        };

        let publish_request = PublishSignalRequest {
            signal,
            parent_workflow_id: "test-wf".to_string(),
        };
        let _ = publish_signal(Json(publish_request)).await;

        let response = get_signals(Path("test-wf".to_string())).await.unwrap();
        assert_eq!(response.signals.len(), 1);
        assert_eq!(response.signals[0].signal_type, "SubagentSpawn");
    }
}
