---
phase: "02"
plan: "02"
subsystem: mcp
tags: [SEC-01, mcp, signing, security, supply-chain]
dependency_graph:
  requires:
    - "02-01"
  provides:
    - "MCP component signing verification"
  affects:
    - "routers/mcp.rs"
    - "mcp/signing.rs"
tech_stack:
  added:
    - "sha2 (SHA-256)"
    - "hmac (HMAC-SHA256)"
    - "hex (hex encoding)"
  patterns:
    - "HMAC-SHA256 signature verification"
    - "Trusted key bundle from environment"
    - "Component registry with signature validation"
key_files:
  created:
    - "backend/src/mcp/mod.rs"
    - "backend/src/mcp/signing.rs"
    - "backend/tests/mcp_signing.rs"
  modified:
    - "backend/src/lib.rs"
    - "backend/src/main.rs"
    - "backend/src/routers/mcp.rs"
    - "backend/Cargo.toml"
decisions:
  - id: "D-01"
    text: "MCP components must be signed; verify signature on load using a trusted key bundle"
  - id: "D-02"
    text: "Provenance checks use SHA-256 hash of component artifact + issuer metadata"
  - id: "D-03"
    text: "Unverified or unsigned components are rejected at load time with a structured error"
metrics:
  duration: "~8 min"
  completed: "2026-03-28T13:37:00Z"
  tasks_completed: 3
  files_created: 3
  files_modified: 4
  tests_added: 7
---

# Phase 02 Plan 02: MCP Component Signing Verification

**One-liner:** HMAC-SHA256 component signing with trusted key bundle from environment variables

## Summary

Implemented SEC-01: MCP component signing and provenance verification to ensure supply-chain integrity for MCP integrations.

### What Was Built

1. **MCP Signing Module** (`backend/src/mcp/signing.rs`):
   - `ComponentSignature` struct with SHA-256 hash, issuer, version, timestamp, and signature fields
   - `verify_component()` function for HMAC-SHA256 integrity verification
   - `TrustedKeyBundle` loading trusted keys from `MCP_TRUSTED_ISSUERS` and `MCP_KEY_*` environment variables
   - `SigningError` enum with structured error types: `Unsigned`, `VerificationFailed`, `MissingKeyBundle`, `UnknownIssuer`

2. **MCP Router Integration** (`backend/src/routers/mcp.rs`):
   - `McpComponentRegistry` holds verified tools and resources
   - `list_tools()` and `list_resources()` verify signatures before exposure
   - `MCP_TOOL_SIGNATURES` and `MCP_RESOURCE_SIGNATURES` env vars for signature configuration
   - Unsigned/invalid components are logged and rejected (D-03)

3. **Integration Tests** (`backend/tests/mcp_signing.rs`):
   - 7 test cases covering valid signatures, tampered artifacts, unknown issuers, missing signatures, wrong keys

### Commits

| Task | Commit | Description |
| ---- | ------ |-------------|
| 1 | `b9c0d52` | feat(02-02): add MCP component signing with HMAC-SHA256 verification |
| 2 | `40c43f7` | feat(02-02): integrate signing verification into MCP router |
| 3 | `904b2a1` | test(02-02): add MCP signing integration tests |

## Success Criteria

| Criteria | Status |
|----------|--------|
| ComponentSignature struct with SHA-256 hash, issuer, version, timestamp, signature | PASS |
| verify_component() rejects unsigned with SigningError::Unsigned | PASS |
| verify_component() rejects tampered artifacts with SigningError::VerificationFailed | PASS |
| MCP router only exposes tools/resources with valid signatures | PASS |
| Signing failures logged with component details | PASS |
| All 4 (lib) + 7 (integration) signing tests pass | PASS |

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None.
