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

## Remaining P0 work

- Replace or remove the dead simplified `backend/src/axum_routers/*` memory,
  storage, search, KG, MM, and user handlers to prevent future regressions.
- Feed persisted retrieval feedback into ranking/importance scoring.
- Add integration tests that boot the live router and verify STM write/read,
  LTM write/read/search, hybrid search, adaptive trace, and MCP memory tools.

## Remaining P1 work

- Finish tenant scheduling/enumeration for background jobs and Qdrant metadata
  filters.
- Replace placeholder GraphRAG query embedding with the configured embedding
  service.
- Standardize search responses with `memoryId`, `sourceLayer`, `score`,
  `traceId`, `explanation`, and `metadata`.
- Update OpenAPI output so it reflects only stable MVP routes.

## Acceptance commands

When Rust tooling is available:

```bash
cd backend
cargo check
cargo test
```

For the SDK demo after starting the backend:

```bash
PYTHONPATH=sdks/python python examples/agent_memory_demo.py
```
