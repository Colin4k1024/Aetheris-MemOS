use axum::extract::{Path, Query};
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;
use validator::Validate;

use crate::db::SessionMessage;
use crate::services::memory_search::{MemorySearchService, SearchResult};
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

/// 三路混合搜索请求（向量 + 关键词 + KG图谱）
#[derive(Deserialize, ToSchema, Validate)]
pub struct TripleHybridSearchRequest {
    pub query: String,
    #[serde(rename = "topK")]
    pub top_k: Option<usize>,
    /// 向量搜索权重（默认 0.5）
    #[serde(rename = "vectorWeight")]
    pub vector_weight: Option<f32>,
    /// 关键词搜索权重（默认 0.3）
    #[serde(rename = "keywordWeight")]
    pub keyword_weight: Option<f32>,
    /// 知识图谱搜索权重（默认 0.2）
    #[serde(rename = "graphWeight")]
    pub graph_weight: Option<f32>,
    #[serde(rename = "enableRerank")]
    pub enable_rerank: Option<bool>,
    #[serde(rename = "minScore")]
    pub min_score: Option<f32>,
}

/// 三路混合搜索响应
#[derive(Serialize, ToSchema)]
pub struct TripleHybridSearchResponse {
    pub results: Vec<SearchResult>,
}

/// 搜索短期记忆
pub async fn search_stm(Json(req): Json<SearchSTMRequest>) -> JsonResult<SearchSTMResponse> {
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
pub async fn search_ltm(Json(req): Json<SearchLTMRequest>) -> JsonResult<SearchLTMResponse> {
    req.validate()?;

    info!(
        "Searching LTM: query_length={}, top_k={:?}",
        req.query.len(),
        req.top_k
    );

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
pub async fn hybrid_search(
    Json(req): Json<HybridSearchRequest>,
) -> JsonResult<HybridSearchResponse> {
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
pub async fn search_by_entity(
    Json(req): Json<SearchByEntityRequest>,
) -> JsonResult<SearchByEntityResponse> {
    req.validate()?;

    info!(
        "Searching by entity: entity={}, limit={:?}",
        req.entity, req.limit
    );

    let results = MemorySearchService::search_by_entity(&req.entity, req.limit).await?;

    json_ok(SearchByEntityResponse { results })
}

/// 三路混合搜索（向量 + 关键词 + 知识图谱）
pub async fn triple_hybrid_search(
    Json(req): Json<TripleHybridSearchRequest>,
) -> JsonResult<TripleHybridSearchResponse> {
    req.validate()?;

    info!(
        "Triple hybrid search: query_length={}, top_k={:?}",
        req.query.len(),
        req.top_k
    );

    let results = MemorySearchService::triple_hybrid_search(
        &req.query,
        req.top_k.unwrap_or(10),
        req.vector_weight,
        req.keyword_weight,
        req.graph_weight,
        req.enable_rerank,
        req.min_score,
    )
    .await?;

    json_ok(TripleHybridSearchResponse { results })
}

/// 获取所有知识条目列表
pub async fn list_ltm_entries() -> JsonResult<crate::db::ltm::KnowledgeEntryListResponse> {
    let result = crate::db::ltm::LTMRepository::list_entries(None, None, Some(20), Some(0)).await?;
    info!("LTM list success: {} entries", result.entries.len());
    json_ok(result)
}

/// 获取知识条目详情
pub async fn get_ltm_entry(
    Path(entry_id): Path<String>,
) -> JsonResult<crate::db::ltm::KnowledgeEntry> {
    info!("Getting LTM entry: entry_id={}", entry_id);

    let entry = crate::db::ltm::LTMRepository::get_entry_by_id(&entry_id)
        .await?
        .ok_or_else(|| crate::AppError::NotFound(format!("Entry {} not found", entry_id)))?;

    json_ok(entry)
}

// ============ Bi-temporal Tracking Endpoints ============

/// 时间旅行查询请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct TimeTravelQuery {
    /// RFC3339 格式的时间戳，例如 "2024-01-01T00:00:00Z"
    pub at: String,
    pub limit: Option<i32>,
}

/// 时间旅行查询 - LTM
pub async fn get_ltm_at_time(
    Path(entry_id): Path<String>,
    Query(query): Query<TimeTravelQuery>,
) -> JsonResult<Option<crate::db::ltm::KnowledgeEntry>> {
    info!(
        "Time travel query LTM: entry_id={}, at={}",
        entry_id, query.at
    );

    let entry = crate::db::ltm::LTMRepository::get_entry_at_time(&entry_id, &query.at).await?;

    json_ok(entry)
}

/// 时间旅行搜索 - LTM
pub async fn search_ltm_at_time(
    Json(req): Json<TimeTravelQuery>,
) -> JsonResult<Vec<crate::db::ltm::KnowledgeEntry>> {
    info!("Time travel search LTM: at={}", req.at);

    // 使用 "memory" 作为默认查询词
    let results = crate::db::ltm::LTMRepository::search_entries_at_time(
        "",
        &req.at,
        req.limit,
    )
    .await?;

    json_ok(results)
}

/// 获取条目版本历史
pub async fn get_ltm_history(
    Path(entry_id): Path<String>,
) -> JsonResult<Vec<crate::db::ltm::KnowledgeEntry>> {
    info!("Getting LTM history: entry_id={}", entry_id);

    let history = crate::db::ltm::LTMRepository::get_entry_history(&entry_id).await?;

    json_ok(history)
}

/// 时间旅行查询 - KG Entity
pub async fn get_kg_entity_at_time(
    Path(entity_id): Path<String>,
    Query(query): Query<TimeTravelQuery>,
) -> JsonResult<Option<crate::db::kg::Entity>> {
    info!(
        "Time travel query KG: entity_id={}, at={}",
        entity_id, query.at
    );

    let entity = crate::db::kg::KGRepository::get_entity_at_time(&entity_id, &query.at).await?;

    json_ok(entity)
}

/// 获取实体版本历史
pub async fn get_kg_entity_history(
    Path(entity_id): Path<String>,
) -> JsonResult<Vec<crate::db::kg::Entity>> {
    info!("Getting KG entity history: entity_id={}", entity_id);

    let history = crate::db::kg::KGRepository::get_entity_history(&entity_id).await?;

    json_ok(history)
}
