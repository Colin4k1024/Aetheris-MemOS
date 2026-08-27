//! Recall API route — tenant-scoped distilled-memory recall (#84 recall port).
//!
//! `POST /v1/recall { query, user_id, agent_id?, strategy?, max_results?,
//! max_tokens? }` → `RecallResult`. The caller's tenant (from
//! `RequestTenantContext`) scopes the recall — the body carries **no**
//! `tenant_id` (never trusted from the client). Recalls L1 atoms (keyword,
//! PG) + L3 persona + L2 scene navigation from the PG distillation path.

use axum::{extract::Extension, routing::post, Json, Router};
use serde::Deserialize;

use crate::config;
use crate::error::AppError;
use crate::services::recall::{AutoRecallService, RecallRequest, RecallResult, RecallStrategy};
use crate::tenant::RequestTenantContext;

pub fn router() -> Router {
    Router::new().route("/", post(recall))
}

/// Endpoint body — no `tenant_id` (taken from auth). `RecallRequest` is the
/// internal type that carries the auth-derived tenant_id.
#[derive(Deserialize)]
pub struct RecallEndpointRequest {
    pub query: String,
    pub user_id: String,
    pub agent_id: Option<String>,
    pub strategy: Option<RecallStrategy>,
    pub max_results: Option<usize>,
    pub max_tokens: Option<usize>,
    /// RFC 3339 instant for historical belief queries; default now().
    pub as_of: Option<String>,
}

pub async fn recall(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(req): Json<RecallEndpointRequest>,
) -> Result<Json<RecallResult>, AppError> {
    let cfg = config::get();
    // `max_recall_tokens` isn't a RecallConfig field yet — default 2000.
    let service = AutoRecallService::new(
        cfg.recall.timeout_ms,
        cfg.recall.max_l1_results,
        2000,
        cfg.recall.inject_l3_persona,
        true,
    );
    // Belief-core inputs captured before the legacy request consumes them.
    let belief_user = req.user_id.clone();
    let belief_query = req.query.clone();
    let belief_as_of = req.as_of.clone();
    let belief_max_items = req.max_results;
    let belief_budget = req.max_tokens.map(|t| t * 4);

    let request = RecallRequest {
        query: req.query,
        user_id: req.user_id,
        agent_id: req.agent_id,
        tenant_id: tenant_ctx.tenant_id.as_str().to_string(),
        strategy: req.strategy,
        max_results: req.max_results,
        max_tokens: req.max_tokens,
    };
    let mut result = service
        .recall(&request)
        .await
        .map_err(|e| AppError::Internal(format!("recall failed: {e}")))?;

    // #128: the SAME recall core every transport converges on. The belief
    // surface rides along (hard-filtered, citation-carrying); legacy atom /
    // persona / scene results stay untouched for compatibility.
    if let Some(wm) = crate::services::recall::core::belief_working_memory(
        &tenant_ctx.tenant_id,
        Some(&belief_user),
        &belief_query,
        belief_as_of.as_deref(),
        belief_max_items,
        belief_budget,
        &[],
    )
    .await?
    {
        result.beliefs = Some(wm.items);
        result.working_memory_text = Some(wm.text);
    }
    Ok(Json(result))
}
