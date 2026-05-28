//! Planner Router - API Endpoints for Planner Agent Sandbox
//!
//! This module provides API endpoints for executing planner agent plans
//! in a virtual sandbox environment.

use axum::extract::{Extension, Json, State};
use axum::routing::post;
use axum::Router;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::ToSchema;

use crate::models::dry_run::{DryRunConfig, DryRunResult, ExecutionPlan};
use crate::runtime::planner_sandbox::PlannerSandbox;
use crate::AppError;

/// Application state for the planner sandbox.
pub struct PlannerState {
    pub sandbox: RwLock<PlannerSandbox>,
}

impl PlannerState {
    pub fn new() -> Self {
        Self {
            sandbox: RwLock::new(PlannerSandbox::new()),
        }
    }
}

impl Default for PlannerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Request body for dry-run execution.
#[derive(Debug, Deserialize, ToSchema)]
pub struct DryRunRequest {
    /// The execution plan to dry-run.
    pub plan: ExecutionPlan,
    /// Optional override for max steps.
    #[serde(default)]
    pub max_steps: Option<usize>,
    /// Optional override for timeout in seconds.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Response from dry-run execution.
#[derive(Debug, Serialize, ToSchema)]
pub struct DryRunResponse {
    /// Whether the dry-run completed successfully.
    pub success: bool,
    /// The result of the dry-run execution.
    pub result: DryRunResult,
}

/// Execute a plan in the virtual sandbox (dry-run mode).
///
/// This endpoint allows planner agents to simulate execution of their plans
/// without making actual side effects. Network I/O and filesystem access
/// are blocked and return errors in the result.
#[utoipa::path(
    post,
    path = "/api/v1/planner/sandbox/dry-run",
    request_body = DryRunRequest,
    responses(
        (status = 200, description = "Dry-run completed", body = DryRunResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "planner"
)]
pub async fn dry_run(
    State(state): State<Arc<PlannerState>>,
    Json(request): Json<DryRunRequest>,
) -> Result<Json<DryRunResponse>, AppError> {
    // Build config with optional overrides
    let mut config = DryRunConfig::default();
    if let Some(max_steps) = request.max_steps {
        config.max_steps = max_steps;
    }
    if let Some(timeout_secs) = request.timeout_secs {
        config.timeout_secs = timeout_secs;
    }

    // Create a sandbox with the given config
    let sandbox = PlannerSandbox::with_config(config);

    // Execute the dry-run
    let result = sandbox.dry_run(&request.plan);

    let success = !result.has_errors();

    Ok(Json(DryRunResponse { success, result }))
}

/// Reset the sandbox state (clear recorded effects).
pub async fn reset_sandbox(
    State(state): State<Arc<PlannerState>>,
) -> Result<crate::JsonResult<serde_json::Value>, AppError> {
    let mut sandbox = state.sandbox.write().await;
    sandbox.reset();
    Ok(crate::json_ok(serde_json::json!({
        "status": "ok",
        "message": "Sandbox state reset"
    })))
}

/// Create a new planner router with the given state.
pub fn router(state: Arc<PlannerState>) -> Router {
    Router::new()
        .route("/sandbox/dry-run", post(dry_run))
        .route("/sandbox/reset", post(reset_sandbox))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::dry_run::{PlanMetadata, PlanStep};

    fn create_test_plan() -> ExecutionPlan {
        ExecutionPlan {
            steps: vec![
                PlanStep {
                    tool: "noop".to_string(),
                    parameters: serde_json::json!({}),
                    preconditions: vec![],
                    postconditions: vec![],
                },
                PlanStep {
                    tool: "log".to_string(),
                    parameters: serde_json::json!({"message": "Hello"}),
                    preconditions: vec![],
                    postconditions: vec![],
                },
            ],
            metadata: PlanMetadata::default(),
        }
    }

    #[test]
    fn test_dry_run_request_deserialization() {
        let json = serde_json::json!({
            "plan": {
                "steps": [
                    {"tool": "noop", "parameters": {}}
                ]
            }
        });

        let request: DryRunRequest =
            serde_json::from_value(json).expect("test JSON should deserialize");
        assert_eq!(request.plan.steps.len(), 1);
    }

    #[tokio::test]
    async fn test_dry_run_endpoint() {
        let state = Arc::new(PlannerState::new());
        let request = DryRunRequest {
            plan: create_test_plan(),
            max_steps: Some(10),
            timeout_secs: Some(30),
        };

        let response = dry_run(State(state), Json(request))
            .await
            .expect("dry_run handler should succeed");
        assert!(response.success);
        assert_eq!(response.result.execution_trace.steps.len(), 2);
    }

    #[tokio::test]
    async fn test_reset_sandbox() {
        let state = Arc::new(PlannerState::new());
        let result = reset_sandbox(State(state))
            .await
            .expect("reset_sandbox handler should succeed");
        let json = result.expect("reset_sandbox should return valid JSON").0;
        assert!(json.get("status").is_some());
    }
}
