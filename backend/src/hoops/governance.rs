//! Governance middleware (P1 子项 b).
//!
//! Wires the enterprise governance hooks (RBAC / quota / audit) into the axum
//! request chain. It sits **inside** the rate limiter and **behind** auth, so the
//! effective order for memory routes is:
//!
//! ```text
//! auth → rate_limit → governance → handler
//! ```
//!
//! For each request that maps to a memory operation (see [`classify`]) it builds a
//! [`HookContext`] from the request-scoped tenant context injected by
//! `hoops::jwt::auth_middleware`, asks the global [`EnterpriseHookSet`] for a
//! pre-hook decision, and turns a [`HookDecision::Deny`] into a `403 Forbidden`.
//!
//! Fail-closed by default: the middleware rejects requests with 403 Forbidden when
//! enterprise hooks are not initialised or the tenant context is missing (genuine
//! misconfiguration). Site operators can opt back to fail-open with
//! `GOVERNANCE_FAIL_CLOSED=false` during migration. Quota *denial* is audited inside
//! the governance hook itself (its `pre_store`/`pre_search` call `record_audit`
//! before returning `Deny`), which now persists via the async audit writer
//! (see `enterprise_impl::create_enterprise_hook_set`).
//!
//! [`EnterpriseHookSet`]: crate::hoops::enterprise::EnterpriseHookSet

use axum::extract::{OriginalUri, Request};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;

use crate::hoops::enterprise::{try_enterprise_hooks, HookContext, HookDecision, Operation};
use crate::services::rbac::{get_rbac_service, Permission};
use crate::tenant::RequestTenantContext;
use crate::AppError;

/// Classify a request into the governed memory [`Operation`], or `None` when the
/// route is not a memory operation we govern (the middleware then lets it through).
///
/// Pure function so it can be unit-tested offline. It expects the **full** request
/// path (including the `/api` prefix), which the middleware sources from
/// [`OriginalUri`] — nested routers otherwise strip the mount prefix.
///
/// | method + path                                   | Operation |
/// |-------------------------------------------------|-----------|
/// | any `…/v1/memory/search/…`, `…/kg/search`       | Search    |
/// | POST `…/v1/memory/storage/…`                    | Store     |
/// | POST `…/kg/entities`, `…/kg/relations`          | Store     |
/// | POST `…/mm/store`                               | Store     |
/// | POST `…/forget`                                 | Delete    |
/// | GET  `…/kg/…`, `…/mm/…` (reads)                 | Search    |
/// | PUT / PATCH `*`                                 | Update    |
/// | DELETE `*`                                      | Delete    |
/// | anything else (non-memory routes)              | None (skip) |
pub fn classify(method: &Method, path: &str) -> Option<Operation> {
    // Search / query paths → Search (regardless of GET vs POST).
    if path.contains("/memory/search") || path.ends_with("/kg/search") {
        return Some(Operation::Search);
    }

    let method = method.as_str();

    // Explicit memory-write paths → Store (writes are always POST).
    if method == "POST"
        && (path.contains("/memory/storage")
            || path.ends_with("/kg/entities")
            || path.ends_with("/kg/relations")
            || path.ends_with("/mm/store"))
    {
        return Some(Operation::Store);
    }

    // POST …/forget → Delete (explicit memory deletion route, before generic fallback).
    if method == "POST" && path.contains("/forget") {
        return Some(Operation::Delete);
    }

    // KG / MM reads → Search (per plan: GET /kg/*, /mm/* run pre_search).
    if method == "GET" && (path.contains("/kg/") || path.contains("/mm/")) {
        return Some(Operation::Search);
    }

    // Generic mutation fallbacks for any governed route.
    match method {
        "PUT" | "PATCH" => Some(Operation::Update),
        "DELETE" => Some(Operation::Delete),
        // Non-memory POST/GET routes (e.g. /adaptive/select, /health) are not governed.
        _ => None,
    }
}

/// Classify a request into the RBAC [`Permission`] required to reach a
/// **privileged admin route**, or `None` when the path is not one of the governed
/// admin surfaces (billing / tenant-admin / snapshot / memory-pool / agent-identity).
///
/// These surfaces are administrative rather than per-request memory operations, so
/// they are gated by an explicit RBAC permission instead of the coarse memory
/// [`Operation`] used by [`classify`]. Mapping each surface to a *semantically
/// correct* permission is what makes the gate real: billing and tenant deletion are
/// Owner-only (see [`crate::services::rbac::Role::has_permission`]) — mapping them to
/// `Read`/`Write` would leave the gate a no-op for any authenticated user.
///
/// Order matters: `/memory-pool` is matched before `/agents` because the pool
/// exposes a `…/memory-pool/agents` sub-route that also contains `/agents`.
///
/// Expects the **full** request path (with the `/api` prefix), sourced from
/// [`OriginalUri`] by the middleware — same contract as [`classify`].
///
/// | path contains  | method | Permission     | why                                    |
/// |----------------|--------|----------------|----------------------------------------|
/// | `/billing`     | any    | ManageBilling  | financial; Owner-only (Admin excluded) |
/// | `/tenants`     | DELETE | DeleteTenant   | destructive; Owner-only                |
/// | `/tenants`     | other  | ManageTenant   | cross-tenant admin; Owner + Admin      |
/// | `/snapshot`    | any    | ManageMemory   | context / working-memory state mgmt    |
/// | `/memory-pool` | any    | ManageAgents   | multi-agent pool coordination          |
/// | `/agents`      | any    | ManageAgents   | agent identity / self-model CRUD       |
/// | anything else  | —      | None (skip)    |                                        |
pub fn classify_permission(method: &Method, path: &str) -> Option<Permission> {
    // Billing — financially sensitive; reads take an arbitrary tenant_id in the
    // path (see routers/billing.rs), so even GETs must be Owner-gated.
    if path.contains("/billing") {
        return Some(Permission::ManageBilling);
    }

    // Tenant administration — list/register plus cross-tenant search/sessions. A
    // tenant *deletion* is destructive and Owner-only; route it to the stricter
    // permission even though no such route exists yet (forward-safe).
    if path.contains("/tenants") {
        return Some(if method.as_str() == "DELETE" {
            Permission::DeleteTenant
        } else {
            Permission::ManageTenant
        });
    }

    // Context / working-memory snapshot, restore and rollback — memory-state mgmt.
    if path.contains("/snapshot") {
        return Some(Permission::ManageMemory);
    }

    // Multi-agent collaborative pool — agent registration + cross-agent memory
    // sharing. MUST precede the `/agents` check (the `…/memory-pool/agents`
    // sub-route also contains `/agents`).
    if path.contains("/memory-pool") {
        return Some(Permission::ManageAgents);
    }

    // Agent identity / self-model / capability / episode / behavior CRUD.
    if path.contains("/agents") {
        return Some(Permission::ManageAgents);
    }

    None
}

/// Map a hook decision to a request outcome: `Deny` → `403 Forbidden`, everything
/// else proceeds. Kept separate from the async middleware so it is unit-testable.
fn decision_outcome(decision: HookDecision) -> Result<(), AppError> {
    match decision {
        HookDecision::Deny(reason) => Err(AppError::Forbidden(reason)),
        HookDecision::Allow | HookDecision::Skip => Ok(()),
    }
}

/// Decide the fail-closed posture from a raw configuration value.
///
/// Split out from [`fail_closed_from_env`] so the policy can be tested as a pure
/// function. Tests must NOT mutate `GOVERNANCE_FAIL_CLOSED` directly: env vars are
/// process-global while Rust runs tests in parallel threads, so doing so makes the
/// suite flaky — and a flaky security test trains people to ignore failures.
fn fail_closed_from_value(value: Option<&str>) -> bool {
    match value {
        // Only these disable fail-closed. Everything else stays secure.
        Some(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "false" | "0" | "no" | "off"
        ),
        // Unset — secure default.
        None => true,
    }
}

/// Resolve the fail-closed posture from the environment.
///
/// Fail-closed is the **default**. Because this is a security control, only an
/// explicit and unambiguous false value opts out — any unrecognised value keeps
/// the secure posture rather than silently downgrading it.
///
/// A naive `v == "true"` check would be a footgun here: an operator writing
/// `GOVERNANCE_FAIL_CLOSED=1` or `=TRUE` intending to *enable* fail-closed would
/// instead have selected fail-open, silently weakening security.
fn fail_closed_from_env() -> bool {
    fail_closed_from_value(std::env::var("GOVERNANCE_FAIL_CLOSED").ok().as_deref())
}

/// Governance middleware — see the module docs for the request-chain position.
pub async fn governance_middleware(req: Request, next: Next) -> Result<Response, AppError> {
    // Fail-closed by default: a missing enterprise hook set or missing tenant
    // context is treated as a security incident and rejected with 403 Forbidden.
    // Set GOVERNANCE_FAIL_CLOSED=false to opt back to fail-open during migration.
    //
    // When JWT auth is disabled (dev mode), skip governance entirely — there is no
    // meaningful tenant/user identity to enforce RBAC against.
    if crate::config::get().jwt.disabled {
        return Ok(next.run(req).await);
    }

    let fail_closed = fail_closed_from_env();

    // Full path (with the /api prefix and mount prefixes), independent of nesting.
    // `OriginalUri` is inserted by the outermost router before any prefix stripping;
    // fall back to the (possibly stripped) live URI only if it is missing.
    let path = req
        .extensions()
        .get::<OriginalUri>()
        .map(|o| o.0.path().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());
    let method = req.method().clone();

    // Privileged admin routes (billing / tenants / snapshot / memory-pool /
    // agents) are gated by an explicit RBAC Permission rather than the coarse
    // memory Operation. This gate is authoritative for those routes, so we do not
    // also run the Operation-based quota path for them.
    if let Some(required) = classify_permission(&method, &path) {
        return enforce_permission(req, next, &path, &method, required, fail_closed).await;
    }

    let Some(operation) = classify(&method, &path) else {
        // Not a governed memory operation — let it through.
        return Ok(next.run(req).await);
    };

    // Enterprise hooks not initialized (e.g. non-PG / feature off) → fail open
    // unless GOVERNANCE_FAIL_CLOSED=true.
    let Some(hooks) = try_enterprise_hooks() else {
        if fail_closed {
            return Err(AppError::Forbidden(
                "governance fail-closed: enterprise hooks not initialized".to_string(),
            ));
        }
        return Ok(next.run(req).await);
    };

    // Tenant context is injected by auth_middleware. Its absence means governance is
    // running ahead of auth (misconfiguration) — fail open unless
    // GOVERNANCE_FAIL_CLOSED=true.
    let Some(tenant_ctx) = req.extensions().get::<RequestTenantContext>().cloned() else {
        if fail_closed {
            return Err(AppError::Forbidden(
                "governance fail-closed: tenant context missing (auth misconfiguration)"
                    .to_string(),
            ));
        }
        return Ok(next.run(req).await);
    };

    let ctx = HookContext::new(tenant_ctx.tenant_id.to_string(), operation, path.clone())
        .with_user(tenant_ctx.user_id.clone())
        .with_param("method", method.as_str());

    let decision = match operation {
        Operation::Store => hooks.pre_store(&ctx),
        Operation::Search => hooks.pre_search(&ctx),
        Operation::Update => hooks.pre_update(&ctx),
        Operation::Delete => hooks.pre_delete(&ctx),
    };

    // Deny is audited inside the governance hook (record_audit → audit writer)
    // before it returns, so we only translate the decision to an HTTP outcome here.
    decision_outcome(decision)?;

    let response = next.run(req).await;

    // After the handler returns, if the operation was a Store and the response
    // indicates success (2xx), invoke the post_store hook so that quota usage is
    // actually counted.  Failed or rejected writes must NOT increment usage.
    if operation == Operation::Store && response.status().is_success() {
        hooks.post_store(&ctx, &crate::hoops::enterprise::HookResult::success());
    }

    Ok(response)
}

/// Enforce the RBAC [`Permission`] a privileged admin route requires.
///
/// Consults the global RBAC service directly — the *same* singleton the enterprise
/// governance hook uses (both go through [`get_rbac_service`]) — so the gate holds
/// even when the enterprise hook set is not initialised (e.g. a SQLite dev backend).
/// It depends only on the request-scoped tenant context injected by
/// `hoops::jwt::auth_middleware`, never on the PG-only hook wiring.
///
/// Today every authenticated user is Owner of their own single-user tenant
/// (`tenant_id == user_id`), so this returns `Allow` for same-tenant callers and
/// only starts denying once an org-level tenant model (where `tenant_id != user_id`)
/// exists — see `services/rbac.rs` for the auto-grant rationale. The gate point is
/// in place now so that separation takes effect automatically when that lands.
async fn enforce_permission(
    req: Request,
    next: Next,
    path: &str,
    method: &Method,
    required: Permission,
    fail_closed: bool,
) -> Result<Response, AppError> {
    // Tenant context is injected by auth_middleware. Its absence means governance is
    // running ahead of auth (misconfiguration) — fail open unless fail-closed.
    let Some(tenant_ctx) = req.extensions().get::<RequestTenantContext>().cloned() else {
        if fail_closed {
            return Err(AppError::Forbidden(
                "governance fail-closed: tenant context missing (auth misconfiguration)"
                    .to_string(),
            ));
        }
        return Ok(next.run(req).await);
    };

    let allowed = get_rbac_service().blocking_has_permission(
        tenant_ctx.tenant_id.as_str(),
        &tenant_ctx.user_id,
        required,
    );

    if !allowed {
        // Audit the denial on the best-effort async writer, mirroring the MCP call
        // path — the enterprise governance hook is not on this route's chain, so we
        // record here rather than relying on its internal audit.
        crate::services::audit_writer::record_audit(
            crate::db::audit::AuditEvent::new("governance.rbac_denied", "governance")
                .tenant(tenant_ctx.tenant_id.as_str())
                .actor(tenant_ctx.user_id.clone())
                .resource_id(path.to_string())
                .with_metadata(&serde_json::json!({
                    "outcome": "denied",
                    "reason": "rbac_permission",
                    "required_permission": format!("{required:?}"),
                    "method": method.as_str(),
                })),
        );
        return Err(AppError::Forbidden(
            "Insufficient role permissions".to_string(),
        ));
    }

    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_memory_storage_writes_as_store() {
        for path in [
            "/api/v1/memory/storage/ltm",
            "/api/v1/memory/storage/stm",
            "/api/v1/memory/storage/transfer",
            "/api/v1/memory/storage/batch-ltm",
            "/api/v1/memory/storage/compress/session",
            "/api/v1/memory/storage/qdrant/backfill-tenant-metadata",
        ] {
            assert_eq!(
                classify(&Method::POST, path),
                Some(Operation::Store),
                "POST {path} should be Store"
            );
        }
    }

    #[test]
    fn classifies_memory_search_as_search_for_get_and_post() {
        assert_eq!(
            classify(&Method::GET, "/api/v1/memory/search/ltm"),
            Some(Operation::Search)
        );
        assert_eq!(
            classify(&Method::POST, "/api/v1/memory/search/ltm"),
            Some(Operation::Search)
        );
        assert_eq!(
            classify(&Method::POST, "/api/v1/memory/search/hybrid"),
            Some(Operation::Search)
        );
    }

    #[test]
    fn classifies_kg_writes_and_reads() {
        assert_eq!(
            classify(&Method::POST, "/api/kg/entities"),
            Some(Operation::Store)
        );
        assert_eq!(
            classify(&Method::POST, "/api/kg/relations"),
            Some(Operation::Store)
        );
        // KG search is a query, not a write.
        assert_eq!(
            classify(&Method::POST, "/api/kg/search"),
            Some(Operation::Search)
        );
        // KG reads.
        assert_eq!(
            classify(&Method::GET, "/api/kg/entities"),
            Some(Operation::Search)
        );
        assert_eq!(
            classify(&Method::GET, "/api/kg/entities/by-name/foo"),
            Some(Operation::Search)
        );
    }

    #[test]
    fn classifies_mm_writes_and_reads() {
        assert_eq!(
            classify(&Method::POST, "/api/mm/store"),
            Some(Operation::Store)
        );
        assert_eq!(
            classify(&Method::GET, "/api/mm/list"),
            Some(Operation::Search)
        );
        assert_eq!(
            classify(&Method::GET, "/api/mm/entry/abc"),
            Some(Operation::Search)
        );
    }

    #[test]
    fn classifies_forget_as_delete() {
        assert_eq!(
            classify(&Method::POST, "/api/v1/memory/forget"),
            Some(Operation::Delete)
        );
        // Other methods on /forget are not governed (POST is the canonical delete verb).
        assert_eq!(classify(&Method::GET, "/api/v1/memory/forget"), None);
    }

    #[test]
    fn classifies_mutations_by_method() {
        assert_eq!(
            classify(&Method::PUT, "/api/v1/memory/configs/abc"),
            Some(Operation::Update)
        );
        assert_eq!(
            classify(&Method::PATCH, "/api/v1/memory/configs/abc"),
            Some(Operation::Update)
        );
        assert_eq!(
            classify(&Method::DELETE, "/api/v1/memory/configs/abc"),
            Some(Operation::Delete)
        );
    }

    #[test]
    fn skips_non_memory_operations() {
        // POST that is not a storage/kg/mm write.
        assert_eq!(
            classify(&Method::POST, "/api/v1/memory/adaptive/select"),
            None
        );
        // POST /forget is a memory deletion, governed as Delete.
        assert_eq!(
            classify(&Method::POST, "/api/v1/memory/forget"),
            Some(Operation::Delete)
        );
        // Memory reads that are not search endpoints.
        assert_eq!(classify(&Method::GET, "/api/v1/memory/traces"), None);
        assert_eq!(classify(&Method::GET, "/api/v1/memory/health"), None);
        assert_eq!(
            classify(&Method::GET, "/api/v1/memory/storage/sessions"),
            None
        );
    }

    #[test]
    fn deny_decision_maps_to_403_forbidden() {
        let err = decision_outcome(HookDecision::Deny("quota exceeded".to_string()))
            .expect_err("Deny must be an error");
        assert!(matches!(err, AppError::Forbidden(_)));

        use axum::response::IntoResponse;
        let status = err.into_response().status();
        assert_eq!(status, axum::http::StatusCode::FORBIDDEN);
    }

    #[test]
    fn allow_and_skip_decisions_proceed() {
        assert!(decision_outcome(HookDecision::Allow).is_ok());
        assert!(decision_outcome(HookDecision::Skip).is_ok());
    }

    /// Policy tests target the pure [`fail_closed_from_value`] rather than mutating
    /// `GOVERNANCE_FAIL_CLOSED`. Env vars are process-global and Rust runs tests in
    /// parallel threads, so mutating them here made the suite intermittently fail.
    #[test]
    fn fail_closed_defaults_to_true_when_unset() {
        assert!(
            fail_closed_from_value(None),
            "default must be fail-closed (true)"
        );
    }

    /// The documented opt-out must work in its common spellings and be
    /// case/whitespace tolerant, so a deliberate migration escape hatch is not
    /// defeated by formatting.
    #[test]
    fn recognised_false_values_select_fail_open() {
        for v in ["false", "FALSE", "False", "0", "no", "off", " false "] {
            assert!(
                !fail_closed_from_value(Some(v)),
                "value {v:?} must select fail-open"
            );
        }
    }

    /// Regression guard: an unrecognised value must NOT silently downgrade the
    /// security posture. An operator writing `1` / `TRUE` / `yes` intends to
    /// enable fail-closed, and a bare typo must never select fail-open.
    #[test]
    fn unrecognised_values_keep_fail_closed() {
        for v in [
            "true", "TRUE", "True", "1", "yes", "on", "", "  ", "nonsense",
        ] {
            assert!(
                fail_closed_from_value(Some(v)),
                "value {v:?} must keep the secure fail-closed posture"
            );
        }
    }

    /// The env-reading wrapper is exercised once, without mutation, to prove it
    /// delegates to the policy function above.
    #[test]
    fn env_wrapper_delegates_to_policy() {
        // Whatever the ambient env happens to be, the wrapper must agree with the
        // policy applied to that same value — no independent logic of its own.
        let ambient = std::env::var("GOVERNANCE_FAIL_CLOSED").ok();
        assert_eq!(
            fail_closed_from_env(),
            fail_closed_from_value(ambient.as_deref())
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // classify_permission — privileged admin route gate (billing / tenants /
    // snapshot / memory-pool / agents). Backlog C-1 remediation. These mirror the
    // MCP `tool_to_operation` mapping tests in `routers/mcp.rs`.
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn billing_routes_require_manage_billing() {
        for path in [
            "/api/v1/memory/billing/init",
            "/api/v1/memory/billing/usage",
            "/api/v1/memory/billing/quota/tenant-x",
            "/api/v1/memory/billing/record",
        ] {
            assert_eq!(
                classify_permission(&Method::POST, path),
                Some(Permission::ManageBilling),
                "billing route {path} must require ManageBilling"
            );
        }
        // A read that takes an arbitrary tenant_id in the path is just as sensitive
        // — it must be Owner-gated too, not mapped to Read.
        assert_eq!(
            classify_permission(&Method::GET, "/api/v1/memory/billing/usage/tenant-x"),
            Some(Permission::ManageBilling)
        );
    }

    #[test]
    fn tenant_admin_routes_require_manage_tenant() {
        for (method, path) in [
            (Method::GET, "/api/tenants/"),
            (Method::POST, "/api/tenants/"),
            (Method::POST, "/api/tenants/tenant-x/search"),
            (Method::GET, "/api/tenants/tenant-x/sessions"),
            (Method::POST, "/api/tenants/access/check"),
        ] {
            assert_eq!(
                classify_permission(&method, path),
                Some(Permission::ManageTenant),
                "tenant admin route {path} must require ManageTenant"
            );
        }
    }

    #[test]
    fn tenant_deletion_requires_owner_only_delete_tenant() {
        // No DELETE /tenants route exists today, but if one is added it must map to
        // the Owner-only DeleteTenant permission, not the weaker ManageTenant that
        // Admin also holds.
        assert_eq!(
            classify_permission(&Method::DELETE, "/api/tenants/tenant-x"),
            Some(Permission::DeleteTenant)
        );
    }

    #[test]
    fn snapshot_routes_require_manage_memory() {
        for path in [
            "/api/v1/memory/snapshot/task",
            "/api/v1/memory/snapshot/create",
            "/api/v1/memory/snapshot/restore",
            "/api/v1/memory/snapshot/rollback",
            "/api/v1/memory/snapshot/checkpoints/task-x",
        ] {
            assert_eq!(
                classify_permission(&Method::POST, path),
                Some(Permission::ManageMemory),
                "snapshot route {path} must require ManageMemory"
            );
        }
    }

    #[test]
    fn memory_pool_routes_require_manage_agents() {
        for path in [
            "/api/v1/memory/memory-pool/register",
            "/api/v1/memory/memory-pool/share/agent-x",
            "/api/v1/memory/memory-pool/network",
            // The pool's own /agents sub-route must be caught by the memory-pool
            // branch, not fall through to the agent-identity branch — this locks
            // the match order in place.
            "/api/v1/memory/memory-pool/agents",
        ] {
            assert_eq!(
                classify_permission(&Method::POST, path),
                Some(Permission::ManageAgents),
                "memory-pool route {path} must require ManageAgents"
            );
        }
    }

    #[test]
    fn agent_identity_routes_require_manage_agents() {
        for (method, path) in [
            (Method::POST, "/api/v1/agents"),
            (Method::GET, "/api/v1/agents"),
            (Method::PUT, "/api/v1/agents/agent-x"),
            (Method::DELETE, "/api/v1/agents/agent-x"),
            (Method::GET, "/api/v1/agents/agent-x/self-model"),
            (Method::POST, "/api/v1/agents/agent-x/capabilities"),
        ] {
            assert_eq!(
                classify_permission(&method, path),
                Some(Permission::ManageAgents),
                "agent route {path} must require ManageAgents"
            );
        }
    }

    #[test]
    fn ordinary_memory_routes_are_not_permission_gated() {
        // Data-plane memory routes are governed by classify()/Operation, NOT by the
        // admin permission gate — the two classifiers must not overlap, or the
        // Operation-based quota path would be skipped for a genuine memory write.
        for path in [
            "/api/v1/memory/search/ltm",
            "/api/v1/memory/storage/ltm",
            "/api/v1/memory/forget",
            "/api/kg/entities",
            "/api/mm/list",
            "/api/v1/memory/adaptive/select",
            "/api/v1/memory/health",
        ] {
            assert_eq!(
                classify_permission(&Method::POST, path),
                None,
                "{path} must not be caught by the admin permission gate"
            );
        }
    }

    /// Anti-drift guard: every privileged admin surface must map to *some*
    /// permission. If a future refactor drops or weakens a mapping — making the
    /// route reachable by any authenticated user again — this fails. Mirrors the
    /// MCP `all_known_tools_are_covered` guard in `routers/mcp.rs`.
    #[test]
    fn all_privileged_admin_surfaces_stay_permission_gated() {
        let surfaces = [
            ("/api/v1/memory/billing/record", Permission::ManageBilling),
            ("/api/tenants/", Permission::ManageTenant),
            ("/api/v1/memory/snapshot/restore", Permission::ManageMemory),
            (
                "/api/v1/memory/memory-pool/register",
                Permission::ManageAgents,
            ),
            ("/api/v1/agents", Permission::ManageAgents),
        ];
        for (path, expected) in surfaces {
            assert_eq!(
                classify_permission(&Method::POST, path),
                Some(expected),
                "privileged surface {path} must stay permission-gated"
            );
        }
    }
}
