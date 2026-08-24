//! Skill API routes — tenant-scoped CRUD for the `skills` asset table
//! (#90 first increment). Mirrors the agent_equipment pattern: the caller's
//! `tenant_id` (from `RequestTenantContext`) scopes every read/write; the DB
//! RLS policy (`20260824000002_rls_skills.sql`) fail-closes any path that
//! forgets it. Skill lifecycle (draft→active→deprecated) goes through the
//! update endpoint's `status` field; version-on-publish, extractor-from-trace,
//! Wiki and CodeGraph are follow-ups.

use axum::{
    extract::{Extension, Path, Query},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::db;
use crate::db::skill::SkillRepository;
use crate::error::AppError;
use crate::models::skill::{CreateSkillRequest, PublishSkillRequest, Skill, UpdateSkillRequest};
use crate::tenant::RequestTenantContext;

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_skills).post(create_skill))
        .route("/{id}", get(get_skill).put(update_skill).delete(delete_skill))
        .route("/{id}/publish", post(publish_skill))
        .route("/extract", post(extract_skills))
}

#[derive(Deserialize)]
pub struct ListSkillsQuery {
    pub owner_agent_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list_skills(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Query(q): Query<ListSkillsQuery>,
) -> Result<Json<Vec<Skill>>, AppError> {
    let pool = db::pool();
    let repo = SkillRepository::new(pool.clone());
    let skills = repo
        .list(
            &tenant_ctx.tenant_id,
            q.owner_agent_id.as_deref(),
            q.status.as_deref(),
            q.limit.unwrap_or(50),
            q.offset.unwrap_or(0),
        )
        .await?;
    Ok(Json(skills))
}

pub async fn create_skill(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(payload): Json<CreateSkillRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = db::pool();
    let repo = SkillRepository::new(pool.clone());
    let id = repo.create(&tenant_ctx.tenant_id, payload).await?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn get_skill(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Path(id): Path<String>,
) -> Result<Json<Skill>, AppError> {
    let pool = db::pool();
    let repo = SkillRepository::new(pool.clone());
    let skill = repo
        .get(&tenant_ctx.tenant_id, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("skill not found".to_string()))?;
    Ok(Json(skill))
}

pub async fn update_skill(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateSkillRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = db::pool();
    let repo = SkillRepository::new(pool.clone());
    let updated = repo
        .update(&tenant_ctx.tenant_id, &id, payload)
        .await?;
    Ok(Json(serde_json::json!({ "updated": updated })))
}

pub async fn delete_skill(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = db::pool();
    let repo = SkillRepository::new(pool.clone());
    let deleted = repo.delete(&tenant_ctx.tenant_id, &id).await?;
    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

/// Publish a new version of a skill: new row version+1 `active`, old row
/// `deprecated` (history preserved, immutable). POST /v1/skills/{id}/publish.
pub async fn publish_skill(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Path(id): Path<String>,
    Json(payload): Json<PublishSkillRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = db::pool();
    let repo = SkillRepository::new(pool.clone());
    let new_id = repo
        .publish_version(&tenant_ctx.tenant_id, &id, payload)
        .await?;
    Ok(Json(serde_json::json!({ "id": new_id })))
}

#[derive(Deserialize)]
pub struct ExtractSkillsRequest {
    pub transcript: String,
    pub reason: Option<String>,
}

/// Extract Skill candidates from an execution transcript via the LLM (#90).
/// Candidates are **suggestions only** — NOT auto-published; the caller reviews
/// them and publishes the chosen ones via `POST /v1/skills`.
pub async fn extract_skills(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(req): Json<ExtractSkillsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = tenant_ctx; // extraction is LLM-only (no DB); auth gate via route_layer
    let extractor = crate::services::skill::extractor::SkillExtractor::new();
    let candidates = extractor
        .extract_from_transcript(&req.transcript, req.reason.as_deref())
        .await
        .map_err(|e| AppError::Internal(format!("skill extraction failed: {e}")))?;
    Ok(Json(serde_json::to_value(&candidates).unwrap_or(serde_json::json!([]))))
}
