//! Billing Router
//!
//! API endpoints for billing and usage tracking.

use axum::extract::Path;
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::info;
use validator::Validate;

use crate::services::usage_tracker::{MetricType, SubscriptionTier, UsageTracker};
use crate::{json_ok, JsonResult};

// Global usage tracker instance
static USAGE_TRACKER: std::sync::OnceLock<UsageTracker> = std::sync::OnceLock::new();

fn get_usage_tracker() -> &'static UsageTracker {
    USAGE_TRACKER.get_or_init(UsageTracker::new)
}

/// Initialize tenant billing request
#[derive(Deserialize, Serialize, Validate)]
pub struct InitTenantRequest {
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub tier: SubscriptionTier,
}

/// Record usage request
#[derive(Deserialize, Serialize, Validate)]
pub struct RecordUsageRequest {
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "metricType")]
    pub metric_type: MetricType,
    pub quantity: f64,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// Get usage request
#[derive(Deserialize, Serialize, Validate)]
pub struct GetUsageRequest {
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub start: Option<i64>,
    pub end: Option<i64>,
}

/// Usage response
#[derive(Serialize)]
pub struct UsageResponse {
    pub tenant_id: String,
    pub api_calls: u64,
    pub storage_mb: u64,
    pub cognitive_units: u64,
    pub memory_operations: u64,
    pub vector_queries: u64,
    pub estimated_cost: f64,
}

/// Quota status response
#[derive(Serialize)]
pub struct QuotaStatusResponse {
    pub within_quota: bool,
    pub overage: std::collections::HashMap<String, i64>,
}

/// Initialize tenant for billing
pub async fn init_tenant(Json(req): Json<InitTenantRequest>) -> JsonResult<serde_json::Value> {
    req.validate()?;
    info!(
        "Initializing billing for tenant {} with tier {:?}",
        req.tenant_id, req.tier
    );

    get_usage_tracker()
        .init_tenant(&req.tenant_id, req.tier)
        .await;

    json_ok(serde_json::json!({
        "success": true,
        "tenant_id": req.tenant_id,
        "tier": req.tier
    }))
}

/// Record usage
pub async fn record_usage(Json(req): Json<RecordUsageRequest>) -> JsonResult<serde_json::Value> {
    req.validate()?;
    info!(
        "Recording {} units for tenant {}",
        req.quantity, req.tenant_id
    );

    get_usage_tracker()
        .record_usage(&req.tenant_id, req.metric_type, req.quantity, req.metadata)
        .await?;

    json_ok(serde_json::json!({
        "success": true
    }))
}

/// Get usage for tenant
pub async fn get_usage(Json(req): Json<GetUsageRequest>) -> JsonResult<UsageResponse> {
    req.validate()?;
    info!("Getting usage for tenant {}", req.tenant_id);

    let now = chrono::Utc::now().timestamp();
    let start = req.start.unwrap_or(now - 30 * 24 * 3600); // Default: last 30 days
    let end = req.end.unwrap_or(now);

    let usage = get_usage_tracker()
        .get_usage(&req.tenant_id, start, end)
        .await;

    let estimated_cost = get_usage_tracker().calculate_cost(&req.tenant_id).await;

    json_ok(UsageResponse {
        tenant_id: req.tenant_id,
        api_calls: usage.api_calls,
        storage_mb: usage.storage_mb,
        cognitive_units: usage.cognitive_units,
        memory_operations: usage.memory_operations,
        vector_queries: usage.vector_queries,
        estimated_cost,
    })
}

/// Get current usage
pub async fn get_current_usage(Path(tenant_id): Path<String>) -> JsonResult<UsageResponse> {
    info!("Getting current usage for tenant {}", tenant_id);

    let usage = get_usage_tracker()
        .get_current_usage(&tenant_id)
        .await
        .ok_or_else(|| crate::AppError::NotFound(format!("Tenant {} not found", tenant_id)))?;

    let estimated_cost = get_usage_tracker().calculate_cost(&tenant_id).await;

    json_ok(UsageResponse {
        tenant_id,
        api_calls: usage.api_calls,
        storage_mb: usage.storage_mb,
        cognitive_units: usage.cognitive_units,
        memory_operations: usage.memory_operations,
        vector_queries: usage.vector_queries,
        estimated_cost,
    })
}

/// Get quota status
pub async fn get_quota_status(Path(tenant_id): Path<String>) -> JsonResult<QuotaStatusResponse> {
    info!("Getting quota status for tenant {}", tenant_id);

    let status = get_usage_tracker().check_quota(&tenant_id).await;

    json_ok(QuotaStatusResponse {
        within_quota: status.within_quota,
        overage: status.overage,
    })
}
