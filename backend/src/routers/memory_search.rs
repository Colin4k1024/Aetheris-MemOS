use salvo::oapi::extract::*;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::{info, error};
use validator::Validate;

use crate::services::memory_search::{MemorySearchService, SearchResult};
use crate::db::SessionMessage;
use crate::{json_ok, JsonResult};

/// 搜索短期记忆请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct SearchSTMRequest {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "sessionType")]
    pub session_type: Option<String>,
    pub limit: Option<i32>,
}

/// 搜索短期记忆响应
#[derive(Serialize, ToSchema)]
pub struct SearchSTMResponse {
    pub messages: Vec<SessionMessage>,
}

/// 搜索长期记忆请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct SearchLTMRequest {
    pub query: String,
    #[serde(rename = "topK")]
    pub top_k: Option<usize>,
    #[serde(rename = "enableRerank")]
    pub enable_rerank: Option<bool>,
    #[serde(rename = "minScore")]
    pub min_score: Option<f32>,
}

/// 搜索长期记忆响应
#[derive(Serialize, ToSchema)]
pub struct SearchLTMResponse {
    pub results: Vec<SearchResult>,
}

/// 混合搜索请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct HybridSearchRequest {
    pub query: String,
    #[serde(rename = "topK")]
    pub top_k: Option<usize>,
    #[serde(rename = "keywordWeight")]
    pub keyword_weight: Option<f32>,
    #[serde(rename = "vectorWeight")]
    pub vector_weight: Option<f32>,
    #[serde(rename = "enableRerank")]
    pub enable_rerank: Option<bool>,
    #[serde(rename = "minScore")]
    pub min_score: Option<f32>,
}

/// 混合搜索响应
#[derive(Serialize, ToSchema)]
pub struct HybridSearchResponse {
    pub results: Vec<SearchResult>,
}

/// 基于实体搜索请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct SearchByEntityRequest {
    pub entity: String,
    pub limit: Option<i32>,
}

/// 基于实体搜索响应
#[derive(Serialize, ToSchema)]
pub struct SearchByEntityResponse {
    pub results: Vec<SearchResult>,
}

/// 搜索短期记忆
#[endpoint(tags("memory-search"))]
pub async fn search_stm(
    body: JsonBody<SearchSTMRequest>,
) -> JsonResult<SearchSTMResponse> {
    let req = body.into_inner();
    req.validate()?;

    info!(
        "Searching STM: user_id={}, agent_id={}, session_type={:?}",
        req.user_id, req.agent_id, req.session_type
    );

    let messages = MemorySearchService::search_stm(
        &req.user_id,
        &req.agent_id,
        req.session_type.as_deref(),
        req.limit,
    )
    .await?;

    json_ok(SearchSTMResponse { messages })
}

/// 搜索长期记忆（向量搜索）
#[endpoint(tags("memory-search"))]
pub async fn search_ltm(
    body: JsonBody<SearchLTMRequest>,
) -> JsonResult<SearchLTMResponse> {
    let req = body.into_inner();
    req.validate()?;

    info!("Searching LTM: query_length={}, top_k={:?}", req.query.len(), req.top_k);

    let results = MemorySearchService::search_ltm(
        &req.query,
        req.top_k.unwrap_or(10),
        req.enable_rerank,
        req.min_score,
    )
    .await?;

    json_ok(SearchLTMResponse { results })
}

/// 混合搜索
#[endpoint(tags("memory-search"))]
pub async fn hybrid_search(
    body: JsonBody<HybridSearchRequest>,
) -> JsonResult<HybridSearchResponse> {
    let req = body.into_inner();
    req.validate()?;

    info!(
        "Hybrid search: query_length={}, top_k={:?}",
        req.query.len(),
        req.top_k
    );

    let results = MemorySearchService::hybrid_search(
        &req.query,
        req.top_k.unwrap_or(10),
        req.keyword_weight.unwrap_or(0.3),
        req.vector_weight.unwrap_or(0.7),
        req.enable_rerank,
        req.min_score,
    )
    .await?;

    json_ok(HybridSearchResponse { results })
}

/// 基于实体搜索
#[endpoint(tags("memory-search"))]
pub async fn search_by_entity(
    body: JsonBody<SearchByEntityRequest>,
) -> JsonResult<SearchByEntityResponse> {
    let req = body.into_inner();
    req.validate()?;

    info!("Searching by entity: entity={}, limit={:?}", req.entity, req.limit);

    let results = MemorySearchService::search_by_entity(
        &req.entity,
        req.limit,
    )
    .await?;

    json_ok(SearchByEntityResponse { results })
}

/// 获取所有知识条目列表
#[endpoint(tags("memory-search"))]
pub async fn list_ltm_entries(
    _req: &mut Request,
) -> JsonResult<crate::db::ltm::KnowledgeEntryListResponse> {
    // 调用数据库
    match crate::db::ltm::LTMRepository::list_entries(None, None, Some(20), Some(0)).await {
        Ok(result) => {
            info!("LTM list success: {} entries", result.entries.len());
            json_ok(result)
        }
        Err(e) => {
            error!("LTM list error: {}", e);
            json_ok(crate::db::ltm::KnowledgeEntryListResponse {
                entries: vec![],
                total: 0,
                limit: 20,
                offset: 0,
            })
        }
    }
}

/// 获取知识条目详情
#[endpoint(tags("memory-search"))]
pub async fn get_ltm_entry(
    req: &mut Request,
) -> JsonResult<crate::db::ltm::KnowledgeEntry> {
    let entry_id: String = req.param("entry_id")
        .ok_or_else(|| crate::AppError::BadRequest("entry_id parameter is required".to_string()))?;
    info!("Getting LTM entry: entry_id={}", entry_id);

    let entry = crate::db::ltm::LTMRepository::get_entry_by_id(&entry_id)
        .await?
        .ok_or_else(|| crate::AppError::NotFound(format!("Entry {} not found", entry_id)))?;

    json_ok(entry)
}

