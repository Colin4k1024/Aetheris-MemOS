//! Time-travel debugging API endpoints.
//!
//! Provides workflow event replay and DAG reconstruction endpoints for
//! visualizing and stepping through historical workflow executions.

use axum::extract::Path;
use axum::routing::get;
use axum::Json;
use axum::Router;
use serde::{Deserialize, Serialize};

use crate::db::event_store::{EventStore, WorkflowEvent};
use crate::{json_ok, JsonResult};

/// Application state shared across routing.
pub type TracingState = std::collections::HashMap<String, String>;

/// DAG node in the workflow visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNode {
    pub id: String,
    pub event_type: String,
    pub label: String,
    pub status: String,
    pub timestamp: String,
    pub attempt_id: String,
    pub span_id: Option<String>,
    pub parent_span_id: Option<String>,
}

/// DAG edge connecting two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagEdge {
    pub from: String,
    pub to: String,
}

/// Complete DAG structure for frontend visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDag {
    pub workflow_instance_id: String,
    pub nodes: Vec<DagNode>,
    pub edges: Vec<DagEdge>,
}

/// GET /api/v1/tracing/workflow/{workflow_instance_id}/events
///
/// Returns all workflow events for a given workflow instance.
pub async fn get_workflow_events(
    Path(workflow_instance_id): Path<String>,
) -> JsonResult<Vec<WorkflowEvent>> {
    let events = EventStore::get_workflow_events(&workflow_instance_id).await?;
    json_ok(events)
}

/// GET /api/v1/tracing/workflow/{workflow_instance_id}/dag
///
/// Returns the DAG structure for a workflow instance, suitable for
/// frontend visualization with nodes and edges.
pub async fn get_workflow_dag(
    Path(workflow_instance_id): Path<String>,
) -> JsonResult<WorkflowDag> {
    let events = EventStore::get_workflow_events(&workflow_instance_id).await?;

    let mut nodes = Vec::with_capacity(events.len());
    let mut edges = Vec::new();
    let mut span_to_node: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for event in &events {
        let node = DagNode {
            id: event.event_id.clone(),
            event_type: event.event_type.clone(),
            label: format!(
                "{} ({})",
                event.event_type,
                event.timestamp.split('T').next().unwrap_or(&event.timestamp)
            ),
            status: status_from_event_type(&event.event_type),
            timestamp: event.timestamp.clone(),
            attempt_id: event.attempt_id.clone(),
            span_id: Some(event.event_id.clone()),
            parent_span_id: event.parent_span_id.clone(),
        };

        if let Some(ref parent_span) = event.parent_span_id {
            if let Some(parent_node) = span_to_node.get(parent_span) {
                edges.push(DagEdge {
                    from: parent_node.clone(),
                    to: event.event_id.clone(),
                });
            }
        }

        span_to_node.insert(event.event_id.clone(), event.event_id.clone());
        nodes.push(node);
    }

    json_ok(WorkflowDag {
        workflow_instance_id,
        nodes,
        edges,
    })
}

/// Derive a display status from event type.
fn status_from_event_type(event_type: &str) -> String {
    match event_type {
        "span_start" | "decision_start" | "action_start" => "running".to_string(),
        "span_end" | "decision_end" | "action_end" => "completed".to_string(),
        "error" | "span_error" => "error".to_string(),
        "decision" => "decision".to_string(),
        _ => "pending".to_string(),
    }
}

/// Create the tracing router with all time-travel debugging endpoints.
pub fn router() -> Router {
    Router::new()
        .route(
            "/workflow/{workflow_instance_id}/events",
            get(get_workflow_events),
        )
        .route(
            "/workflow/{workflow_instance_id}/dag",
            get(get_workflow_dag),
        )
}
