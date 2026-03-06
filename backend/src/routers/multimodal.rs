//! Multimodal Memory API Routes

use salvo::oapi::extract::*;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::db::mm::MMRepository;
use crate::db::pool;
use crate::{json_ok, JsonResult};

/// 存储多模态记忆请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct StoreMMRequest {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(rename = "sourceId")]
    pub source_id: String,
    #[serde(rename = "modalityType")]
    pub modality_type: String,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Base64 编码的二进制数据
    pub content: Option<String>,
    #[serde(rename = "textContent")]
    pub text_content: Option<String>,
    #[serde(rename = "imageUrl")]
    pub image_url: Option<String>,
    #[serde(rename = "audioUrl")]
    pub audio_url: Option<String>,
}

/// 存储多模态记忆响应
#[derive(Serialize, ToSchema)]
pub struct StoreMMResponse {
    #[serde(rename = "entryId")]
    pub entry_id: String,
}

/// 获取多模态记忆请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct GetMMRequest {
    #[serde(rename = "entryId")]
    pub entry_id: String,
}

/// 多模态记忆信息
#[derive(Serialize, ToSchema)]
pub struct MMEntryInfo {
    #[serde(rename = "entryId")]
    pub entry_id: String,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(rename = "sourceId")]
    pub source_id: String,
    #[serde(rename = "modalityType")]
    pub modality_type: String,
    pub title: Option<String>,
    pub description: Option<String>,
}

/// 搜索多模态记忆请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct SearchMMRequest {
    pub query: String,
    #[serde(rename = "modalityType")]
    pub modality_type: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i32,
}

fn default_limit() -> i32 {
    10
}

/// 存储多模态记忆
#[endpoint]
pub async fn store_mm(
    body: JsonBody<StoreMMRequest>,
) -> JsonResult<StoreMMResponse> {
    // 解析二进制内容
    let _binary_data = if let Some(content) = &body.content {
        use base64::Engine;
        Some(base64::engine::general_purpose::STANDARD.decode(content).unwrap_or_default())
    } else {
        None
    };

    let entry_id = MMRepository::create_entry(
        body.session_id.as_deref(),
        &body.source_id,
        &body.modality_type,
        "{}",  // content_metadata
        body.text_content.as_deref(),
        body.image_url.as_deref(),
        body.audio_url.as_deref(),
        None,  // video_url
    )
    .await
    .map_err(|e| crate::AppError::Internal(format!("Failed to store multimodal: {}", e)))?;

    json_ok(StoreMMResponse { entry_id })
}

/// 获取多模态记忆
#[endpoint]
pub async fn get_mm(
    entry_id: PathParam<String>,
) -> JsonResult<Option<MMEntryInfo>> {
    let entry = MMRepository::get_entry_by_id(&entry_id)
        .await
        .map_err(|e| crate::AppError::Internal(format!("Failed to get multimodal: {}", e)))?;

    let info = entry.map(|e| MMEntryInfo {
        entry_id: e.entry_id,
        session_id: e.session_id,
        source_id: e.source_id,
        modality_type: e.modality_type,
        title: e.title,
        description: e.description,
    });

    json_ok(info)
}

/// 获取会话的多模态记忆
#[endpoint]
pub async fn get_session_mm(
    session_id: PathParam<String>,
    limit: QueryParam<usize, false>,
) -> JsonResult<Vec<MMEntryInfo>> {
    let limit = limit.unwrap_or(20) as i32;

    let entries = MMRepository::get_entries_by_session(&session_id, Some(limit))
        .await
        .map_err(|e| crate::AppError::Internal(format!("Failed to get session multimodal: {}", e)))?;

    let infos: Vec<MMEntryInfo> = entries
        .into_iter()
        .map(|e| MMEntryInfo {
            entry_id: e.entry_id,
            session_id: e.session_id,
            source_id: e.source_id,
            modality_type: e.modality_type,
            title: e.title,
            description: e.description,
        })
        .collect();

    json_ok(infos)
}

/// 获取指定模态的多模态记忆
#[endpoint]
pub async fn get_by_modality(
    modality_type: PathParam<String>,
    limit: QueryParam<usize, false>,
) -> JsonResult<Vec<MMEntryInfo>> {
    let limit = limit.unwrap_or(20) as i32;

    let entries = MMRepository::get_entries_by_modality(&modality_type, Some(limit))
        .await
        .map_err(|e| crate::AppError::Internal(format!("Failed to get by modality: {}", e)))?;

    let infos: Vec<MMEntryInfo> = entries
        .into_iter()
        .map(|e| MMEntryInfo {
            entry_id: e.entry_id,
            session_id: e.session_id,
            source_id: e.source_id,
            modality_type: e.modality_type,
            title: e.title,
            description: e.description,
        })
        .collect();

    json_ok(infos)
}

/// 多模态记忆列表响应
#[derive(Serialize, ToSchema)]
pub struct MMEntryListResponse {
    pub entries: Vec<MMEntryInfo>,
    pub total: usize,
    pub limit: i32,
    pub offset: i32,
}

/// 获取多模态记忆列表
#[endpoint]
pub async fn list_mm(
    modality_type: QueryParam<String, false>,
    limit: QueryParam<usize, false>,
    offset: QueryParam<usize, false>,
) -> JsonResult<MMEntryListResponse> {
    let limit = limit.unwrap_or(20) as i32;
    let offset = offset.unwrap_or(0) as i32;

    let response = MMRepository::list_entries(
        modality_type.as_deref(),
        Some(limit),
        Some(offset),
    )
    .await
    .map_err(|e| crate::AppError::Internal(format!("Failed to list multimodal entries: {}", e)))?;

    let entries: Vec<MMEntryInfo> = response.entries
        .into_iter()
        .map(|e| MMEntryInfo {
            entry_id: e.entry_id,
            session_id: e.session_id,
            source_id: e.source_id,
            modality_type: e.modality_type,
            title: e.title,
            description: e.description,
        })
        .collect();

    json_ok(MMEntryListResponse {
        entries,
        total: response.total,
        limit,
        offset,
    })
}
