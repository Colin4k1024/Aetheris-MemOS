# Aetheris MemOS MVP Memory Platform Fixlist

This checklist tracks the MVP work needed to make Aetheris MemOS usable as a
memory substrate for external agents.

## Implemented in this pass

- Live backend entry now uses the fuller production router via
  `axum_routers::create_router() -> routers::root()`, so the running service no
  longer lands on the simplified `axum_routers/*` memory stubs.
- `MemoryAgent` exposes session history and unscoped adapter recall helpers.
- OpenAI, Anthropic, and LangChain runtime adapters now read real session
  history and perform real memory recall through `MemoryAgent`.
- Python SDK exposes the high-level agent memory contract:
  `remember`, `recall`, `search`, `forget`, `explain`, and `feedback`.
- Server-side REST endpoints now exist for `feedback`, `forget`, and `explain`
  under `/api/v1/memory/*`.
- MCP `memory_forget` now calls the same forget contract as REST instead of
  returning a pure acknowledgement.
- STM writes, LTM writes, STM reads, LTM reads, and REST hybrid search now use
  request-scoped tenant context instead of the default tenant.
- REST triple-hybrid/scored search and KG entity/list/relation lookup paths now
  use request-scoped tenant context.
- MM REST handlers now use request-scoped tenant context instead of accepting
  tenant scope from request body/query parameters.
- KG repository entity-expansion search now has tenant-scoped variants, with
  default-tenant wrappers retained only for compatibility.
- Retrieval feedback is persisted through the new `memory_feedback` table and
  exposed through the REST/SDK feedback contract.
- Background STM-to-LTM transfer and reflection cycles now have tenant-scoped
  service entry points; default-tenant wrappers remain for compatibility.
- New STM sessions are persisted with the tenant prefix that existing isolation
  checks expect.
- Added `examples/agent_memory_demo.py` for a minimal
  `remember -> recall/search -> feedback` flow.
- Added a live-router smoke test that verifies MVP memory/KG/MM paths are
  mounted on the real startup router and protected by auth.
- Removed the dead simplified `backend/src/axum_routers/*` memory, storage,
  search, KG, MM, user, and protected routers from the compatibility entry.
- Retrieval feedback now adjusts LTM, hybrid, triple-hybrid, and entity search
  scores before thresholding, with the adjustment recorded in result metadata.
- Search results now expose stable agent-facing fields such as `memoryId`,
  `sourceLayer`, `score`, `traceId`, `explanation`, and `metadata`.
- New LTM vector writes include `tenantId` Qdrant metadata and vector search
  applies the matching tenant filter.
- GraphRAG query embeddings now use the configured embedding service in
  production instead of a hardcoded zero vector.
- `/api-doc/openapi.json` now lists the stable MVP memory, KG, MM, and MCP
  routes instead of returning an empty path map.
- Live-router smoke coverage now includes adaptive, STM/LTM, hybrid,
  triple/scored search, KG, MM, MCP, and OpenAPI route registration.
- Workflow evidence remains available on the real router at
  `/api/v1/workflows/{id}/evidence`, with API tests covering success, not found,
  and OpenAPI path registration.
- Background STM-to-LTM transfer and reflection daemons now enumerate the
  default tenant plus registered tenants each cycle instead of only scanning the
  default tenant.
- Added a protected Qdrant tenant metadata backfill endpoint:
  `POST /api/v1/memory/storage/qdrant/backfill-tenant-metadata`, with `dryRun`
  enabled by default.
- MCP memory tool handlers now use the authenticated request tenant context for
  STM/LTM/KG/MM operations.
- Added `backend/tests/memory_platform_e2e.rs`, an environment-backed E2E flow
  gated by `AMS_E2E=1`, covering authenticated STM write/read, LTM write,
  hybrid search, adaptive trace, MCP memory write, and Qdrant backfill dry-run.
- OpenAPI now includes request/response schemas for the core stable
  agent-facing memory and MCP endpoints, and live-router tests assert schema
  coverage.

## Remaining P0 work

- Run the `AMS_E2E=1 cargo test --test memory_platform_e2e` flow in an
  environment with test PostgreSQL, Qdrant, and Ollama/embedding services.

## Remaining P1 work

- Execute the Qdrant tenant metadata backfill with `dryRun=false` against the
  target collection after reviewing the dry-run count.
- Continue expanding OpenAPI schemas for non-MVP/internal routes as those
  contracts become stable.

## Acceptance commands

When Rust tooling is available:

```bash
cd backend
cargo check
cargo test
```

For environment-backed E2E:

```bash
cd backend
AMS_E2E=1 cargo test --test memory_platform_e2e
```

For the SDK demo after starting the backend:

```bash
PYTHONPATH=sdks/python python examples/agent_memory_demo.py
```
