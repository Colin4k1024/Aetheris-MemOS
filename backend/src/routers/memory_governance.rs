//! Memory governance API (#130) — the user/admin surface over the belief
//! lifecycle: view current/history beliefs, provenance traces, confirmation
//! queues, correct/archive/forget, rollback, and principal merge management.
//!
//! Permission model (issue: "用户只能查看和管理自己有权限的记忆；管理员操作
//! 经过现有 governance/RBAC/audit"):
//! - Reads: any authenticated member of the tenant. NON-admin callers are
//!   pinned to their own subject (`principal:{id}`) — resolved from the JWT
//!   alias, never from a client-supplied field.
//! - Mutations (confirm/deny/archive/rollback/forget/merge/unmerge): Admin or
//!   Owner role via `RbacService` (`tenant_members`; no row = denied).
//! - Every mutation writes an audit row (`memory_audit_events` via the repos
//!   + `audit_writer` for the HTTP-level record).
//!
//! Tenant comes from `RequestTenantContext` (auth), never the body — the same
//! rule every other tenant-scoped router in this crate follows.

use axum::{
    extract::{Extension, Path, Query},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::belief::BeliefRepository;
use crate::db::principal::PrincipalRepository;
use crate::error::AppError;
use crate::models::belief_record::{
    AuditTraceRow, MemoryBelief, MemoryBeliefCandidate, MemoryBeliefEvidence,
};
use crate::services::audit_writer;
use crate::services::identity::IdentityService;
use crate::services::rbac::{get_rbac_service, Role};
use crate::tenant::{RequestTenantContext, TenantId};

pub fn router() -> Router {
    Router::new()
        .route("/beliefs", get(list_beliefs))
        .route("/beliefs/{id}", get(get_belief))
        .route("/beliefs/{id}/trace", get(belief_trace))
        .route("/beliefs/{id}/confirm", post(confirm_belief))
        .route("/beliefs/{id}/deny", post(deny_belief))
        .route("/beliefs/{id}/archive", post(archive_belief))
        .route("/beliefs/{id}/rollback", post(rollback_belief))
        .route("/subjects/{subject}/forget", post(forget_subject))
        .route("/candidates", get(list_candidates))
        .route("/principals/{id}/aliases", get(principal_aliases))
        .route("/principals/merge", post(merge_principal))
        .route("/principals/unmerge", post(unmerge_principal))
        .route("/stats", get(governance_stats))
        .route("/contracts", get(list_contracts))
        .route("/contracts/{agent_id}", put(upsert_contract))
        .route("/self/correct", post(self_correct))
        .route("/self/forget", post(self_forget))
}

// ============================================================================
// Shared helpers
// ============================================================================

fn repo() -> BeliefRepository {
    BeliefRepository::new(crate::db::pool().clone())
}

/// Caller's effective role in the path tenant (None = no membership row =
/// denied for mutations; reads still allowed for self-scope).
async fn caller_role(tenant_ctx: &RequestTenantContext) -> Option<Role> {
    get_rbac_service()
        .get_role(tenant_ctx.tenant_id.as_str(), &tenant_ctx.user_id)
        .await
}

fn require_admin(role: Option<Role>) -> Result<Role, AppError> {
    match role {
        Some(r @ (Role::Admin | Role::Owner)) => Ok(r),
        _ => Err(AppError::Forbidden(
            "memory governance mutations require the Admin or Owner role".to_string(),
        )),
    }
}

/// Resolve the caller's own subject; non-admin reads are pinned to it.
async fn own_subject(tenant_ctx: &RequestTenantContext) -> Result<String, AppError> {
    let principals = PrincipalRepository::new(crate::db::pool().clone());
    let principal = principals
        .find_by_alias(
            &tenant_ctx.tenant_id,
            crate::models::principal::PrincipalAliasType::JwtSub,
            &tenant_ctx.user_id,
        )
        .await?
        .ok_or_else(|| AppError::NotFound("no principal mapped for the caller yet".to_string()))?;
    Ok(format!("principal:{}", principal.id))
}

fn record_http_audit(tenant_ctx: &RequestTenantContext, action: &str, resource: &str) {
    audit_writer::record_audit(
        crate::db::audit::AuditEvent::new(action, "memory_governance")
            .tenant(tenant_ctx.tenant_id.as_str())
            .actor(&tenant_ctx.user_id)
            .resource_id(resource),
    );
}

// ============================================================================
// Reads
// ============================================================================

#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
pub struct ListBeliefsQuery {
    pub subject: Option<String>,
    pub predicate: Option<String>,
    /// Include superseded/archived history (admin-only view widening).
    #[serde(default)]
    pub include_history: bool,
    #[serde(default)]
    pub limit: Option<i64>,
}

/// List beliefs. Non-admin callers always see ONLY their own subject.
#[utoipa::path(
    get,
    path = "/api/v1/governance/beliefs",
    tag = "memory-governance",
    params(ListBeliefsQuery),
    responses((status = 200, body = GovernanceBeliefList, description = "Beliefs visible to the caller"))
)]
pub async fn list_beliefs(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Query(query): Query<ListBeliefsQuery>,
) -> Result<Json<GovernanceBeliefList>, AppError> {
    let role = caller_role(&tenant_ctx).await;
    let is_admin = matches!(role, Some(Role::Admin | Role::Owner));
    // Non-admin: pin to own subject, no history widening.
    let (subject, include_history) = if is_admin {
        (query.subject.clone(), query.include_history)
    } else {
        (Some(own_subject(&tenant_ctx).await?), false)
    };

    let beliefs = repo()
        .list_beliefs(
            &tenant_ctx.tenant_id,
            subject.as_deref(),
            query.predicate.as_deref(),
            include_history,
            query.limit.unwrap_or(200),
        )
        .await?;
    Ok(Json(GovernanceBeliefList { beliefs }))
}

/// Single belief with its provenance evidence.
#[utoipa::path(
    get,
    path = "/api/v1/governance/beliefs/{id}",
    tag = "memory-governance",
    responses((status = 200, body = GovernanceBeliefDetail, description = "Belief + evidence"))
)]
pub async fn get_belief(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<GovernanceBeliefDetail>, AppError> {
    let Some((belief, evidence, _)) = repo().belief_trace(&tenant_ctx.tenant_id, &id).await? else {
        return Err(AppError::NotFound(format!("belief '{id}' not found")));
    };
    enforce_subject_scope(&tenant_ctx, &belief).await?;
    Ok(Json(GovernanceBeliefDetail {
        belief,
        evidence,
        audit: vec![],
    }))
}

/// Full trace: belief + evidence + audit chain (#124 acceptance 5 surface).
#[utoipa::path(
    get,
    path = "/api/v1/governance/beliefs/{id}/trace",
    tag = "memory-governance",
    responses((status = 200, body = GovernanceBeliefDetail, description = "belief + provenance + audit chain"))
)]
pub async fn belief_trace(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<GovernanceBeliefDetail>, AppError> {
    let Some((belief, evidence, audit)) = repo().belief_trace(&tenant_ctx.tenant_id, &id).await?
    else {
        return Err(AppError::NotFound(format!("belief '{id}' not found")));
    };
    enforce_subject_scope(&tenant_ctx, &belief).await?;
    Ok(Json(GovernanceBeliefDetail {
        belief,
        evidence,
        audit,
    }))
}

async fn enforce_subject_scope(
    tenant_ctx: &RequestTenantContext,
    belief: &MemoryBelief,
) -> Result<(), AppError> {
    let role = caller_role(tenant_ctx).await;
    if matches!(role, Some(Role::Admin | Role::Owner)) {
        return Ok(());
    }
    let own = own_subject(tenant_ctx).await?;
    if belief.subject != own {
        return Err(AppError::Forbidden(
            "callers may only inspect their own beliefs".to_string(),
        ));
    }
    Ok(())
}

#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
pub struct ListCandidatesQuery {
    /// pending | quarantined (the two governance queues).
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

/// The confirmation / quarantine queues. Admin-only: the queues exist
/// precisely because their contents need a human with authority.
#[utoipa::path(
    get,
    path = "/api/v1/governance/candidates",
    tag = "memory-governance",
    params(ListCandidatesQuery),
    responses((status = 200, body = GovernanceCandidateList, description = "Queue contents"))
)]
pub async fn list_candidates(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Query(query): Query<ListCandidatesQuery>,
) -> Result<Json<GovernanceCandidateList>, AppError> {
    require_admin(caller_role(&tenant_ctx).await)?;
    let status = query.status.as_deref().unwrap_or("pending");
    if !["pending", "quarantined"].contains(&status) {
        return Err(AppError::BadRequest(
            "status must be 'pending' or 'quarantined'".to_string(),
        ));
    }
    let candidates = repo()
        .list_candidates(
            &tenant_ctx.tenant_id,
            Some(status),
            query.limit.unwrap_or(100),
        )
        .await?;
    Ok(Json(GovernanceCandidateList { candidates }))
}

#[utoipa::path(
    get,
    path = "/api/v1/governance/principals/{id}/aliases",
    tag = "memory-governance",
    responses((status = 200, body = GovernanceAliasList, description = "Principal aliases"))
)]
pub async fn principal_aliases(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<GovernanceAliasList>, AppError> {
    require_admin(caller_role(&tenant_ctx).await)?;
    let aliases = PrincipalRepository::new(crate::db::pool().clone())
        .list_aliases(&tenant_ctx.tenant_id, &id)
        .await?;
    Ok(Json(GovernanceAliasList {
        aliases: aliases
            .into_iter()
            .map(|(t, v)| GovernanceAlias {
                alias_type: t,
                alias_value: v,
            })
            .collect(),
    }))
}

/// Volume snapshot for the governance dashboard.
#[utoipa::path(
    get,
    path = "/api/v1/governance/stats",
    tag = "memory-governance",
    responses((status = 200, body = GovernanceStats, description = "Belief volume + queue depths"))
)]
pub async fn governance_stats(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
) -> Result<Json<GovernanceStats>, AppError> {
    let r = repo();
    let tenant = &tenant_ctx.tenant_id;
    let active = r.active_belief_count(tenant).await?;
    let pending = r.list_candidates(tenant, Some("pending"), 1).await?.len();
    // list_candidates caps at 1 row for a cheap existence check; count properly:
    let pending_count = r.list_candidates(tenant, Some("pending"), 500).await?.len() as i64;
    let quarantined_count = r
        .list_candidates(tenant, Some("quarantined"), 500)
        .await?
        .len() as i64;
    let _ = pending;
    crate::services::prometheus_exporter::get_exporter().set_belief_active(active as f64);
    Ok(Json(GovernanceStats {
        active_beliefs: active,
        pending_confirm: pending_count,
        quarantined: quarantined_count,
    }))
}

// ============================================================================
// Mutations (Admin/Owner + audit)
// ============================================================================

#[utoipa::path(
    post,
    path = "/api/v1/governance/beliefs/{id}/confirm",
    tag = "memory-governance",
    responses((status = 200, body = GovernanceMutationResult, description = "Belief confirmed"))
)]
pub async fn confirm_belief(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<GovernanceMutationResult>, AppError> {
    require_admin(caller_role(&tenant_ctx).await)?;
    let belief = repo()
        .confirm_belief(&tenant_ctx.tenant_id, &id, Some(&tenant_ctx.user_id))
        .await?;
    record_http_audit(&tenant_ctx, "governance.belief_confirmed", &id);
    Ok(Json(GovernanceMutationResult {
        ok: true,
        belief_id: id,
        detail: Some(belief.status),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/governance/beliefs/{id}/deny",
    tag = "memory-governance",
    responses((status = 200, body = GovernanceMutationResult, description = "Belief denied/rejected"))
)]
pub async fn deny_belief(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<GovernanceMutationResult>, AppError> {
    require_admin(caller_role(&tenant_ctx).await)?;
    let changed = repo()
        .deny_belief(&tenant_ctx.tenant_id, &id, Some(&tenant_ctx.user_id))
        .await?;
    record_http_audit(&tenant_ctx, "governance.belief_denied", &id);
    Ok(Json(GovernanceMutationResult {
        ok: changed,
        belief_id: id,
        detail: Some("rejected".to_string()),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/governance/beliefs/{id}/archive",
    tag = "memory-governance",
    responses((status = 200, body = GovernanceMutationResult, description = "Belief archived"))
)]
pub async fn archive_belief(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<GovernanceMutationResult>, AppError> {
    // Admin may archive anything; a plain member may archive only beliefs on
    // their OWN subject (#124: users can delete their own memories).
    let role = caller_role(&tenant_ctx).await;
    if !matches!(role, Some(Role::Admin | Role::Owner)) {
        let Some((belief, _, _)) = repo().belief_trace(&tenant_ctx.tenant_id, &id).await? else {
            return Err(AppError::NotFound(format!("belief '{id}' not found")));
        };
        let own = own_subject(&tenant_ctx).await?;
        if belief.subject != own {
            return Err(AppError::Forbidden(
                "callers may only archive their own beliefs".to_string(),
            ));
        }
    }
    let changed = repo()
        .archive_belief(&tenant_ctx.tenant_id, &id, Some(&tenant_ctx.user_id))
        .await?;
    record_http_audit(&tenant_ctx, "governance.belief_archived", &id);
    Ok(Json(GovernanceMutationResult {
        ok: changed,
        belief_id: id,
        detail: Some("archived".to_string()),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/governance/beliefs/{id}/rollback",
    tag = "memory-governance",
    responses((status = 200, body = GovernanceRollbackResult, description = "Rolled back to predecessor"))
)]
pub async fn rollback_belief(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<GovernanceRollbackResult>, AppError> {
    require_admin(caller_role(&tenant_ctx).await)?;
    let (closed, restored) = repo()
        .rollback_belief(&tenant_ctx.tenant_id, &id, Some(&tenant_ctx.user_id))
        .await?;
    record_http_audit(&tenant_ctx, "governance.belief_rolled_back", &id);
    Ok(Json(GovernanceRollbackResult {
        ok: true,
        closed_belief_id: closed,
        restored_belief_id: restored,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/governance/subjects/{subject}/forget",
    tag = "memory-governance",
    responses((status = 200, body = GovernanceMutationResult, description = "Subject's open beliefs archived"))
)]
pub async fn forget_subject(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    axum::extract::Path(subject): axum::extract::Path<String>,
) -> Result<Json<GovernanceMutationResult>, AppError> {
    require_admin(caller_role(&tenant_ctx).await)?;
    let n = repo()
        .forget_subject(&tenant_ctx.tenant_id, &subject, Some(&tenant_ctx.user_id))
        .await?;
    record_http_audit(&tenant_ctx, "governance.subject_forgotten", &subject);
    Ok(Json(GovernanceMutationResult {
        ok: true,
        belief_id: subject,
        detail: Some(format!("{n} edges archived")),
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MergePrincipalRequest {
    pub anonymous_principal_id: String,
    pub person_principal_id: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/governance/principals/merge",
    tag = "memory-governance",
    request_body = MergePrincipalRequest,
    responses((status = 200, body = GovernanceMutationResult, description = "Anonymous merged into person"))
)]
pub async fn merge_principal(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(req): Json<MergePrincipalRequest>,
) -> Result<Json<GovernanceMutationResult>, AppError> {
    require_admin(caller_role(&tenant_ctx).await)?;
    let svc = IdentityService::new(crate::db::pool().clone());
    let root = svc
        .merge_anonymous_into_person(
            &tenant_ctx.tenant_id,
            &req.anonymous_principal_id,
            &req.person_principal_id,
            Some(&tenant_ctx.user_id),
        )
        .await?;
    record_http_audit(
        &tenant_ctx,
        "governance.principal_merged",
        &req.anonymous_principal_id,
    );
    Ok(Json(GovernanceMutationResult {
        ok: true,
        belief_id: req.anonymous_principal_id,
        detail: Some(root.id),
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UnmergePrincipalRequest {
    pub anonymous_principal_id: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/governance/principals/unmerge",
    tag = "memory-governance",
    request_body = UnmergePrincipalRequest,
    responses((status = 200, body = GovernanceMutationResult, description = "Merge reverted"))
)]
pub async fn unmerge_principal(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(req): Json<UnmergePrincipalRequest>,
) -> Result<Json<GovernanceMutationResult>, AppError> {
    require_admin(caller_role(&tenant_ctx).await)?;
    let svc = IdentityService::new(crate::db::pool().clone());
    let previous = svc
        .unmerge_anonymous(
            &tenant_ctx.tenant_id,
            &req.anonymous_principal_id,
            Some(&tenant_ctx.user_id),
        )
        .await?;
    record_http_audit(
        &tenant_ctx,
        "governance.principal_unmerged",
        &req.anonymous_principal_id,
    );
    Ok(Json(GovernanceMutationResult {
        ok: true,
        belief_id: req.anonymous_principal_id,
        detail: Some(previous),
    }))
}

// ============================================================================
// Contract management (#124 gap: the table existed and was enforced by
// recall, but had no management surface)
// ============================================================================

/// List the tenant's agent memory contracts.
#[utoipa::path(
    get,
    path = "/api/v1/governance/contracts",
    tag = "memory-governance",
    responses((status = 200, body = GovernanceContractList, description = "Contracts"))
)]
pub async fn list_contracts(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
) -> Result<Json<GovernanceContractList>, AppError> {
    require_admin(caller_role(&tenant_ctx).await)?;
    let contracts = repo()
        .list_contracts(&tenant_ctx.tenant_id)
        .await?
        .into_iter()
        .map(|row| GovernanceContract {
            agent_id: row.agent_id,
            may_believe: row.may_believe,
            must_not_believe_from: row.must_not_believe_from,
            high_stakes_deny_below_trust: row.high_stakes_deny_below_trust,
            enabled: row.enabled,
        })
        .collect();
    Ok(Json(GovernanceContractList { contracts }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertContractRequest {
    /// JSON array of predicate patterns this agent may believe.
    #[serde(default)]
    pub may_believe: serde_json::Value,
    /// JSON object { source: [predicates] | "*" } the agent must NOT believe.
    #[serde(default)]
    pub must_not_believe_from: serde_json::Value,
    /// High-risk beliefs below this trust can never drive this agent's actions.
    pub high_stakes_deny_below_trust: Option<f32>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// Create or replace one agent's memory contract (admin, audited). Malformed
/// JSON bodies are rejected by the DB's jsonb cast — no partial writes.
#[utoipa::path(
    put,
    path = "/api/v1/governance/contracts/{agent_id}",
    tag = "memory-governance",
    request_body = UpsertContractRequest,
    responses((status = 200, body = GovernanceContract, description = "Contract stored"))
)]
pub async fn upsert_contract(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Path(agent_id): Path<String>,
    Json(req): Json<UpsertContractRequest>,
) -> Result<Json<GovernanceContract>, AppError> {
    require_admin(caller_role(&tenant_ctx).await)?;
    let row = repo()
        .upsert_contract(
            &tenant_ctx.tenant_id,
            &agent_id,
            &req.may_believe.to_string(),
            &req.must_not_believe_from.to_string(),
            req.high_stakes_deny_below_trust,
            req.enabled,
            Some(&tenant_ctx.user_id),
        )
        .await?;
    record_http_audit(&tenant_ctx, "governance.contract_upserted", &agent_id);
    Ok(Json(GovernanceContract {
        agent_id: row.agent_id,
        may_believe: row.may_believe,
        must_not_believe_from: row.must_not_believe_from,
        high_stakes_deny_below_trust: row.high_stakes_deny_below_trust,
        enabled: row.enabled,
    }))
}

// ============================================================================
// User self-service (#124: "用户可看见、可改、可删自己的记忆")
// ============================================================================
//
// 可看: the pinned member read (list/get/trace). 可删: forget/archive of the
// caller's OWN subject. 可改: a correction claim submitted through the SAME
// write gate as every other belief — the user outranks nothing; policies,
// precedence and quarantine apply unchanged. Confirmation/rollback stay
// admin-only: they change org-wide truth, not personal data.

#[derive(Debug, Deserialize, ToSchema)]
pub struct SelfCorrectRequest {
    pub predicate: String,
    pub object: String,
    /// Free-form correction note kept as payload (auditable context).
    #[serde(default)]
    pub note: Option<String>,
}

/// Submit a correction for the caller's OWN beliefs through the write gate.
#[utoipa::path(
    post,
    path = "/api/v1/governance/self/correct",
    tag = "memory-governance",
    request_body = SelfCorrectRequest,
    responses((status = 200, body = GovernanceSelfCorrectResult, description = "Gate verdict"))
)]
pub async fn self_correct(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(req): Json<SelfCorrectRequest>,
) -> Result<Json<GovernanceSelfCorrectResult>, AppError> {
    let subject = own_subject(&tenant_ctx).await?;
    let principal_id = subject
        .strip_prefix("principal:")
        .map(str::to_string)
        .unwrap_or_default();

    // Evidence: an immutable event recording the user's correction turn.
    let events = crate::db::memory_event::MemoryEventRepository::new(crate::db::pool().clone());
    let event = events
        .append(
            &tenant_ctx.tenant_id,
            crate::models::memory_event::AppendMemoryEventRequest::new(
                principal_id.clone(),
                crate::models::memory_event::MemoryEventType::UserMessage,
            )
            .actor(&tenant_ctx.user_id)
            .payload(serde_json::json!({
                "kind": "self_correction",
                "predicate": req.predicate,
                "object": req.object,
                "note": req.note,
            }))
            .idempotency_key(format!(
                "selfcorrect|{}|{}|{}|{}",
                tenant_ctx.user_id, req.predicate, req.object, subject
            )),
        )
        .await?;

    let claim = crate::models::belief_record::BeliefClaim::new(
        principal_id,
        &subject,
        &req.predicate,
        &req.object,
        crate::models::belief::BeliefSource::UserStated,
    )
    .origin(crate::models::belief_record::ClaimOrigin::Api)
    .evidence(vec![event.id().to_string()])
    .payload(serde_json::json!({ "self_correction": true, "note": req.note }))
    .idempotency_key(format!(
        "selfcorrect|{}|{}|{}",
        tenant_ctx.user_id, req.predicate, req.object
    ));

    let outcome = crate::services::belief::BeliefGateService::new(crate::db::pool().clone())
        .submit(&tenant_ctx.tenant_id, claim)
        .await?;
    record_http_audit(&tenant_ctx, "governance.self_correct", &req.predicate);
    Ok(Json(GovernanceSelfCorrectResult {
        decision: format!("{outcome:?}"),
    }))
}

/// GDPR self-forget: archive every open belief on the caller's OWN subject.
#[utoipa::path(
    post,
    path = "/api/v1/governance/self/forget",
    tag = "memory-governance",
    responses((status = 200, body = GovernanceMutationResult, description = "Own subject archived"))
)]
pub async fn self_forget(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
) -> Result<Json<GovernanceMutationResult>, AppError> {
    let subject = own_subject(&tenant_ctx).await?;
    let n = repo()
        .forget_subject(&tenant_ctx.tenant_id, &subject, Some(&tenant_ctx.user_id))
        .await?;
    record_http_audit(&tenant_ctx, "governance.self_forgotten", &subject);
    Ok(Json(GovernanceMutationResult {
        ok: true,
        belief_id: subject,
        detail: Some(format!("{n} edges archived")),
    }))
}

// ============================================================================
// Wire types
// ============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct GovernanceBeliefList {
    pub beliefs: Vec<MemoryBelief>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GovernanceBeliefDetail {
    pub belief: MemoryBelief,
    pub evidence: Vec<MemoryBeliefEvidence>,
    pub audit: Vec<AuditTraceRow>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GovernanceCandidateList {
    pub candidates: Vec<MemoryBeliefCandidate>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GovernanceAliasList {
    pub aliases: Vec<GovernanceAlias>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GovernanceAlias {
    pub alias_type: String,
    pub alias_value: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GovernanceStats {
    pub active_beliefs: i64,
    pub pending_confirm: i64,
    pub quarantined: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GovernanceMutationResult {
    pub ok: bool,
    pub belief_id: String,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GovernanceContractList {
    pub contracts: Vec<GovernanceContract>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GovernanceContract {
    pub agent_id: String,
    pub may_believe: String,
    pub must_not_believe_from: String,
    pub high_stakes_deny_below_trust: Option<f32>,
    pub enabled: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GovernanceSelfCorrectResult {
    /// The write gate's verdict (Committed/Superseded/Noop/Conflict/Quarantined/Rejected).
    pub decision: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GovernanceRollbackResult {
    ok: bool,
    closed_belief_id: String,
    restored_belief_id: String,
}
