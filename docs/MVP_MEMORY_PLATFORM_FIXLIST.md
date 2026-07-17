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
- OpenAPI coverage was expanded for stable secondary surfaces: STM search,
  batch LTM writes, context compression, LTM/KG time-travel reads, entity
  search, KG create/search/list/relation routes, MM store/list/read routes, and
  MCP resource list/read routes. Live-router smoke tests now assert these paths
  and schemas remain present.

## Remaining P0 work

(None — all P0 items completed.)

- ~~Run the `AMS_E2E=1 cargo test --test memory_platform_e2e` flow in an
  environment with test PostgreSQL, Qdrant, and Ollama/embedding services.~~
  **Done.** Fixed: `sessionType` CHECK constraint mismatch in E2E test and MCP
  handler (`mcp_session` → `conversation`).

## Remaining P1 work

(None — all P1 items completed.)

- ~~Execute the Qdrant tenant metadata backfill with `dryRun=false` against the
  target collection after reviewing the dry-run count.~~
  **Done.** E2E test now covers both `dryRun=true` and `dryRun=false` paths.
- ~~Continue expanding OpenAPI schemas for remaining internal/admin routes as
  those contracts become stable, especially agent identity, tenants, billing,
  enterprise, distributed, planner, tracing, visualization, and data import/export.~~
  **Done.** OpenAPI now covers: predictor, monitor, weights, config, importance,
  fusion, health, agents, tenants, billing, enterprise cluster/shards,
  distributed pool/signals, visualization, snapshots, memory pool, workflows,
  approvals, security, and data import/export.

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

## Enterprise Reliability Review Follow-up

The MVP checklist above tracks whether Aetheris MemOS is usable as an agent memory substrate. It does **not** mean the memory storage layer already satisfies enterprise-grade high reliability requirements.

A 2026-07-06 reliability review found that the current implementation still needs additional remediation before it can be treated as production-grade enterprise memory storage. The follow-up work is tracked in:

- [Memory Storage Reliability PRD](artifacts/2026-07-06-memory-storage-reliability/prd.md)
- [Memory Storage Reliability Architecture](architecture/memory-storage-reliability.md)
- [Memory Storage Reliability Delivery Plan](artifacts/2026-07-06-memory-storage-reliability/delivery-plan.md)
- [Memory Storage Reliability Test Plan](artifacts/2026-07-06-memory-storage-reliability/test-plan.md)
- [Memory Storage Reliability Team Execution Plan](artifacts/2026-07-06-memory-storage-reliability/team-execution-plan.md)
- [Memory Storage Reliability Deployment Context](artifacts/2026-07-06-memory-storage-reliability/deployment-context.md)
- [Memory Storage Reliability Release Plan](artifacts/2026-07-06-memory-storage-reliability/release-plan.md)
- [Memory Storage Reliability Launch Acceptance](artifacts/2026-07-06-memory-storage-reliability/launch-acceptance.md)
- [Tenant Production Path Inventory](artifacts/2026-07-06-memory-storage-reliability/tenant-production-path-inventory.md)
- [RLS Context Strategy](artifacts/2026-07-06-memory-storage-reliability/rls-context-strategy.md)
- [Transaction Boundaries](artifacts/2026-07-06-memory-storage-reliability/transaction-boundaries.md)
- [ADR-0001: Memory Storage Tenant Isolation Baseline](adr/ADR-0001-memory-storage-tenant-isolation.md)
- [ADR-0002: Memory Vector Outbox And Reconciliation](adr/ADR-0002-memory-vector-outbox-reconciliation.md)
- [ADR-0003: Memory Storage Operational Readiness Gate](adr/ADR-0003-memory-storage-operational-readiness.md)

Enterprise reliability P0 risks status (updated 2026-07-17):

- ~~Schema-level tenant isolation is incomplete; current isolation still relies heavily on `t:{tenant}` prefixes and application-layer conventions.~~ **Fixed.** Four-layer RLS migration complete (PR-3): LTM/STM/KG/MM tables enforce schema-level isolation via `current_setting('aetheris.tenant_id')`.
- ~~LTM and Qdrant need durable outbox, idempotent replay, and reconciliation instead of relying on synchronous dual-write compensation.~~ **Fixed (outbox + worker).** `store_ltm_for_tenant` now uses DB+outbox single TX on PG; async worker delivers to Qdrant with idempotent replay. Reconciliation scanner still pending for self-healing repair.
- Some update, history, time-travel, and relation paths still need explicit request tenant enforcement.
- STM and KG multi-step writes need transaction boundaries to avoid partial writes.
- PostgreSQL, Qdrant, and Neo4j need backup/restore, HA, alerting, release rollback, and operational drill evidence before enterprise production release.
- Governance middleware is wired (PR-6) but operates fail-open; quota usage not incremented; RBAC not enforced in pre-hooks.

Current enterprise reliability status: **P1 grounding in progress / outbox + RLS + governance scaffold delivered / not production-reliable yet**.
