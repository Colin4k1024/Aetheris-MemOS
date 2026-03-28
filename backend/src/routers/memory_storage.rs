use axum::extract::{Path, Query};
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;
use validator::Validate;

use crate::services::memory_storage::MemoryStorageService;
use crate::{json_ok, JsonResult};

/// 存储短期记忆请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct StoreSTMRequest {
    #[serde(rename = "tenantId")]
    pub tenant_id: Option<String>,
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
    #[serde(rename = "tenantId")]
    pub tenant_id: Option<String>,
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
    #[serde(rename = "tenantId")]
    pub tenant_id: Option<String>,
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
pub async fn store_stm(Json(req): Json<StoreSTMRequest>) -> JsonResult<StoreSTMResponse> {
    req.validate()?;

    info!(
        "Storing STM: user_id={}, agent_id={}, session_type={}",
        req.user_id, req.agent_id, req.session_type
    );

    let (session_id, message_id) = if req.tenant_id.as_deref().is_some() {
        MemoryStorageService::store_stm_with_tenant(
            req.tenant_id.as_deref(),
            &req.user_id,
            &req.agent_id,
            &req.session_type,
            &req.role,
            &req.content,
            req.max_context_length.unwrap_or(4096),
            req.retention_hours.unwrap_or(24),
        )
        .await?
    } else {
        MemoryStorageService::store_stm(
            &req.user_id,
            &req.agent_id,
            &req.session_type,
            &req.role,
            &req.content,
            req.max_context_length.unwrap_or(4096),
            req.retention_hours.unwrap_or(24),
        )
        .await?
    };

    json_ok(StoreSTMResponse {
        session_id,
        message_id,
    })
}

/// 存储长期记忆
pub async fn store_ltm(Json(req): Json<StoreLTMRequest>) -> JsonResult<StoreLTMResponse> {
    req.validate()?;

    info!(
        "Storing LTM: source_id={}, source_type={}, content_length={}",
        req.source_id,
        req.source_type,
        req.content.len()
    );

    let entry_id = MemoryStorageService::store_ltm_with_tenant(
        req.tenant_id.as_deref(),
        &req.source_id,
        &req.source_type,
        &req.content,
        req.title.as_deref(),
    )
    .await?;

    json_ok(StoreLTMResponse { entry_id })
}

/// 手动触发 STM 到 LTM 转移
pub async fn transfer_stm_to_ltm(Json(req): Json<TransferRequest>) -> JsonResult<TransferResponse> {
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
pub async fn batch_store_ltm(
    Json(req): Json<BatchStoreLTMRequest>,
) -> JsonResult<BatchStoreLTMResponse> {
    req.validate()?;

    info!("Batch storing LTM: count={}", req.entries.len());

    let entries: Vec<crate::services::memory_storage::LtmWriteRequest> = req
        .entries
        .into_iter()
        .map(|e| crate::services::memory_storage::LtmWriteRequest {
            tenant_id: e.tenant_id,
            source_id: e.source_id,
            source_type: e.source_type,
            content: e.content,
            title: e.title,
        })
        .collect();

    let entry_ids = MemoryStorageService::batch_store_ltm_with_tenant(entries).await?;

    json_ok(BatchStoreLTMResponse { entry_ids })
}

/// 获取所有会话列表
pub async fn list_sessions(
    Query(params): Query<ListSessionsQuery>,
) -> JsonResult<crate::db::SessionListResponse> {
    let sessions = crate::db::stm::STMRepository::list_sessions(
        params.user_id.as_deref(),
        params.status.as_deref(),
        params.limit,
        params.offset,
    )
    .await?;

    json_ok(sessions)
}

/// 获取会话消息
pub async fn get_session_messages(
    Path(session_id): Path<String>,
    Query(params): Query<GetSessionMessagesQuery>,
) -> JsonResult<Vec<crate::db::SessionMessage>> {
    let messages =
        crate::db::stm::STMRepository::get_session_messages(&session_id, params.limit).await?;

    json_ok(messages)
}

#[derive(Debug, Deserialize, Default)]
pub struct ListSessionsQuery {
    pub user_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[derive(Debug, Deserialize, Default)]
pub struct GetSessionMessagesQuery {
    pub limit: Option<i32>,
}

// ============ 上下文压缩端点（Issue #54）============

/// 压缩会话请求
#[derive(Deserialize, ToSchema)]
pub struct CompressSessionRequest {
    pub session_id: String,
    /// 压缩策略（sliding_window / llm_summary / importance_prune / hierarchical）
    pub strategy: Option<crate::services::context_compressor::CompressionStrategy>,
    /// Token 预算（默认 4096）
    pub token_budget: Option<usize>,
    /// 窗口大小（默认 20）
    pub window_size: Option<usize>,
    /// 分层压缩保留最近条数（默认 10）
    pub hierarchical_recent_k: Option<usize>,
}

/// 压缩任意消息列表请求
#[derive(Deserialize, ToSchema)]
pub struct CompressMessagesRequest {
    pub messages: Vec<crate::services::context_compressor::MessageEntry>,
    pub strategy: Option<crate::services::context_compressor::CompressionStrategy>,
    pub token_budget: Option<usize>,
    pub window_size: Option<usize>,
    pub hierarchical_recent_k: Option<usize>,
}

fn build_compression_cfg(
    strategy: Option<crate::services::context_compressor::CompressionStrategy>,
    token_budget: Option<usize>,
    window_size: Option<usize>,
    hierarchical_recent_k: Option<usize>,
) -> crate::services::context_compressor::CompressionConfig {
    let mut cfg = crate::services::context_compressor::CompressionConfig::default();
    if let Some(s) = strategy {
        cfg.strategy = s;
    }
    if let Some(t) = token_budget {
        cfg.token_budget = t;
    }
    if let Some(w) = window_size {
        cfg.window_size = w;
    }
    if let Some(k) = hierarchical_recent_k {
        cfg.hierarchical_recent_k = k;
    }
    cfg
}

/// 压缩指定会话的消息
pub async fn compress_session(
    Json(req): Json<CompressSessionRequest>,
) -> crate::JsonResult<crate::services::context_compressor::CompressionResult> {
    info!("Compressing session: session_id={}", req.session_id);
    let cfg = build_compression_cfg(
        req.strategy,
        req.token_budget,
        req.window_size,
        req.hierarchical_recent_k,
    );
    let result = crate::services::context_compressor::ContextCompressor::compress_session(
        &req.session_id,
        &cfg,
    )
    .await?;
    crate::json_ok(result)
}

/// 压缩任意消息列表（无需存入 DB）
pub async fn compress_messages(
    Json(req): Json<CompressMessagesRequest>,
) -> crate::JsonResult<crate::services::context_compressor::CompressionResult> {
    info!("Compressing {} messages", req.messages.len());
    let cfg = build_compression_cfg(
        req.strategy,
        req.token_budget,
        req.window_size,
        req.hierarchical_recent_k,
    );
    let result =
        crate::services::context_compressor::ContextCompressor::compress(req.messages, &cfg)
            .await?;
    crate::json_ok(result)
}
