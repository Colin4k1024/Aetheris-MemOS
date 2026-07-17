//! Persistence for decision traces (v0.3 explainability).

use tracing::{error, info};
use ulid::Ulid;

use crate::db::{DatabasePool, DATABASE_POOL};
use crate::tenant::TenantId;
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
    // W1.4: Application-layer tenant enforcement (no RLS on this table).
    // All reads and writes scope by tenant_id to prevent cross-tenant leakage
    // even though decision_trace has no row-level security policy.

    /// Persist a decision trace (JSON string).
    pub async fn create(
        tenant_id: &TenantId,
        task_id: &str,
        trace_json: &str,
    ) -> Result<String, AppError> {
        let trace_id = Ulid::new().to_string();
        match DATABASE_POOL.get() {
            Some(DatabasePool::Postgres(pool)) => {
                sqlx::query(
                    r#"
                    INSERT INTO decision_trace (trace_id, tenant_id, task_id, trace_json)
                    VALUES ($1, $2, $3, $4)
                    "#,
                )
                .bind(&trace_id)
                .bind(tenant_id.as_str())
                .bind(task_id)
                .bind(trace_json)
                .execute(pool)
                .await
                .map_err(db_error)?;
            }
            Some(DatabasePool::Sqlite(pool)) => {
                sqlx::query(
                    r#"
                    INSERT INTO decision_trace (trace_id, tenant_id, task_id, trace_json)
                    VALUES ($1, $2, $3, $4)
                    "#,
                )
                .bind(&trace_id)
                .bind(tenant_id.as_str())
                .bind(task_id)
                .bind(trace_json)
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
            "Created decision trace: {} for task: {} (tenant: {})",
            trace_id,
            task_id,
            tenant_id.as_str()
        );
        Ok(trace_id)
    }

    /// Get traces by task_id, newest first.
    // W1.4: Application-layer tenant enforcement (no RLS on this table).
    pub async fn get_by_task_id(
        tenant_id: &TenantId,
        task_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<DecisionTraceRow>, AppError> {
        let limit = limit.unwrap_or(50);
        let rows = match DATABASE_POOL.get() {
            Some(DatabasePool::Postgres(pool)) => sqlx::query_as::<_, DecisionTraceRow>(
                r#"
                    SELECT trace_id, task_id, trace_json, created_at::text AS created_at
                    FROM decision_trace
                    WHERE tenant_id = $1 AND task_id = $2
                    ORDER BY created_at DESC
                    LIMIT $3
                    "#,
            )
            .bind(tenant_id.as_str())
            .bind(task_id)
            .bind(limit)
            .fetch_all(pool)
            .await
            .map_err(db_error)?,
            Some(DatabasePool::Sqlite(pool)) => sqlx::query_as::<_, DecisionTraceRow>(
                r#"
                    SELECT trace_id, task_id, trace_json, created_at
                    FROM decision_trace
                    WHERE tenant_id = $1 AND task_id = $2
                    ORDER BY created_at DESC
                    LIMIT $3
                    "#,
            )
            .bind(tenant_id.as_str())
            .bind(task_id)
            .bind(limit)
            .fetch_all(pool)
            .await
            .map_err(db_error)?,
            None => {
                return Err(AppError::DatabaseConnection(
                    "database pool not initialized".into(),
                ))
            }
        };

        Ok(rows)
    }

    /// Get recent traces, optionally by time range.
    // W1.4: Application-layer tenant enforcement (no RLS on this table).
    pub async fn get_recent(
        tenant_id: &TenantId,
        limit: Option<i32>,
        start_time: Option<&str>,
        end_time: Option<&str>,
    ) -> Result<Vec<DecisionTraceRow>, AppError> {
        let limit = limit.unwrap_or(100);
        let rows = match DATABASE_POOL.get() {
            Some(DatabasePool::Postgres(pool)) => {
                if let (Some(start), Some(end)) = (start_time, end_time) {
                    sqlx::query_as::<_, DecisionTraceRow>(
                        r#"
                        SELECT trace_id, task_id, trace_json, created_at::text AS created_at
                        FROM decision_trace
                        WHERE tenant_id = $1
                          AND created_at >= $2::timestamptz
                          AND created_at <= $3::timestamptz
                        ORDER BY created_at DESC
                        LIMIT $4
                        "#,
                    )
                    .bind(tenant_id.as_str())
                    .bind(start)
                    .bind(end)
                    .bind(limit)
                    .fetch_all(pool)
                    .await
                } else {
                    sqlx::query_as::<_, DecisionTraceRow>(
                        r#"
                        SELECT trace_id, task_id, trace_json, created_at::text AS created_at
                        FROM decision_trace
                        WHERE tenant_id = $1
                        ORDER BY created_at DESC
                        LIMIT $2
                        "#,
                    )
                    .bind(tenant_id.as_str())
                    .bind(limit)
                    .fetch_all(pool)
                    .await
                }
                .map_err(db_error)?
            }
            Some(DatabasePool::Sqlite(pool)) => {
                if let (Some(start), Some(end)) = (start_time, end_time) {
                    sqlx::query_as::<_, DecisionTraceRow>(
                        r#"
                        SELECT trace_id, task_id, trace_json, created_at
                        FROM decision_trace
                        WHERE tenant_id = $1 AND created_at >= $2 AND created_at <= $3
                        ORDER BY created_at DESC
                        LIMIT $4
                        "#,
                    )
                    .bind(tenant_id.as_str())
                    .bind(start)
                    .bind(end)
                    .bind(limit)
                    .fetch_all(pool)
                    .await
                } else {
                    sqlx::query_as::<_, DecisionTraceRow>(
                        r#"
                        SELECT trace_id, task_id, trace_json, created_at
                        FROM decision_trace
                        WHERE tenant_id = $1
                        ORDER BY created_at DESC
                        LIMIT $2
                        "#,
                    )
                    .bind(tenant_id.as_str())
                    .bind(limit)
                    .fetch_all(pool)
                    .await
                }
                .map_err(db_error)?
            }
            None => {
                return Err(AppError::DatabaseConnection(
                    "database pool not initialized".into(),
                ))
            }
        };
        Ok(rows)
    }
}

fn db_error(error_value: sqlx::Error) -> AppError {
    error!("Decision trace repository failure: {}", error_value);
    AppError::DatabaseQuery(format!("Database error: {}", error_value))
}
