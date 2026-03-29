//! Event store for workflow execution events.
//!
//! Stores structured events that enable time-travel debugging and DAG reconstruction
//! of workflow executions across the adaptive memory system.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use ulid::Ulid;

use crate::db::{DatabasePool, DATABASE_POOL};
use crate::AppError;

/// A single workflow execution event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEvent {
    /// Unique event identifier.
    pub event_id: String,
    /// ISO 8601 timestamp when the event occurred.
    pub timestamp: String,
    /// Workflow instance this event belongs to.
    pub workflow_instance_id: String,
    /// Attempt number within the workflow instance.
    pub attempt_id: String,
    /// Parent span ID for trace context propagation.
    pub parent_span_id: Option<String>,
    /// Type of event (e.g., "span_start", "span_end", "decision", "action").
    pub event_type: String,
    /// JSON-encoded event payload with event-specific data.
    pub payload: serde_json::Value,
}

impl WorkflowEvent {
    /// Create a new workflow event.
    pub fn new(
        workflow_instance_id: impl Into<String>,
        attempt_id: impl Into<String>,
        parent_span_id: Option<String>,
        event_type: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            event_id: Ulid::new().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            workflow_instance_id: workflow_instance_id.into(),
            attempt_id: attempt_id.into(),
            parent_span_id,
            event_type: event_type.into(),
            payload,
        }
    }
}

/// Row type returned from database queries.
#[derive(Debug, Clone, sqlx::FromRow)]
struct EventRow {
    event_id: String,
    timestamp: String,
    workflow_instance_id: String,
    attempt_id: String,
    parent_span_id: Option<String>,
    event_type: String,
    payload: String,
}

pub struct EventStore;

impl EventStore {
    /// Persist a workflow event to the database.
    pub async fn append(event: &WorkflowEvent) -> Result<String, AppError> {
        let payload_json = serde_json::to_string(&event.payload)
            .map_err(|e| AppError::Serialization(e.to_string()))?;

        match DATABASE_POOL.get() {
            Some(DatabasePool::Postgres(pool)) => {
                sqlx::query(
                    r#"
                    INSERT INTO workflow_events
                      (event_id, timestamp, workflow_instance_id, attempt_id, parent_span_id, event_type, payload)
                    VALUES ($1, $2, $3, $4, $5, $6, $7)
                    "#,
                )
                .bind(&event.event_id)
                .bind(&event.timestamp)
                .bind(&event.workflow_instance_id)
                .bind(&event.attempt_id)
                .bind(&event.parent_span_id)
                .bind(&event.event_type)
                .bind(&payload_json)
                .execute(pool)
                .await
                .map_err(db_error)?;
            }
            Some(DatabasePool::Sqlite(pool)) => {
                sqlx::query(
                    r#"
                    INSERT INTO workflow_events
                      (event_id, timestamp, workflow_instance_id, attempt_id, parent_span_id, event_type, payload)
                    VALUES ($1, $2, $3, $4, $5, $6, $7)
                    "#,
                )
                .bind(&event.event_id)
                .bind(&event.timestamp)
                .bind(&event.workflow_instance_id)
                .bind(&event.attempt_id)
                .bind(&event.parent_span_id)
                .bind(&event.event_type)
                .bind(&payload_json)
                .execute(pool)
                .await
                .map_err(db_error)?;
            }
            None => {
                return Err(AppError::DatabaseConnection(
                    "database pool not initialized".into(),
                ))
            }
        }

        info!(
            event_id = %event.event_id,
            workflow_instance_id = %event.workflow_instance_id,
            event_type = %event.event_type,
            "Workflow event appended"
        );
        Ok(event.event_id.clone())
    }

    /// Retrieve all events for a given workflow instance, ordered by timestamp.
    pub async fn get_workflow_events(
        workflow_instance_id: &str,
    ) -> Result<Vec<WorkflowEvent>, AppError> {
        let rows: Vec<EventRow> = match DATABASE_POOL.get() {
            Some(DatabasePool::Postgres(pool)) => sqlx::query_as::<_, EventRow>(
                r#"
                    SELECT event_id, timestamp, workflow_instance_id, attempt_id,
                           parent_span_id, event_type, payload
                    FROM workflow_events
                    WHERE workflow_instance_id = $1
                    ORDER BY timestamp ASC
                    "#,
            )
            .bind(workflow_instance_id)
            .fetch_all(pool)
            .await
            .map_err(db_error)?,
            Some(DatabasePool::Sqlite(pool)) => sqlx::query_as::<_, EventRow>(
                r#"
                    SELECT event_id, timestamp, workflow_instance_id, attempt_id,
                           parent_span_id, event_type, payload
                    FROM workflow_events
                    WHERE workflow_instance_id = $1
                    ORDER BY timestamp ASC
                    "#,
            )
            .bind(workflow_instance_id)
            .fetch_all(pool)
            .await
            .map_err(db_error)?,
            None => {
                return Err(AppError::DatabaseConnection(
                    "database pool not initialized".into(),
                ))
            }
        };

        rows.into_iter()
            .map(|row| {
                let payload: serde_json::Value =
                    serde_json::from_str(&row.payload).map_err(|e| {
                        AppError::Deserialization(format!("invalid JSON in payload: {}", e))
                    })?;
                Ok(WorkflowEvent {
                    event_id: row.event_id,
                    timestamp: row.timestamp,
                    workflow_instance_id: row.workflow_instance_id,
                    attempt_id: row.attempt_id,
                    parent_span_id: row.parent_span_id,
                    event_type: row.event_type,
                    payload,
                })
            })
            .collect()
    }

    /// Get events within a time range for a workflow instance.
    pub async fn get_events_in_range(
        workflow_instance_id: &str,
        start: &str,
        end: &str,
    ) -> Result<Vec<WorkflowEvent>, AppError> {
        let rows: Vec<EventRow> = match DATABASE_POOL.get() {
            Some(DatabasePool::Postgres(pool)) => sqlx::query_as::<_, EventRow>(
                r#"
                    SELECT event_id, timestamp, workflow_instance_id, attempt_id,
                           parent_span_id, event_type, payload
                    FROM workflow_events
                    WHERE workflow_instance_id = $1
                      AND timestamp >= $2 AND timestamp <= $3
                    ORDER BY timestamp ASC
                    "#,
            )
            .bind(workflow_instance_id)
            .bind(start)
            .bind(end)
            .fetch_all(pool)
            .await
            .map_err(db_error)?,
            Some(DatabasePool::Sqlite(pool)) => sqlx::query_as::<_, EventRow>(
                r#"
                    SELECT event_id, timestamp, workflow_instance_id, attempt_id,
                           parent_span_id, event_type, payload
                    FROM workflow_events
                    WHERE workflow_instance_id = $1
                      AND timestamp >= $2 AND timestamp <= $3
                    ORDER BY timestamp ASC
                    "#,
            )
            .bind(workflow_instance_id)
            .bind(start)
            .bind(end)
            .fetch_all(pool)
            .await
            .map_err(db_error)?,
            None => {
                return Err(AppError::DatabaseConnection(
                    "database pool not initialized".into(),
                ))
            }
        };

        rows.into_iter()
            .map(|row| {
                let payload: serde_json::Value =
                    serde_json::from_str(&row.payload).map_err(|e| {
                        AppError::Deserialization(format!("invalid JSON in payload: {}", e))
                    })?;
                Ok(WorkflowEvent {
                    event_id: row.event_id,
                    timestamp: row.timestamp,
                    workflow_instance_id: row.workflow_instance_id,
                    attempt_id: row.attempt_id,
                    parent_span_id: row.parent_span_id,
                    event_type: row.event_type,
                    payload,
                })
            })
            .collect()
    }
}

fn db_error(error_value: sqlx::Error) -> AppError {
    error!("Event store repository failure: {}", error_value);
    AppError::DatabaseQuery(format!("Database error: {}", error_value))
}
