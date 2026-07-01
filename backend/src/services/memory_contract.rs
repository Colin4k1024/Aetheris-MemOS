//! Agent-facing memory contract helpers.

use serde::{Deserialize, Serialize};

use crate::db::{
    ltm::LTMRepository, memory_feedback::MemoryFeedbackRepository, pool, stm::STMRepository,
};
use crate::tenant::TenantId;
use crate::AppError;

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct MemoryFeedbackRequest {
    #[serde(rename = "memoryId")]
    pub memory_id: String,
    pub useful: bool,
    pub query: Option<String>,
    #[serde(rename = "traceId")]
    pub trace_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct MemoryFeedbackRecord {
    #[serde(rename = "feedbackId")]
    pub feedback_id: String,
    #[serde(rename = "memoryId")]
    pub memory_id: String,
    pub useful: bool,
    pub query: Option<String>,
    #[serde(rename = "traceId")]
    pub trace_id: Option<String>,
    pub metadata: serde_json::Value,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct MemoryFeedbackResponse {
    pub success: bool,
    pub feedback: MemoryFeedbackRecord,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct MemoryForgetRequest {
    #[serde(rename = "memoryId")]
    pub memory_id: String,
    pub layer: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct MemoryForgetResponse {
    pub success: bool,
    #[serde(rename = "memoryId")]
    pub memory_id: String,
    pub layer: String,
    pub deleted: bool,
    pub message: String,
}

fn build_feedback_record(
    feedback_id: String,
    req: MemoryFeedbackRequest,
    created_at: i64,
) -> Result<MemoryFeedbackRecord, AppError> {
    if req.memory_id.trim().is_empty() {
        return Err(AppError::BadRequest("memoryId is required".to_string()));
    }

    Ok(MemoryFeedbackRecord {
        feedback_id,
        memory_id: req.memory_id,
        useful: req.useful,
        query: req.query,
        trace_id: req.trace_id,
        metadata: req.metadata.unwrap_or_else(|| serde_json::json!({})),
        created_at,
    })
}

pub async fn record_feedback(
    tenant_id: &TenantId,
    req: MemoryFeedbackRequest,
) -> Result<MemoryFeedbackResponse, AppError> {
    let metadata = req
        .metadata
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));
    let row = MemoryFeedbackRepository::create(
        tenant_id,
        req.memory_id.trim(),
        req.useful,
        req.query.as_deref(),
        req.trace_id.as_deref(),
        &metadata,
    )
    .await?;

    let created_at = chrono::DateTime::parse_from_rfc3339(&row.created_at)
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|_| chrono::Utc::now().timestamp());
    let feedback = MemoryFeedbackRecord {
        feedback_id: row.feedback_id,
        memory_id: row.memory_id,
        useful: row.useful,
        query: row.query,
        trace_id: row.trace_id,
        metadata: serde_json::from_str(&row.metadata_json).unwrap_or(metadata),
        created_at,
    };

    Ok(MemoryFeedbackResponse {
        success: true,
        feedback,
    })
}

pub async fn forget_memory(
    tenant_id: &TenantId,
    req: MemoryForgetRequest,
) -> Result<MemoryForgetResponse, AppError> {
    if req.memory_id.trim().is_empty() {
        return Err(AppError::BadRequest("memoryId is required".to_string()));
    }

    let layer = req.layer.to_lowercase();
    let (deleted, message) = match layer.as_str() {
        "stm" => {
            let deleted = STMRepository::delete_session(pool(), tenant_id, &req.memory_id).await?;
            let message = if deleted {
                "STM session deleted".to_string()
            } else {
                "STM session not found or not accessible".to_string()
            };
            (deleted, message)
        }
        "ltm" => {
            let deleted =
                LTMRepository::soft_delete_entry(pool(), tenant_id, &req.memory_id).await?;
            let message = if deleted {
                "LTM entry deprecated".to_string()
            } else {
                "LTM entry not found or not accessible".to_string()
            };
            (deleted, message)
        }
        "kg" | "mm" => {
            return Err(AppError::BadRequest(format!(
                "forget is not implemented for layer {} yet",
                layer
            )));
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "Invalid layer: {}",
                req.layer
            )))
        }
    };

    Ok(MemoryForgetResponse {
        success: true,
        memory_id: req.memory_id,
        layer,
        deleted,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_feedback_returns_standard_response() {
        let record = build_feedback_record(
            "feedback-1".to_string(),
            MemoryFeedbackRequest {
                memory_id: "mem-1".to_string(),
                useful: true,
                query: Some("preference".to_string()),
                trace_id: Some("trace-1".to_string()),
                metadata: Some(serde_json::json!({"source": "test"})),
            },
            123,
        )
        .expect("feedback should be built");

        assert_eq!(record.feedback_id, "feedback-1");
        assert_eq!(record.memory_id, "mem-1");
        assert!(record.useful);
        assert_eq!(record.trace_id.as_deref(), Some("trace-1"));
    }

    #[test]
    fn record_feedback_rejects_empty_memory_id() {
        let err = build_feedback_record(
            "feedback-1".to_string(),
            MemoryFeedbackRequest {
                memory_id: " ".to_string(),
                useful: false,
                query: None,
                trace_id: None,
                metadata: None,
            },
            123,
        )
        .expect_err("empty memory id should fail");

        assert!(matches!(err, AppError::BadRequest(_)));
    }
}
