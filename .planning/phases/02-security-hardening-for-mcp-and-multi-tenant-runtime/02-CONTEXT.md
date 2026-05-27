# Phase 2: Security Hardening for MCP and Multi-Tenant Runtime - Context

**Gathered:** 2026-03-28
**Status:** Ready for planning
**Mode:** Auto — all decisions at Claude's discretion

<domain>
## Phase Boundary

This phase reduces runtime and supply-chain risk for MCP integrations and tenant-isolation boundaries.

In scope:
- MCP component signing and provenance enforcement (SEC-01)
- Input validation and sanitization boundaries on high-risk paths (SEC-02)
- Multi-tenant execution isolation controls and monitoring hooks (SEC-03)

Out of scope:
- Evidence graph work (Phase 1)
- MemOS deep fusion (Phase 3)

</domain>

<decisions>
## Implementation Decisions

### MCP Trust Model
- **D-01:** MCP components must be signed; verify signature on load using a trusted key bundle
- **D-02:** Provenance checks use a SHA-256 hash of the component artifact + issuer metadata
- **D-03:** Unverified or unsigned components are rejected at load time with a structured error

### Input Validation Strategy
- **D-04:** All external input enters through a dedicated validation layer (middleware/hoop)
- **D-05:** Validation uses schema-based approach (serde + custom validators) at system boundaries
- **D-06:** Sanitization is context-aware: SQL uses parameterized queries, HTML uses a allowlist sanitizer, shell uses no direct invocation

### Multi-Tenant Isolation
- **D-07:** Tenant ID is injected at request entry and propagated via request extensions (Axum Extension)
- **D-08:** All data access goes through tenant-scoped repositories that prefix all queries with tenant_id
- **D-09:** Isolation failures produce metric increments + structured log events; no silent cross-tenant data leakage

### Auth Token Storage
- **D-10:** JWT moved from localStorage to httpOnly cookies with Secure and SameSite=Strict flags
- **D-11:** Route-level auth middleware on all protected routes; anonymous routes explicitly marked
- **D-12:** Passwords never logged; hashing uses bcrypt with cost factor 12

### Claude's Discretion
- Exact key bundle format and rotation strategy
- Specific validation library selection (validate.rs or custom)
- Middleware ordering and error response shape
- Metric cardinality and log sampling strategy

</decisions>

<canonical_refs>
## Canonical References

### Existing code touchpoints
- `backend/src/hoops/` — existing middleware/hoop pattern for auth
- `backend/src/routers/mcp.rs` — MCP protocol handlers
- `backend/src/routers/auth.rs` — existing auth flow
- `backend/src/db/` — repository layer for tenant-scoped queries

### Documentation touchpoints
- `docs/ARCHITECTURE.md` — baseline security architecture
- `.planning/codebase/CONCERNS.md` — known security concerns (JWT, cookies, auth mismatch)

### Requirements
- `REQUIREMENTS.md` §SEC-01, SEC-02, SEC-03

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `backend/src/hoops/` — existing enterprise middleware pattern
- `backend/src/db/` — repository pattern already established
- `backend/src/models/` — serde-based model validation

### Established Patterns
- Axum Extension for request-level state propagation
- Hoops middleware chain for cross-cutting concerns
- Repository pattern with db::pool()

### Integration Points
- MCP handlers in `backend/src/routers/mcp.rs`
- Auth middleware in `backend/src/hoops/enterprise.rs`
- All routers registered in `backend/src/main.rs`

</code_context>

<specifics>
## Specific Ideas

- Fix JWT storage as highest priority (known XSS vector)
- Route auth middleware as second priority (missing enforcement)
- MCP signing as third (supply-chain integrity)

</specifics>

<deferred>
## Deferred Ideas

- OAuth 2.0 + PKCE upgrade (future auth phase)
- WebAuthn/passkey support (future phase)
- Per-tenant resource quotas (Phase 3 or later)

</deferred>

---

*Phase: 02-security-hardening-for-mcp-and-multi-tenant-runtime*
*Context gathered: 2026-03-28 via autonomous mode — all decisions at Claude's discretion*
