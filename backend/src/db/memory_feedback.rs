use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::error;

use crate::db::pool;
use crate::tenant::TenantId;
use crate::AppError;

pub struct MemoryFeedbackRepository;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct MemoryFeedbackRow {
    pub feedback_id: String,
    pub tenant_id: String,
    pub memory_id: String,
    pub useful: bool,
    pub query: Option<String>,
    pub trace_id: Option<String>,
    pub metadata_json: String,
    pub created_at: String,
}

impl MemoryFeedbackRepository {
    pub async fn create(
        tenant_id: &TenantId,
        memory_id: &str,
        useful: bool,
        query: Option<&str>,
        trace_id: Option<&str>,
        metadata: &serde_json::Value,
    ) -> Result<MemoryFeedbackRow, AppError> {
        let feedback_id = ulid::Ulid::new().to_string();
        let metadata_json = serde_json::to_string(metadata)
            .map_err(|e| AppError::Serialization(format!("Invalid feedback metadata: {e}")))?;

        let row = sqlx::query_as::<_, MemoryFeedbackRow>(
            r#"
            INSERT INTO memory_feedback (
                feedback_id, tenant_id, memory_id, useful, query, trace_id, metadata_json
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING feedback_id, tenant_id, memory_id, useful, query, trace_id,
                      metadata_json, created_at::text as created_at
            "#,
        )
        .bind(&feedback_id)
        .bind(tenant_id.as_str())
        .bind(memory_id)
        .bind(useful)
        .bind(query)
        .bind(trace_id)
        .bind(&metadata_json)
        .fetch_one(pool())
        .await
        .map_err(|e| {
            error!("Failed to persist memory feedback: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(row)
    }

    pub async fn list_by_memory(
        tenant_id: &TenantId,
        memory_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<MemoryFeedbackRow>, AppError> {
        let rows = sqlx::query_as::<_, MemoryFeedbackRow>(
            r#"
            SELECT feedback_id, tenant_id, memory_id, useful, query, trace_id,
                   metadata_json, created_at::text as created_at
            FROM memory_feedback
            WHERE tenant_id = $1 AND memory_id = $2
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(memory_id)
        .bind(limit.unwrap_or(50))
        .fetch_all(pool())
        .await
        .map_err(|e| {
            error!("Failed to list memory feedback: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(rows)
    }
}
