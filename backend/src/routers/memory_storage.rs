use salvo::oapi::extract::*;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::info;
use validator::Validate;

use crate::services::memory_storage::MemoryStorageService;
use crate::{json_ok, JsonResult};

/// 存储短期记忆请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct StoreSTMRequest {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "sessionType")]
    pub session_type: String,
    pub role: String,
    pub content: String,
    #[serde(rename = "maxContextLength")]
    pub max_context_length: Option<i32>,
    #[serde(rename = "retentionHours")]
    pub retention_hours: Option<i32>,
}

/// 存储短期记忆响应
#[derive(Serialize, ToSchema)]
pub struct StoreSTMResponse {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "messageId")]
    pub message_id: String,
}

/// 存储长期记忆请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct StoreLTMRequest {
    #[serde(rename = "sourceId")]
    pub source_id: String,
    #[serde(rename = "sourceType")]
    pub source_type: String,
    pub content: String,
    pub title: Option<String>,
}

/// 存储长期记忆响应
#[derive(Serialize, ToSchema)]
pub struct StoreLTMResponse {
    #[serde(rename = "entryId")]
    pub entry_id: String,
}

/// 手动转移请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct TransferRequest {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "messageCountThreshold")]
    pub message_count_threshold: Option<i32>,
}

/// 手动转移响应
#[derive(Serialize, ToSchema)]
pub struct TransferResponse {
    #[serde(rename = "entryIds")]
    pub entry_ids: Vec<String>,
}

/// 批量存储请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct BatchStoreLTMRequest {
    pub entries: Vec<BatchStoreEntry>,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct BatchStoreEntry {
    #[serde(rename = "sourceId")]
    pub source_id: String,
    #[serde(rename = "sourceType")]
    pub source_type: String,
    pub content: String,
    pub title: Option<String>,
}

/// 批量存储响应
#[derive(Serialize, ToSchema)]
pub struct BatchStoreLTMResponse {
    #[serde(rename = "entryIds")]
    pub entry_ids: Vec<String>,
}

/// 存储短期记忆
#[endpoint(tags("memory-storage"))]
pub async fn store_stm(
    body: JsonBody<StoreSTMRequest>,
) -> JsonResult<StoreSTMResponse> {
    let req = body.into_inner();
    req.validate()?;

    info!(
        "Storing STM: user_id={}, agent_id={}, session_type={}",
        req.user_id, req.agent_id, req.session_type
    );

    let (session_id, message_id) = MemoryStorageService::store_stm(
        &req.user_id,
        &req.agent_id,
        &req.session_type,
        &req.role,
        &req.content,
        req.max_context_length.unwrap_or(4096),
        req.retention_hours.unwrap_or(24),
    )
    .await?;

    json_ok(StoreSTMResponse {
        session_id,
        message_id,
    })
}

/// 存储长期记忆
#[endpoint(tags("memory-storage"))]
pub async fn store_ltm(
    body: JsonBody<StoreLTMRequest>,
) -> JsonResult<StoreLTMResponse> {
    let req = body.into_inner();
    req.validate()?;

    info!(
        "Storing LTM: source_id={}, source_type={}, content_length={}",
        req.source_id,
        req.source_type,
        req.content.len()
    );

    let entry_id = MemoryStorageService::store_ltm(
        &req.source_id,
        &req.source_type,
        &req.content,
        req.title.as_deref(),
    )
    .await?;

    json_ok(StoreLTMResponse { entry_id })
}

/// 手动触发 STM 到 LTM 转移
#[endpoint(tags("memory-storage"))]
pub async fn transfer_stm_to_ltm(
    body: JsonBody<TransferRequest>,
) -> JsonResult<TransferResponse> {
    let req = body.into_inner();
    req.validate()?;

    info!("Manual transfer: session_id={}", req.session_id);

    let entry_ids = MemoryStorageService::auto_transfer_stm_to_ltm(
        &req.session_id,
        req.message_count_threshold.unwrap_or(100),
    )
    .await?;

    json_ok(TransferResponse { entry_ids })
}

/// 批量存储长期记忆
#[endpoint(tags("memory-storage"))]
pub async fn batch_store_ltm(
    body: JsonBody<BatchStoreLTMRequest>,
) -> JsonResult<BatchStoreLTMResponse> {
    let req = body.into_inner();
    req.validate()?;

    info!("Batch storing LTM: count={}", req.entries.len());

    let entries: Vec<(String, String, String, Option<String>)> = req
        .entries
        .into_iter()
        .map(|e| (e.source_id, e.source_type, e.content, e.title))
        .collect();

    let entry_ids = MemoryStorageService::batch_store_ltm(entries).await?;

    json_ok(BatchStoreLTMResponse { entry_ids })
}

/// 获取会话消息
#[endpoint(tags("memory-storage"))]
pub async fn get_session_messages(
    req: &mut Request,
) -> JsonResult<Vec<crate::db::SessionMessage>> {
    let session_id: String = req.param("session_id")
        .ok_or_else(|| crate::AppError::BadRequest("session_id parameter is required".to_string()))?;
    let limit: Option<i32> = req.query("limit");

    let messages = crate::db::stm::STMRepository::get_session_messages(
        &session_id,
        limit,
    )
    .await?;

    json_ok(messages)
}

