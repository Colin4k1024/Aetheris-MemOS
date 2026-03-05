//! Persistence for decision traces (v0.3 explainability).

use tracing::{error, info};
use ulid::Ulid;

use crate::db::pool;
use crate::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DecisionTraceRow {
    pub trace_id: String,
    pub task_id: String,
    pub trace_json: String,
    pub created_at: String,
}

pub struct DecisionTraceRepository;

impl DecisionTraceRepository {
    /// Persist a decision trace (JSON string).
    pub async fn create(task_id: &str, trace_json: &str) -> Result<String, AppError> {
        let trace_id = Ulid::new().to_string();
        let pool = pool();
        sqlx::query(
            r#"
            INSERT INTO decision_trace (trace_id, task_id, trace_json)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(&trace_id)
        .bind(task_id)
        .bind(trace_json)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create decision trace: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;
        info!("Created decision trace: {} for task: {}", trace_id, task_id);
        Ok(trace_id)
    }

    /// Get traces by task_id, newest first.
    pub async fn get_by_task_id(
        task_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<DecisionTraceRow>, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(50);
        let rows = sqlx::query_as::<_, DecisionTraceRow>(
            r#"
            SELECT trace_id, task_id, trace_json, created_at::text
            FROM decision_trace
            WHERE task_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get decision traces by task_id: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;
        Ok(rows)
    }

    /// Get recent traces, optionally by time range.
    pub async fn get_recent(
        limit: Option<i32>,
        start_time: Option<&str>,
        end_time: Option<&str>,
    ) -> Result<Vec<DecisionTraceRow>, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(100);
        let rows = if let (Some(start), Some(end)) = (start_time, end_time) {
            sqlx::query_as::<_, DecisionTraceRow>(
                r#"
                SELECT trace_id, task_id, trace_json, created_at::text
                FROM decision_trace
                WHERE created_at >= $1::timestamptz AND created_at <= $2::timestamptz
                ORDER BY created_at DESC
                LIMIT $3
                "#,
            )
            .bind(start)
            .bind(end)
            .bind(limit)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_as::<_, DecisionTraceRow>(
                r#"
                SELECT trace_id, task_id, trace_json, created_at::text
                FROM decision_trace
                ORDER BY created_at DESC
                LIMIT $1
                "#,
            )
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        .map_err(|e| {
            error!("Failed to get decision traces: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;
        Ok(rows)
    }
}
