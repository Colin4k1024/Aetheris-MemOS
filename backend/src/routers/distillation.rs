use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::db::distillation::DistillationRepository;
use crate::models::distillation::DistillationJobType;
use crate::services::distillation::DistillationService;
use crate::tenant::RequestTenantContext;

#[derive(Debug, Deserialize)]
pub struct TriggerRequest {
    pub user_id: String,
    pub agent_id: String,
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ListAtomsQuery {
    pub user_id: String,
    pub agent_id: String,
    pub atom_type: Option<String>,
    pub scene_name: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct ListScenesQuery {
    pub user_id: String,
    pub agent_id: String,
}

#[derive(Debug, Deserialize)]
pub struct GetPersonaQuery {
    pub user_id: String,
    pub agent_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ListJobsQuery {
    pub status: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

#[derive(Debug, Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Json<Self> {
        Json(Self {
            success: true,
            data: Some(data),
            error: None,
        })
    }
}

fn error_response(msg: &str) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "success": false,
            "error": msg
        })),
    )
}

pub async fn trigger_distillation(
    tenant_ctx: RequestTenantContext,
    Json(req): Json<TriggerRequest>,
) -> impl IntoResponse {
    info!(
        "Manual distillation trigger: tenant={}, user={}, session={}",
        tenant_ctx.tenant_id, req.user_id, req.session_id
    );

    match DistillationService::enqueue_job(
        &tenant_ctx.tenant_id,
        &req.user_id,
        &req.agent_id,
        &req.session_id,
        DistillationJobType::L0ToL1,
    )
    .await
    {
        Ok(job_id) => ApiResponse::ok(serde_json::json!({ "job_id": job_id })).into_response(),
        Err(e) => {
            error!("Failed to trigger distillation: {}", e);
            error_response(&format!("{}", e)).into_response()
        }
    }
}

pub async fn list_atoms(
    tenant_ctx: RequestTenantContext,
    Query(params): Query<ListAtomsQuery>,
) -> impl IntoResponse {
    match DistillationRepository::list_atoms(
        &tenant_ctx.tenant_id,
        &params.user_id,
        &params.agent_id,
        params.atom_type.as_deref(),
        params.scene_name.as_deref(),
        params.limit,
        params.offset,
    )
    .await
    {
        Ok(atoms) => ApiResponse::ok(atoms).into_response(),
        Err(e) => {
            error!("Failed to list atoms: {}", e);
            error_response(&format!("{}", e)).into_response()
        }
    }
}

pub async fn get_atom(
    tenant_ctx: RequestTenantContext,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match DistillationRepository::get_atom(&id, &tenant_ctx.tenant_id).await {
        Ok(Some(atom)) => ApiResponse::ok(atom).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": "Atom not found"})),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to get atom: {}", e);
            error_response(&format!("{}", e)).into_response()
        }
    }
}

pub async fn list_scenes(
    tenant_ctx: RequestTenantContext,
    Query(params): Query<ListScenesQuery>,
) -> impl IntoResponse {
    match DistillationRepository::list_scenes(&tenant_ctx.tenant_id, &params.user_id, &params.agent_id).await {
        Ok(scenes) => ApiResponse::ok(scenes).into_response(),
        Err(e) => {
            error!("Failed to list scenes: {}", e);
            error_response(&format!("{}", e)).into_response()
        }
    }
}

pub async fn get_scene(
    tenant_ctx: RequestTenantContext,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match DistillationRepository::get_scene(&id, &tenant_ctx.tenant_id).await {
        Ok(Some(scene)) => ApiResponse::ok(scene).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": "Scene not found"})),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to get scene: {}", e);
            error_response(&format!("{}", e)).into_response()
        }
    }
}

pub async fn get_persona(
    tenant_ctx: RequestTenantContext,
    Query(params): Query<GetPersonaQuery>,
) -> impl IntoResponse {
    match DistillationRepository::get_persona(&tenant_ctx.tenant_id, &params.user_id, &params.agent_id).await {
        Ok(Some(persona)) => ApiResponse::ok(persona).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": "Persona not found"})),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to get persona: {}", e);
            error_response(&format!("{}", e)).into_response()
        }
    }
}

pub async fn list_jobs(
    tenant_ctx: RequestTenantContext,
    Query(params): Query<ListJobsQuery>,
) -> impl IntoResponse {
    match DistillationRepository::list_jobs(
        &tenant_ctx.tenant_id,
        params.status.as_deref(),
        params.limit,
        params.offset,
    )
    .await
    {
        Ok(jobs) => ApiResponse::ok(jobs).into_response(),
        Err(e) => {
            error!("Failed to list jobs: {}", e);
            error_response(&format!("{}", e)).into_response()
        }
    }
}
