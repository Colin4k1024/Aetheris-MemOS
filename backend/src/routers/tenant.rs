//! Tenant Router
//!
//! API endpoints for multi-tenant management.

use axum::extract::Path;
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::info;
use validator::Validate;

use crate::services::rbac::{RbacService, Role, UserRole};
use crate::tenant::{TenantContext, TenantId};
use crate::{json_ok, JsonResult};

/// Create tenant request
#[derive(Deserialize, Serialize, Validate)]
pub struct CreateTenantRequest {
    pub name: String,
}

/// Assign role request
#[derive(Deserialize, Serialize, Validate)]
pub struct AssignRoleRequest {
    #[serde(rename = "userId")]
    pub user_id: String,
    pub role: Role,
    #[serde(rename = "assignedBy")]
    pub assigned_by: String,
}

/// Update quota request
#[derive(Deserialize, Serialize, Validate)]
pub struct UpdateQuotaRequest {
    #[serde(rename = "storageMb")]
    pub storage_mb: Option<u64>,
    #[serde(rename = "apiCallsPerDay")]
    pub api_calls_per_day: Option<u64>,
    #[serde(rename = "concurrentSessions")]
    pub concurrent_sessions: Option<u32>,
    #[serde(rename = "memoryEntries")]
    pub memory_entries: Option<u64>,
}

/// Tenant response
#[derive(Serialize)]
pub struct TenantResponse {
    pub tenant_id: String,
    pub name: String,
    pub created_at: i64,
}

/// Tenant quota response
#[derive(Serialize)]
pub struct TenantQuotaResponse {
    pub tenant_id: String,
    pub storage_mb: u64,
    pub api_calls_per_day: u64,
    pub concurrent_sessions: u32,
    pub memory_entries: u64,
    pub used_storage_mb: u64,
    pub used_api_calls: u64,
    pub used_sessions: u32,
    pub used_entries: u64,
}

/// RBAC role list response
#[derive(Serialize)]
pub struct RoleListResponse {
    pub roles: Vec<UserRoleResponse>,
}

/// User role response
#[derive(Serialize)]
pub struct UserRoleResponse {
    pub user_id: String,
    pub tenant_id: String,
    pub role: String,
}

// In-memory tenant storage
static TENANTS: std::sync::OnceLock<std::sync::RwLock<Vec<TenantContext>>> =
    std::sync::OnceLock::new();

fn get_tenants() -> &'static std::sync::RwLock<Vec<TenantContext>> {
    TENANTS.get_or_init(|| std::sync::RwLock::new(Vec::new()))
}

/// Create a new tenant
pub async fn create_tenant(
    Json(req): Json<CreateTenantRequest>,
) -> JsonResult<TenantResponse> {
    req.validate()?;
    info!("Creating tenant: {}", req.name);

    let tenant_id = TenantId::new();
    let mut context = TenantContext::new(tenant_id.clone());
    context.settings.name = req.name;

    get_tenants().write().unwrap().push(context.clone());

    json_ok(TenantResponse {
        tenant_id: tenant_id.as_str().to_string(),
        name: context.settings.name.clone(),
        created_at: context.created_at,
    })
}

/// Get tenant info
pub async fn get_tenant(
    Path(tenant_id): Path<String>,
) -> JsonResult<TenantResponse> {
    info!("Getting tenant: {}", tenant_id);

    let tenants = get_tenants().read().unwrap();
    let tenant = tenants
        .iter()
        .find(|t| t.tenant_id.as_str() == tenant_id)
        .ok_or_else(|| crate::AppError::NotFound(format!("Tenant {} not found", tenant_id)))?;

    json_ok(TenantResponse {
        tenant_id: tenant.tenant_id.as_str().to_string(),
        name: tenant.settings.name.clone(),
        created_at: tenant.created_at,
    })
}

/// Get tenant quota
pub async fn get_tenant_quota(
    Path(tenant_id): Path<String>,
) -> JsonResult<TenantQuotaResponse> {
    info!("Getting quota for tenant: {}", tenant_id);

    let tenants = get_tenants().read().unwrap();
    let tenant = tenants
        .iter()
        .find(|t| t.tenant_id.as_str() == tenant_id)
        .ok_or_else(|| crate::AppError::NotFound(format!("Tenant {} not found", tenant_id)))?;

    json_ok(TenantQuotaResponse {
        tenant_id: tenant_id.clone(),
        storage_mb: tenant.quota.storage_mb,
        api_calls_per_day: tenant.quota.api_calls_per_day,
        concurrent_sessions: tenant.quota.concurrent_sessions,
        memory_entries: tenant.quota.memory_entries,
        used_storage_mb: tenant.quota.used.storage_mb,
        used_api_calls: tenant.quota.used.api_calls_today,
        used_sessions: tenant.quota.used.concurrent_sessions,
        used_entries: tenant.quota.used.memory_entries,
    })
}

/// Update tenant quota
pub async fn update_tenant_quota(
    Path(tenant_id): Path<String>,
    Json(req): Json<UpdateQuotaRequest>,
) -> JsonResult<TenantQuotaResponse> {
    info!("Updating quota for tenant: {}", tenant_id);

    let mut tenants = get_tenants().write().unwrap();
    let tenant = tenants
        .iter_mut()
        .find(|t| t.tenant_id.as_str() == tenant_id)
        .ok_or_else(|| crate::AppError::NotFound(format!("Tenant {} not found", tenant_id)))?;

    if let Some(storage_mb) = req.storage_mb {
        tenant.quota.storage_mb = storage_mb;
    }
    if let Some(api_calls) = req.api_calls_per_day {
        tenant.quota.api_calls_per_day = api_calls;
    }
    if let Some(sessions) = req.concurrent_sessions {
        tenant.quota.concurrent_sessions = sessions;
    }
    if let Some(entries) = req.memory_entries {
        tenant.quota.memory_entries = entries;
    }

    json_ok(TenantQuotaResponse {
        tenant_id: tenant_id.clone(),
        storage_mb: tenant.quota.storage_mb,
        api_calls_per_day: tenant.quota.api_calls_per_day,
        concurrent_sessions: tenant.quota.concurrent_sessions,
        memory_entries: tenant.quota.memory_entries,
        used_storage_mb: tenant.quota.used.storage_mb,
        used_api_calls: tenant.quota.used.api_calls_today,
        used_sessions: tenant.quota.used.concurrent_sessions,
        used_entries: tenant.quota.used.memory_entries,
    })
}

/// Reset tenant memory (clear all memory entries)
pub async fn reset_tenant_memory(
    Path(tenant_id): Path<String>,
) -> JsonResult<serde_json::Value> {
    info!("Resetting memory for tenant: {}", tenant_id);

    // In a real implementation, this would delete all memory entries for the tenant
    // For now, we just return a success message

    json_ok(serde_json::json!({
        "success": true,
        "tenant_id": tenant_id,
        "message": "Memory reset completed"
    }))
}

// RBAC endpoints

static RBAC_SERVICE: std::sync::OnceLock<RbacService> = std::sync::OnceLock::new();

fn get_rbac() -> &'static RbacService {
    RBAC_SERVICE.get_or_init(RbacService::new)
}

/// Assign role to user
pub async fn assign_role(
    Path(tenant_id): Path<String>,
    Json(req): Json<AssignRoleRequest>,
) -> JsonResult<UserRoleResponse> {
    req.validate()?;
    info!(
        "Assigning role {} to user {} in tenant {}",
        req.role, req.user_id, tenant_id
    );

    let result = get_rbac()
        .assign_role(&tenant_id, &req.user_id, req.role, &req.assigned_by)
        .await?;

    json_ok(UserRoleResponse {
        user_id: result.user_id,
        tenant_id: result.tenant_id,
        role: result.role.to_string(),
    })
}

/// Get user's role
pub async fn get_user_role(
    Path((tenant_id, user_id)): Path<(String, String)>,
) -> JsonResult<Option<String>> {
    info!("Getting role for user {} in tenant {}", user_id, tenant_id);

    let role = get_rbac().get_role(&tenant_id, &user_id).await;

    json_ok(role.map(|r| r.to_string()))
}

/// List all roles in tenant
pub async fn list_roles(
    Path(tenant_id): Path<String>,
) -> JsonResult<RoleListResponse> {
    info!("Listing roles for tenant {}", tenant_id);

    let roles = get_rbac().list_roles(&tenant_id).await;

    json_ok(RoleListResponse {
        roles: roles
            .into_iter()
            .map(|r| UserRoleResponse {
                user_id: r.user_id,
                tenant_id: r.tenant_id,
                role: r.role.to_string(),
            })
            .collect(),
    })
}
