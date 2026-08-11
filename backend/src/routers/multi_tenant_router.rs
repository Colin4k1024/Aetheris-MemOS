/// 多租户 API 路由处理器 — Issue #56
use axum::extract::{Extension, Path};
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;

use crate::services::multi_tenant::{
    AccessController, AccessDecision, AccessRequest, CrossAgentMemoryQuery, QuotaEnforcer,
    TenantConfig, TenantId, TenantRole,
};
use crate::tenant::RequestTenantContext;
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
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(req): Json<RegisterTenantRequest>,
) -> JsonResult<RegisterTenantResponse> {
    tenant_ctx.authorize_path_tenant(&req.tenant_id)?;
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

/// 列举调用方自己的租户
///
/// 收窄自「枚举全部租户」。当前每个已认证用户都是自身租户的 Owner
/// (`tenant_id == user_id`)，返回全量租户列表等于把所有租户 id 泄漏给任何登录用户
/// （加 `ManageTenant` 门禁挡不住——那道门禁对每个 Owner 都放行）。在 org 层租户模型
/// (C-3) 与显式 admin 授权入口落地前，只返回调用方自己的租户。保留 list 返回形状
/// （0 或 1 个元素）以免破坏调用方的响应结构。
pub async fn list_tenants(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
) -> JsonResult<ListTenantsResponse> {
    let tenants: Vec<String> = crate::services::multi_tenant::list_tenants()
        .into_iter()
        .filter(|t| t == tenant_ctx.tenant_id.as_str())
        .collect();
    let total = tenants.len();
    json_ok(ListTenantsResponse { tenants, total })
}

/// 在指定租户范围内进行三路混合搜索
pub async fn tenant_search(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Path(tenant_id): Path<String>,
    Json(req): Json<TenantSearchRequest>,
) -> JsonResult<Vec<crate::services::memory_search::SearchResult>> {
    tenant_ctx.authorize_path_tenant(&tenant_id)?;
    info!(
        "Tenant search: tenant={}, query_len={}",
        tenant_id,
        req.query.len()
    );

    let tid = TenantId::new(tenant_id);
    let results =
        CrossAgentMemoryQuery::search_tenant_ltm(&tid, &req.query, req.top_k.unwrap_or(10)).await?;

    json_ok(results)
}

/// 列举租户下所有 STM 会话
pub async fn tenant_sessions(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Path(tenant_id): Path<String>,
) -> JsonResult<Vec<crate::db::stm::Session>> {
    tenant_ctx.authorize_path_tenant(&tenant_id)?;
    info!("Listing sessions for tenant: {}", tenant_id);

    let tid = TenantId::new(tenant_id);
    let sessions = CrossAgentMemoryQuery::list_tenant_sessions(&tid, Some(100)).await?;

    json_ok(sessions)
}

/// 检查跨租户访问权限
pub async fn check_access(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(req): Json<CheckAccessRequest>,
) -> JsonResult<AccessDecision> {
    // The caller may only ask access questions *as themselves*. Without this, any
    // authenticated user could set `requester_tenant` to someone else and probe the
    // cross-tenant access policy for tenants that are not theirs.
    tenant_ctx.authorize_path_tenant(&req.requester_tenant)?;

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
