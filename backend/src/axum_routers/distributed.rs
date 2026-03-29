//! Distributed routes - epoch management and interrupt propagation

use axum::{routing::get, Router};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::distributed::{EpochManager, InterruptPropagator};
use crate::JsonResult;

/// Epoch status response
#[derive(Serialize, Debug, ToSchema)]
pub struct EpochStatusResponse {
    /// The current epoch value
    #[serde(rename = "current_epoch")]
    pub current_epoch: u64,
    /// The number of active contexts registered across all epochs
    #[serde(rename = "active_contexts")]
    pub active_contexts: usize,
}

/// Global instances for epoch management
/// These are initialized once and shared across all requests
static EPOCH_MANAGER: std::sync::OnceLock<Arc<EpochManager>> = std::sync::OnceLock::new();
static INTERRUPT_PROPAGATOR: std::sync::OnceLock<Arc<InterruptPropagator>> =
    std::sync::OnceLock::new();

/// Initialize the global epoch manager and interrupt propagator
pub fn init_distributed() {
    let _ = EPOCH_MANAGER.set(Arc::new(EpochManager::new()));
    let _ = INTERRUPT_PROPAGATOR.set(Arc::new(InterruptPropagator::new()));
}

/// Get the global epoch manager
pub fn epoch_manager() -> Arc<EpochManager> {
    EPOCH_MANAGER
        .get()
        .cloned()
        .unwrap_or_else(|| Arc::new(EpochManager::new()))
}

/// Get the global interrupt propagator
pub fn interrupt_propagator() -> Arc<InterruptPropagator> {
    INTERRUPT_PROPAGATOR
        .get()
        .cloned()
        .unwrap_or_else(|| Arc::new(InterruptPropagator::new()))
}

/// Get epoch status
#[utoipa::path(
    get,
    path = "/api/v1/distributed/epoch/status",
    tag = "Distributed",
    responses(
        (status = 200, description = "Epoch status returned", body = EpochStatusResponse),
        (status = 500, description = "Internal server error")
    )
)]
async fn get_epoch_status() -> JsonResult<EpochStatusResponse> {
    let manager = epoch_manager();
    let propagator = interrupt_propagator();

    let current_epoch = manager.current_epoch();
    let active_contexts = propagator.active_epoch_count();

    crate::json_ok(EpochStatusResponse {
        current_epoch,
        active_contexts,
    })
}

/// Create distributed routes
pub fn router() -> Router {
    Router::new().route("/api/v1/distributed/epoch/status", get(get_epoch_status))
}
