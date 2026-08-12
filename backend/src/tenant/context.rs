//! Tenant Context
//!
//! This module provides tenant context management.

use axum::{extract::FromRequestParts, http::request::Parts};
use serde::{Deserialize, Serialize};

use crate::tenant::quota::ResourceQuota;
use crate::tenant::TenantId;
use crate::AppError;

/// Tenant context containing tenant-specific information.
#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub quota: ResourceQuota,
    pub settings: TenantSettings,
    pub created_at: i64,
    pub updated_at: i64,
}

impl TenantContext {
    pub fn new(tenant_id: TenantId) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            tenant_id,
            quota: ResourceQuota::default(),
            settings: TenantSettings::default(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_quota(mut self, quota: ResourceQuota) -> Self {
        self.quota = quota;
        self
    }

    pub fn with_settings(mut self, settings: TenantSettings) -> Self {
        self.settings = settings;
        self
    }

    /// Check if a resource is within quota.
    pub fn check_quota(&self, resource: &QuotaResource) -> bool {
        self.quota.check(resource)
    }
}

/// Tenant-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantSettings {
    pub name: String,
    pub timezone: String,
    pub default_language: String,
    pub features: Vec<String>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for TenantSettings {
    fn default() -> Self {
        Self {
            name: String::new(),
            timezone: "UTC".to_string(),
            default_language: "en-US".to_string(),
            features: vec![],
            metadata: Default::default(),
        }
    }
}

/// Resource quota type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaResource {
    StorageMB,
    ApiCallsPerDay,
    ConcurrentSessions,
    MemoryEntries,
}

impl std::fmt::Display for QuotaResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuotaResource::StorageMB => write!(f, "storage_mb"),
            QuotaResource::ApiCallsPerDay => write!(f, "api_calls_per_day"),
            QuotaResource::ConcurrentSessions => write!(f, "concurrent_sessions"),
            QuotaResource::MemoryEntries => write!(f, "memory_entries"),
        }
    }
}

// ============ Request-scoped Tenant Context ============
//
// lightweight tenant context for request-scoped data isolation.
// populated by auth_middleware from JWT claims.
/// Request-scoped tenant context for data isolation.
///
/// This is a lightweight context extracted from the authenticated request.
/// It differs from `TenantContext` (above) which is the persistent tenant
/// configuration entity.
///
/// Usage in handlers:
/// ```
/// use axum::response::IntoResponse;
/// use backend::tenant::RequestTenantContext;
///
/// // `RequestTenantContext` is an axum extractor: request it by value and the
/// // auth middleware populates it from the JWT before the handler runs.
/// async fn handler(tenant: RequestTenantContext) -> impl IntoResponse {
///     // tenant.tenant_id scopes data access for isolation;
///     // tenant.user_id identifies the authenticated user.
///     format!("tenant={} user={}", tenant.tenant_id, tenant.user_id)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RequestTenantContext {
    /// Tenant identifier for data isolation.
    pub tenant_id: TenantId,
    /// Authenticated user ID from JWT.
    pub user_id: String,
}

impl RequestTenantContext {
    /// Construct from an authenticated JWT — the primary production path.
    ///
    /// `tenant_id` comes from the token's `org` claim (falling back to `uid` for
    /// tokens issued before the claim existed); `user_id` comes from `uid`. The
    /// two are now independent, which is what makes role differentiation possible.
    pub fn from_authenticated(tenant_id: impl Into<String>, user_id: impl Into<String>) -> Self {
        Self {
            tenant_id: TenantId::from_string(tenant_id),
            user_id: user_id.into(),
        }
    }

    /// Convenience for tests and the SQLite fallback: personal org where
    /// `tenant_id = user_id`. Equivalent to the old `new(user_id)`.
    pub fn new(user_id: impl Into<String>) -> Self {
        let user_id = user_id.into();
        Self {
            tenant_id: TenantId::from_string(&user_id),
            user_id,
        }
    }

    /// Authorize access to a resource whose tenant is named by a URL path parameter.
    ///
    /// Any handler that reads a `tenant_id` from the request path (e.g.
    /// `/billing/usage/{tenant_id}`, `/tenants/{tenant_id}/sessions`) MUST call this
    /// before touching that tenant's data. Returns `Ok(())` only when the path tenant
    /// matches the authenticated caller's tenant; otherwise it fails closed. The RBAC
    /// permission gate in `hoops::governance` only checks *whether the caller holds a
    /// permission*, not *whether the tenant is theirs* — so without this check any
    /// authenticated user can reach another tenant just by editing the path.
    ///
    /// On mismatch it returns [`AppError::Forbidden`] (HTTP 403) — deliberately **not**
    /// [`AppError::NotFound`] (404). A 404 would distinguish "this tenant exists but you
    /// may not see it" from "this tenant does not exist", turning the endpoint into a
    /// tenant-existence oracle an attacker could enumerate. 403 gives both cases the same
    /// answer, and the message intentionally carries neither tenant id for the same reason.
    pub fn authorize_path_tenant(&self, path_tenant: &str) -> Result<(), AppError> {
        if self.tenant_id.as_str() == path_tenant {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "Access to the requested tenant is forbidden".to_string(),
            ))
        }
    }
}

impl<S> FromRequestParts<S> for RequestTenantContext
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Read the context that `auth_middleware` already constructed and inserted.
        // This eliminates the second derivation site — previously this impl re-ran
        // `RequestTenantContext::new(&claims.uid)` which hard-wired tenant=uid and
        // silently bypassed the org claim the middleware had already resolved.
        parts
            .extensions
            .get::<RequestTenantContext>()
            .cloned()
            .ok_or_else(|| {
                AppError::Unauthorized("Authentication required for this endpoint".to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_allows_the_callers_own_tenant() {
        let ctx = RequestTenantContext::new("tenant-a");
        assert!(ctx.authorize_path_tenant("tenant-a").is_ok());
    }

    #[test]
    fn authorize_rejects_another_tenant_with_403() {
        let ctx = RequestTenantContext::new("tenant-a");
        let err = ctx
            .authorize_path_tenant("tenant-b")
            .expect_err("cross-tenant path access must be rejected");
        assert!(matches!(err, AppError::Forbidden(_)));

        use axum::response::IntoResponse;
        assert_eq!(
            err.into_response().status(),
            axum::http::StatusCode::FORBIDDEN,
            "mismatch must map to 403 Forbidden, not 404 (no tenant-existence oracle)"
        );
    }

    #[test]
    fn authorize_error_leaks_neither_tenant_id() {
        // 403 + an id-free message means the endpoint answers "exists but forbidden"
        // and "does not exist" identically — it cannot be used to enumerate tenants.
        let ctx = RequestTenantContext::new("caller-secret-tenant");
        let AppError::Forbidden(msg) = ctx
            .authorize_path_tenant("victim-tenant")
            .expect_err("must reject")
        else {
            panic!("expected Forbidden");
        };
        assert!(
            !msg.contains("caller-secret-tenant"),
            "message must not leak the caller's tenant id"
        );
        assert!(
            !msg.contains("victim-tenant"),
            "message must not leak the requested tenant id"
        );
    }

    /// Anti-drift guard — mirrors the mapping-guard style of `routers/mcp.rs`
    /// `all_known_tools_are_covered` and `hoops/governance.rs`
    /// `all_privileged_admin_surfaces_stay_permission_gated`.
    ///
    /// Scans **every** handler under `src/routers`: a handler that pulls a `tenant_id`
    /// out of the URL path MUST call `authorize_path_tenant`. Without this, the next
    /// person who adds a `/{tenant_id}/…` route (the "11th handler") silently re-opens
    /// the cross-tenant hole. Fixing the 11 known handlers is easy; keeping the 12th
    /// honest is the point of this test — it fails the build the moment a new path-tenant
    /// handler skips the check.
    #[test]
    fn every_path_tenant_handler_calls_authorize() {
        let mut scanned = 0usize;
        for h in router_handlers() {
            // Takes a tenant_id from the URL path iff the parameter list binds it via a
            // `Path(...)` extractor. A body/JSON tenant_id never appears in the signature.
            if !(h.signature.contains("Path") && h.signature.contains("tenant_id")) {
                continue;
            }
            scanned += 1;
            assert!(
                h.body.contains("authorize_path_tenant"),
                "{}: handler `{}` reads tenant_id from the URL path but never calls \
                 RequestTenantContext::authorize_path_tenant. Any authenticated user could then \
                 reach another tenant by editing the path. Add \
                 `tenant_ctx.authorize_path_tenant(&tenant_id)?;` (see tenant/context.rs), or — if \
                 the endpoint is genuinely cross-tenant — route it through an explicit admin \
                 authorization path instead of a bare path parameter.",
                h.file,
                h.name
            );
        }

        // Floor guards against the scanner silently matching nothing (e.g. a refactor that
        // renames `pub async fn` or the `Path`/`tenant_id` tokens). The four always-mounted
        // handlers (billing usage + quota, tenant search + sessions) are the stable minimum;
        // the dead-code handlers in routers/tenant.rs are additionally covered while they
        // exist but are deliberately not part of this floor, in case that file is removed.
        assert!(
            scanned >= 4,
            "expected the scan to find the mounted tenant_id-path handlers (>=4), found {scanned}; \
             did the scanner or the handler signatures change shape?"
        );
    }

    /// Body-side counterpart of the path guard (P0-7). A handler that reads a tenant
    /// identifier out of its request *body* (a `Json(..)` / `Query(..)` binding) is
    /// trusting client input exactly as much as a path parameter, so it MUST authorize it.
    ///
    /// Keyed on the actual **read** of the binding variable's tenant field, NOT on the
    /// struct definition. This is deliberate: memory_storage / multimodal / kg carry a
    /// `tenant_id` field on their request bodies but ignore it and scope by
    /// `tenant_ctx.tenant_id` (the safe pattern) — they never read `req.tenant_id`, so they
    /// are correctly NOT flagged. Only handlers that actually trust a client-supplied body
    /// tenant are required to authorize. Catches the 7th body-tenant handler the moment it
    /// appears, in any router file.
    #[test]
    fn every_body_tenant_handler_calls_authorize() {
        let mut scanned = 0usize;
        for h in router_handlers() {
            let Some(var) = client_body_tenant_binding(&h.signature, &h.body) else {
                continue;
            };
            scanned += 1;
            assert!(
                h.body.contains("authorize_path_tenant"),
                "{}: handler `{}` reads a tenant field out of its `{}` request body/query but \
                 never calls RequestTenantContext::authorize_path_tenant. Any authenticated user \
                 could then act on another tenant by changing that field. Compare it against the \
                 caller (`tenant_ctx.authorize_path_tenant(&{}.tenant_id)?;`), or scope the \
                 operation to `tenant_ctx.tenant_id` and stop reading the body tenant.",
                h.file,
                h.name,
                var,
                var
            );
        }

        // Floor: the five body-tenant handlers that must stay guarded (billing
        // get_usage / init_tenant / record_usage, multi_tenant register_tenant /
        // check_access). Catches the scanner silently matching nothing.
        assert!(
            scanned >= 5,
            "expected the scan to find the body-tenant handlers (>=5), found {scanned}; did the \
             scanner or the handler shapes change?"
        );
    }

    /// Regression guard for the one no-param enumerate endpoint (P0-7). `list_tenants`
    /// carries no path or body tenant, so neither signature-based scan above can cover it —
    /// yet returning the full tenant registry leaks every tenant id to any authenticated
    /// user. It must scope its result to the caller; if a refactor drops the `tenant_ctx`
    /// filter and re-widens it, this fails.
    #[test]
    fn list_tenants_is_scoped_to_the_caller() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/routers/multi_tenant_router.rs"),
        )
        .expect("read multi_tenant_router.rs");
        let handler = src
            .split("pub async fn ")
            .find(|c| c.starts_with("list_tenants"))
            .expect("list_tenants handler must exist");
        let body = &handler[handler.find('{').unwrap_or(0)..];
        assert!(
            body.contains("tenant_ctx.tenant_id"),
            "list_tenants must filter its result by the authenticated tenant_ctx.tenant_id; \
             returning the full tenant registry leaks every tenant id to any authenticated user"
        );
    }

    // ── shared scanning helpers ────────────────────────────────────────────────

    /// One `pub async fn` handler found under `src/routers`, split into its `signature`
    /// (parameter list + return type, before the body `{`) and its `body`.
    struct Handler {
        file: String,
        name: String,
        signature: String,
        body: String,
    }

    /// Read and split every `pub async fn` under `src/routers` into [`Handler`]s.
    ///
    /// Text-level scanning (not a real parser) is deliberate: dependency-free and robust to
    /// handlers moving between files. `CARGO_MANIFEST_DIR` makes it independent of the
    /// test's working directory. Private `async fn` (e.g. mod.rs helpers) are intentionally
    /// out of scope — only `pub async fn` are mountable as route handlers.
    fn router_handlers() -> Vec<Handler> {
        let routers_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/routers");
        let mut handlers = Vec::new();
        let entries = std::fs::read_dir(&routers_dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", routers_dir.display()));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            let file = path.file_name().unwrap().to_string_lossy().into_owned();
            for chunk in src.split("pub async fn ").skip(1) {
                let sig_end = chunk.find('{').unwrap_or(chunk.len());
                handlers.push(Handler {
                    file: file.clone(),
                    name: chunk.split('(').next().unwrap_or("").trim().to_string(),
                    signature: chunk[..sig_end].to_string(),
                    body: chunk[sig_end..].to_string(),
                });
            }
        }
        handlers
    }

    /// If the handler reads a tenant identifier out of one of its `Json(..)` / `Query(..)`
    /// bindings, returns that binding's variable name. Reads of `tenant_ctx.tenant_id` (the
    /// authenticated context) are not body reads, so the safe ctx-scoping pattern is not
    /// matched. Unused bindings (`_query`) are skipped — they cannot be read.
    fn client_body_tenant_binding(signature: &str, body: &str) -> Option<String> {
        for binder in ["Json(", "Query("] {
            let mut rest = signature;
            while let Some(pos) = rest.find(binder) {
                let after = &rest[pos + binder.len()..];
                let var: String = after
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                rest = &after[var.len()..];
                if var.is_empty() || var.starts_with('_') {
                    continue;
                }
                if body_reads_tenant_field(body, &var) {
                    return Some(var);
                }
            }
        }
        None
    }

    /// True when `body` reads `<var>.<field>` for some field whose name contains "tenant"
    /// (e.g. `req.tenant_id`, `req.requester_tenant`). Enforces an identifier boundary
    /// before `<var>` so a var of `req` does not match a longer identifier like `myreq.`.
    fn body_reads_tenant_field(body: &str, var: &str) -> bool {
        let needle = format!("{var}.");
        let bytes = body.as_bytes();
        let mut from = 0;
        while let Some(rel) = body[from..].find(&needle) {
            let start = from + rel;
            let boundary_ok = start == 0
                || !matches!(bytes[start - 1], b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_');
            if boundary_ok {
                let field: String = body[start + needle.len()..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if field.to_ascii_lowercase().contains("tenant") {
                    return true;
                }
            }
            from = start + needle.len();
        }
        false
    }
}
