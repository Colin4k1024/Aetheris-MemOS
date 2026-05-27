---
phase: 02-security-hardening-for-mcp-and-multi-tenant-runtime
plan: "01"
subsystem: auth
tags: [jwt, cookie, axum, middleware, tenant-isolation, httpOnly, SameSite]

# Dependency graph
requires:
  - phase: 01-evidence-graph-decision-snapshots-completeness
    provides: Backend API foundation, database models, Axum router structure
provides:
  - httpOnly+Secure+SameSite=Strict JWT cookie authentication
  - Explicit route-level auth middleware on all protected endpoints
  - TenantContext extractor for request-scoped tenant propagation
  - Rejection of tokens in query strings
affects:
  - phase: 02-02 (rate limiting and RBAC)
  - phase: 02-03 (audit logging)
  - phase: 02-04 (multi-tenant data isolation)

# Tech tracking
tech-stack:
  added: [axum::middleware, axum_extra::extract::CookieJar]
  patterns:
    - httpOnly cookie-based JWT (eliminates localStorage XSS vector)
    - Route-level middleware composition (public vs protected router)
    - Request-scoped tenant context via Axum FromRequestParts extractor

key-files:
  created:
    - backend/src/axum_routers/protected.rs (protected router with auth middleware)
    - backend/src/tenant/context.rs (RequestTenantContext with FromRequestParts)
  modified:
    - backend/src/axum_routers/auth.rs (SameSite=Strict, Secure flag, removed token-query login)
    - backend/src/axum_routers/mod.rs (uses protected_router, documents public vs protected)
    - backend/src/hoops/jwt.rs (cookie-first token extraction, TenantContext population)
    - backend/src/tenant/mod.rs (exports RequestTenantContext, backward-compat TenantId)

key-decisions:
  - "JWT stored in httpOnly+Secure+SameSite=Strict cookie (not localStorage) to eliminate XSS token theft"
  - "Authorization header supported as fallback for API clients (curl, Brave, etc.)"
  - "Query-string tokens explicitly rejected with 401 to prevent referrer leakage"
  - "Route protection applied via protected_router() composition rather than per-router layers"
  - "MVP: each user is their own tenant (tenant_id derived from JWT uid claim)"

patterns-established:
  - "Protected routes marked with // PROTECTED comment; public routes marked // PUBLIC"
  - "TenantContext extractor: async fn handler(tenant: RequestTenantContext) auto-injects tenant"

requirements-completed: [SEC-01, SEC-02, SEC-03]

# Metrics
duration: 14min
completed: 2026-03-28
---

# Phase 02 Plan 01 Summary

**JWT cookie auth with httpOnly+Secure+SameSite=Strict, explicit route middleware, and tenant context propagation**

## Performance

- **Duration:** 14 min
- **Started:** 2026-03-28T00:00:00Z
- **Completed:** 2026-03-28T00:13:29Z
- **Tasks:** 3
- **Files modified:** 8 (3 created, 5 modified)

## Accomplishments

- JWT tokens now stored in httpOnly cookies with Secure and SameSite=Strict flags (eliminates localStorage XSS attack surface)
- All protected routes explicitly wrapped with auth_middleware; public routes clearly documented
- Token query parameter login removed (post_login_with_token, get_login_with_token deleted)
- TenantContext extractor available for handlers to get tenant-scoped data
- Auth middleware rejects tokens appearing in query strings

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix JWT cookie security flags** - `db3672a` (fix)
2. **Task 2: Implement route-level auth middleware on protected routes** - `70d0281` (feat)
3. **Task 3: Add tenant context propagation via Axum Extension** - `afc5030` (feat)

**Plan metadata:** `1594c93` (docs: create security hardening phase plans)

## Files Created/Modified

- `backend/src/axum_routers/auth.rs` - SameSite=Strict, Secure flag added; token-query login removed
- `backend/src/axum_routers/mod.rs` - Uses protected_router(), documents public vs protected routes
- `backend/src/axum_routers/protected.rs` - NEW: Protected router with auth_middleware layer applied
- `backend/src/hoops/jwt.rs` - Cookie-first token extraction, TenantContext insertion, query-string rejection
- `backend/src/tenant/context.rs` - NEW: RequestTenantContext with FromRequestParts implementation
- `backend/src/tenant/mod.rs` - Exports RequestTenantContext, backward-compatible TenantId with no-arg new()

## Decisions Made

- Used httpOnly cookie as primary JWT vehicle (not localStorage) to prevent XSS token theft
- Authorization header kept as fallback for non-browser API clients (curl, Brave, etc.)
- Route-level protection via dedicated protected_router() rather than per-router layers (clearer separation)
- MVP tenant: each user is their own tenant (tenant_id = user_id from JWT uid claim)
- Backward compatibility maintained for legacy Salvo routers (TenantId::new() with no args still works)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `TenantId` type conflict: `services::multi_tenant::TenantId` and `tenant::TenantId` were separate types. Resolved by keeping local `TenantId` in `tenant/mod.rs` with full compatibility (both `new()` no-arg and `from_string()` variants).
- `#[axum::async_trait]` vs `#[async_trait::async_trait]`: Axum 0.8 uses `async_trait` crate internally. Implemented `FromRequestParts` without `#[async_trait]` attribute using native Rust async fn in trait support.
- `TenantId::new()` pre-existing bug in `routers/tenant.rs` (legacy Salvo router) called `TenantId::new()` without arguments. Resolved by adding no-arg `new()` to `tenant/mod.rs` for backward compatibility.

## Next Phase Readiness

- Auth foundation complete for phase 02 subsequent plans
- Route protection infrastructure in place for SEC-02 (RBAC/rate limiting)
- TenantContext extractor ready for SEC-03 (multi-tenant data isolation)
- No blockers for plan 02-02

---
*Phase: 02-security-hardening-for-mcp-and-multi-tenant-runtime*
*Completed: 2026-03-28*
