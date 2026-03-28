//! Knowledge Graph API Routes

use axum::extract::{Path, Query};
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::db::kg::KGRepository;
use crate::db::pool;
use crate::tenant::get_default_tenant;
use crate::{json_ok, JsonResult};

/// 创建实体请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct CreateEntityRequest {
    #[serde(rename = "entityName")]
    pub entity_name: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    pub description: Option<String>,
    pub aliases: Option<Vec<String>>,
}

/// 创建实体响应
#[derive(Serialize, ToSchema)]
pub struct CreateEntityResponse {
    #[serde(rename = "entityId")]
    pub entity_id: String,
}

/// 创建关系请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct CreateRelationRequest {
    #[serde(rename = "sourceEntityId")]
    pub source_entity_id: String,
    #[serde(rename = "targetEntityId")]
    pub target_entity_id: String,
    #[serde(rename = "relationType")]
    pub relation_type: String,
    pub weight: Option<f32>,
    pub confidence: Option<f32>,
}

/// 创建关系响应
#[derive(Serialize, ToSchema)]
pub struct CreateRelationResponse {
    #[serde(rename = "relationId")]
    pub relation_id: String,
}

/// 搜索实体请求
#[derive(Deserialize, ToSchema, Validate)]
pub struct SearchEntitiesRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: i32,
}

fn default_limit() -> i32 {
    10
}

/// 实体信息
#[derive(Serialize, ToSchema)]
pub struct EntityInfo {
    #[serde(rename = "entityId")]
    pub entity_id: String,
    #[serde(rename = "entityName")]
    pub entity_name: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    pub description: Option<String>,
}

/// 关系信息
#[derive(Serialize, ToSchema)]
pub struct RelationInfo {
    #[serde(rename = "relationId")]
    pub relation_id: String,
    #[serde(rename = "sourceEntityId")]
    pub source_entity_id: String,
    #[serde(rename = "targetEntityId")]
    pub target_entity_id: String,
    #[serde(rename = "relationType")]
    pub relation_type: String,
    pub weight: f32,
    pub confidence: f32,
}

/// 创建实体
pub async fn create_entity(
    Json(body): Json<CreateEntityRequest>,
) -> JsonResult<CreateEntityResponse> {
    let aliases_refs: Option<Vec<&str>> = body
        .aliases
        .as_deref()
        .map(|v| v.iter().map(|s| s.as_str()).collect());
    let entity_id = KGRepository::create_entity(
        &get_default_tenant(),
        &body.entity_name,
        &body.entity_type,
        body.description.as_deref(),
        None,
        aliases_refs.as_deref(),
        None,
        None,
        1.0,
    )
    .await
    .map_err(|e| crate::AppError::Internal(format!("Failed to create entity: {}", e)))?;

    json_ok(CreateEntityResponse { entity_id })
}

/// 创建关系
pub async fn create_relation(
    Json(body): Json<CreateRelationRequest>,
) -> JsonResult<CreateRelationResponse> {
    let relation_id = KGRepository::create_relation(
        &body.source_entity_id,
        &body.target_entity_id,
        &body.relation_type,
        body.weight.unwrap_or(1.0) as f64,
        body.confidence.unwrap_or(1.0) as f64,
        None,
    )
    .await
    .map_err(|e| crate::AppError::Internal(format!("Failed to create relation: {}", e)))?;

    json_ok(CreateRelationResponse { relation_id })
}

/// 根据名称获取实体
pub async fn get_entity_by_name(Path(name): Path<String>) -> JsonResult<Option<EntityInfo>> {
    let entity = KGRepository::get_entity_by_name(pool(), &get_default_tenant(), &name, None)
        .await
        .map_err(|e| crate::AppError::Internal(format!("Failed to get entity: {}", e)))?;

    let info = entity.map(|e| EntityInfo {
        entity_id: e.entity_id,
        entity_name: e.entity_name,
        entity_type: e.entity_type,
        description: e.description,
    });

    json_ok(info)
}

/// 获取实体的相关实体
pub async fn get_related_entities(
    Path(entity_id): Path<String>,
    Query(query): Query<RelatedEntitiesQuery>,
) -> JsonResult<Vec<RelationInfo>> {
    let limit = query.limit.unwrap_or(10) as i32;

    let relations = KGRepository::get_related_entities(pool(), &get_default_tenant(), &entity_id, None, Some(limit))
        .await
        .map_err(|e| crate::AppError::Internal(format!("Failed to get related entities: {}", e)))?;

    let infos: Vec<RelationInfo> = relations
        .into_iter()
        .map(|(_entity, relation)| RelationInfo {
            relation_id: relation.relation_id,
            source_entity_id: relation.source_entity_id,
            target_entity_id: relation.target_entity_id,
            relation_type: relation.relation_type,
            weight: relation.weight as f32,
            confidence: relation.confidence as f32,
        })
        .collect();

    json_ok(infos)
}

/// 根据实体搜索知识
pub async fn search_by_entity(
    Json(body): Json<SearchEntitiesRequest>,
) -> JsonResult<Vec<EntityInfo>> {
    let pool = pool();
    let entities = KGRepository::search_knowledge_by_entity(pool, &body.query, Some(body.limit))
        .await
        .map_err(|e| crate::AppError::Internal(format!("Failed to search: {}", e)))?;

    let infos: Vec<EntityInfo> = entities
        .into_iter()
        .map(|e| EntityInfo {
            entity_id: e.entity_id,
            entity_name: e.entity_name,
            entity_type: e.entity_type,
            description: e.description,
        })
        .collect();

    json_ok(infos)
}

/// 实体列表响应
#[derive(Serialize, ToSchema)]
pub struct EntityListResponse {
    pub entities: Vec<EntityInfo>,
    pub total: usize,
    pub limit: i32,
    pub offset: i32,
}

/// 获取实体列表
pub async fn list_entities(
    Query(query): Query<ListEntitiesQuery>,
) -> JsonResult<EntityListResponse> {
    let limit = query.limit.unwrap_or(20) as i32;
    let offset = query.offset.unwrap_or(0) as i32;

    let response =
        KGRepository::list_entities(pool(), &get_default_tenant(), query.entity_type.as_deref(), Some(limit), Some(offset))
            .await
            .map_err(|e| crate::AppError::Internal(format!("Failed to list entities: {}", e)))?;

    let entities: Vec<EntityInfo> = response
        .entities
        .into_iter()
        .map(|e| EntityInfo {
            entity_id: e.entity_id,
            entity_name: e.entity_name,
            entity_type: e.entity_type,
            description: e.description,
        })
        .collect();

    json_ok(EntityListResponse {
        entities,
        total: response.total,
        limit,
        offset,
    })
}

#[derive(Debug, Deserialize, Default)]
pub struct RelatedEntitiesQuery {
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListEntitiesQuery {
    pub entity_type: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
