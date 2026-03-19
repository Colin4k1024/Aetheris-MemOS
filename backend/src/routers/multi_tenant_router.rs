/// 多租户 API 路由处理器 — Issue #56
use axum::extract::Path;
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;

use crate::services::multi_tenant::{
    AccessController, AccessDecision, AccessRequest, CrossAgentMemoryQuery, QuotaEnforcer,
    TenantConfig, TenantId, TenantRole,
};
use crate::{json_ok, JsonResult};

// ============ 请求/响应结构体 ============

#[derive(Deserialize, ToSchema)]
pub struct RegisterTenantRequest {
    pub tenant_id: String,
    pub name: String,
    pub max_stm_sessions: Option<usize>,
    pub max_ltm_entries: Option<usize>,
    pub enable_cross_agent_sharing: Option<bool>,
    pub shared_knowledge_tenants: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub struct RegisterTenantResponse {
    pub tenant_id: String,
    pub message: String,
}

#[derive(Serialize, ToSchema)]
pub struct ListTenantsResponse {
    pub tenants: Vec<String>,
    pub total: usize,
}

#[derive(Deserialize, ToSchema)]
pub struct TenantSearchRequest {
    pub query: String,
    pub top_k: Option<usize>,
}

#[derive(Deserialize, ToSchema)]
pub struct CheckAccessRequest {
    pub requester_tenant: String,
    pub requester_role: String,
    pub target_tenant: String,
    pub operation: String,
}

// ============ 处理器 ============

/// 注册新租户
pub async fn register_tenant(
    Json(req): Json<RegisterTenantRequest>,
) -> JsonResult<RegisterTenantResponse> {
    info!("Registering tenant: {}", req.tenant_id);

    let mut cfg = TenantConfig::new(req.tenant_id.clone(), req.name);
    cfg.max_stm_sessions = req.max_stm_sessions;
    cfg.max_ltm_entries = req.max_ltm_entries;
    cfg.enable_cross_agent_sharing = req.enable_cross_agent_sharing.unwrap_or(true);
    cfg.shared_knowledge_tenants = req
        .shared_knowledge_tenants
        .unwrap_or_default()
        .into_iter()
        .map(TenantId::new)
        .collect();

    crate::services::multi_tenant::register_tenant(cfg);

    json_ok(RegisterTenantResponse {
        tenant_id: req.tenant_id,
        message: "Tenant registered successfully".to_string(),
    })
}

/// 列举所有租户
pub async fn list_tenants() -> JsonResult<ListTenantsResponse> {
    let tenants = crate::services::multi_tenant::list_tenants();
    let total = tenants.len();
    json_ok(ListTenantsResponse { tenants, total })
}

/// 在指定租户范围内进行三路混合搜索
pub async fn tenant_search(
    Path(tenant_id): Path<String>,
    Json(req): Json<TenantSearchRequest>,
) -> JsonResult<Vec<crate::services::memory_search::SearchResult>> {
    info!("Tenant search: tenant={}, query_len={}", tenant_id, req.query.len());

    let tid = TenantId::new(tenant_id);
    let results = CrossAgentMemoryQuery::search_tenant_ltm(
        &tid,
        &req.query,
        req.top_k.unwrap_or(10),
    )
    .await?;

    json_ok(results)
}

/// 列举租户下所有 STM 会话
pub async fn tenant_sessions(
    Path(tenant_id): Path<String>,
) -> JsonResult<Vec<crate::db::stm::Session>> {
    info!("Listing sessions for tenant: {}", tenant_id);

    let tid = TenantId::new(tenant_id);
    let sessions = CrossAgentMemoryQuery::list_tenant_sessions(&tid, Some(100)).await?;

    json_ok(sessions)
}

/// 检查跨租户访问权限
pub async fn check_access(
    Json(req): Json<CheckAccessRequest>,
) -> JsonResult<AccessDecision> {
    let role = match req.requester_role.as_str() {
        "admin" => TenantRole::Admin,
        "super_admin" => TenantRole::SuperAdmin,
        _ => TenantRole::Member,
    };

    let access_req = AccessRequest {
        requester_tenant: TenantId::new(req.requester_tenant),
        requester_role: role,
        target_tenant: TenantId::new(req.target_tenant),
        operation: req.operation,
    };

    let decision = AccessController::check(&access_req);
    json_ok(decision)
}
