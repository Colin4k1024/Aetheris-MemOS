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
//! Fail-open by design: if the enterprise hooks are not initialized, no tenant
//! context is present, or the path is not a governed memory operation, the request
//! proceeds untouched. Quota *denial* is audited inside the governance hook itself
//! (its `pre_store`/`pre_search` call `record_audit` before returning `Deny`), which
//! now persists via the async audit writer (see `enterprise_impl::create_enterprise_hook_set`).
//!
//! [`EnterpriseHookSet`]: crate::hoops::enterprise::EnterpriseHookSet

use axum::extract::{OriginalUri, Request};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;

use crate::hoops::enterprise::{try_enterprise_hooks, HookContext, HookDecision, Operation};
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

/// Map a hook decision to a request outcome: `Deny` → `403 Forbidden`, everything
/// else proceeds. Kept separate from the async middleware so it is unit-testable.
fn decision_outcome(decision: HookDecision) -> Result<(), AppError> {
    match decision {
        HookDecision::Deny(reason) => Err(AppError::Forbidden(reason)),
        HookDecision::Allow | HookDecision::Skip => Ok(()),
    }
}

/// Governance middleware — see the module docs for the request-chain position.
pub async fn governance_middleware(req: Request, next: Next) -> Result<Response, AppError> {
    // Full path (with the /api prefix and mount prefixes), independent of nesting.
    // `OriginalUri` is inserted by the outermost router before any prefix stripping;
    // fall back to the (possibly stripped) live URI only if it is missing.
    let path = req
        .extensions()
        .get::<OriginalUri>()
        .map(|o| o.0.path().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());
    let method = req.method().clone();

    let Some(operation) = classify(&method, &path) else {
        // Not a governed memory operation — let it through.
        return Ok(next.run(req).await);
    };

    // Enterprise hooks not initialized (e.g. non-PG / feature off) → fail open.
    let Some(hooks) = try_enterprise_hooks() else {
        return Ok(next.run(req).await);
    };

    // Tenant context is injected by auth_middleware. Its absence means governance is
    // running ahead of auth (misconfiguration) — fail open rather than 500.
    let Some(tenant_ctx) = req.extensions().get::<RequestTenantContext>().cloned() else {
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
        assert_eq!(classify(&Method::POST, "/api/v1/memory/forget"), None);
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
}
