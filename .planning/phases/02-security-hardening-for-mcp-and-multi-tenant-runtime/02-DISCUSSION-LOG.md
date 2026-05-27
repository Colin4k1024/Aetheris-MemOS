# Phase 2: Security Hardening for MCP and Multi-Tenant Runtime - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-28
**Phase:** 2-security-hardening-for-mcp-and-multi-tenant-runtime
**Areas discussed:** MCP Trust Model, Input Validation Strategy, Multi-Tenant Isolation, Auth Token Storage
**Mode:** Autonomous (all decisions at Claude's discretion)

---

## MCP Trust Model

| Option | Description | Selected |
|--------|-------------|----------|
| Signed components + SHA-256 hash verification | Verify signature on load using trusted key bundle | ✓ |
| Trust-on-first-use (TOFU) | Accept on first load, warn on subsequent changes | |
| No signing, network-level trust | Rely on network isolation only | |

**Claude's choice:** Signed components + SHA-256 hash verification
**Rationale:** Supply-chain integrity requires cryptographic verification; TOFU is insufficient for production; network-only trust is too weak.

---

## Input Validation Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Schema-based validation at system boundaries | serde + custom validators at entry points | ✓ |
| Inline validation per handler | Each handler validates its own input | |
| Type-system only | Rely on Rust's type safety only | |

**Claude's choice:** Schema-based validation at system boundaries
**Rationale:** Consistent enforcement, single place to audit, reduces handler boilerplate.

---

## Multi-Tenant Isolation

| Option | Description | Selected |
|--------|-------------|----------|
| Tenant-scoped repositories | All queries prefixed with tenant_id | ✓ |
| Separate database per tenant | Complete physical isolation | |
| Row-level security (Postgres RLS) | Database-enforced tenant boundaries | |

**Claude's choice:** Tenant-scoped repositories
**Rationale:** Simpler operational overhead than per-tenant databases, works with existing schema; Postgres RLS can be layered on later.

---

## Auth Token Storage

| Option | Description | Selected |
|--------|-------------|----------|
| httpOnly cookies + Secure + SameSite=Strict | Browser-safe, XSS-resistant | ✓ |
| localStorage (current) | Simpler but XSS-exposed | |
| SessionStorage | Tab-scoped, still XSS-exposed | |

**Claude's choice:** httpOnly cookies + Secure + SameSite=Strict
**Rationale:** LocalStorage is vulnerable to XSS; cookies with httpOnly+Secure+SameSite prevent XSS token theft.

---

## Claude's Discretion

All remaining technical decisions (key format, validation library, middleware ordering, metric cardinality) delegated to implementation discretion.

## Deferred Ideas

- OAuth 2.0 + PKCE upgrade
- WebAuthn/passkey support
- Per-tenant resource quotas
