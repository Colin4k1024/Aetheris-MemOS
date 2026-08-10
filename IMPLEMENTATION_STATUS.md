# Aetheris MemOS — Implementation Status

> Last updated: 2026-08-10 | Phase: Post-P1 grounding (governance + outbox + RLS)
> **Verified source of truth**: [docs/TEAM_TECH_WALKTHROUGH.md](docs/TEAM_TECH_WALKTHROUGH.md) — all statuses below are cross-referenced against source-code verification on branch `dev` @ `a9a3e5a`, default `cargo build` (`default = []`). If this file conflicts with the walkthrough, the walkthrough is authoritative.

**Status: IN PROGRESS.** The system is a functional memory platform with tenant isolation and governance
scaffolding, but **not** enterprise-production-ready. The previous "COMPLETED v1.0" claim was incorrect
and has been replaced with this per-capability truth table.

---

## Capability Status

### Core Memory Kernel

| Capability | Status | Notes |
|---|---|---|
| STM (short-term memory) | ✅ Operational | Session-scoped, per-tenant, PostgreSQL/SQLite |
| LTM (long-term memory) | ✅ Operational | Bi-temporal, vector-indexed via Qdrant; PG path uses outbox for consistency |
| KG (knowledge graph) | ✅ Operational | Entity/relation CRUD, tenant-scoped, bi-temporal; Neo4j connects but is unused by any router/service |
| MM (multimodal memory) | ✅ Operational | Cross-modal embeddings, tenant-scoped; ⚠️ `MMRepository::count` is global without tenant filter |
| Triple hybrid search | ✅ Operational | Vector + keyword + graph neighborhood; fusion is linear weighted sum (not RRF); keyword is `LIKE '%…%'` (not FTS) |
| Multi-dimensional confidence scoring | ✅ Operational | Quality, relevance, recency, access, completeness |
| Context compression | ✅ Operational | Sliding window, importance prune, LLM summary, hierarchical; real LLM calls with truncation fallback |
| Background STM→LTM transfer | ✅ Operational | Tenant-scoped daemon cycles; triggered by message count/duration/ratio |

### Enterprise Reliability

| Capability | Status | Notes |
|---|---|---|
| Schema-level RLS (8 tables) | ⚠️ Implemented (inert) | LTM/STM/KG/MM tables; migration expand→backfill→enforce→contract is correct. **RLS never triggers** because the app connects as Postgres superuser (`memory` role = table owner). The restricted role `aetheris_app` (NOSUPERUSER NOBYPASSRLS) exists in `docker/initdb/` but no config points `DATABASE_URL` at it. |
| tenant_scope executor | ✅ Implemented | Per-request `SET LOCAL aetheris.tenant_id` via GUC; `is_local=true` prevents connection-pool leakage |
| Vector outbox (PG) | ✅ Implemented | DB fact+outbox single TX; async worker delivers to Qdrant; `indexStatus: pending`; Qdrant not on write critical path |
| Outbox worker (idempotent replay) | ✅ Implemented | `FOR UPDATE SKIP LOCKED` claim, exponential backoff, dead-letter, stale reclaim; ⚠️ zero behavioral tests for retry/idempotency |
| SQLite legacy dual-write | ✅ Retained | Dev mode synchronous Qdrant→DB path; not for production |
| Vector reconciliation scanner | 🟠 Implemented (zero callers) | `services/vector_reconciliation.rs` has complete implementation (missing/orphan/tenant_mismatch/content_hash_mismatch), but no daemon, no route, no caller |
| DB backup/PITR/HA/alerting | ❌ Planned | ADR-0003; requires ops environment |
| Release rollback drill | ❌ Planned | ADR-0003 |

### Security & Governance

| Capability | Status | Notes |
|---|---|---|
| JWT auth | ✅ Operational | Weak-key fail-fast, login rate limiting |
| Rate limiting | ✅ Operational | ⚠️ XFF-based without `ConnectInfo` injection → client-controlled header; login brute-force protection shares this gap |
| Governance middleware | ✅ Wired | `auth → rate_limit → governance → handler`; classify+pre-hook chain; ⚠️ `jwt.disabled=true` shorts governance entirely (RBAC/quota/audit all skipped) |
| Audit event store | ✅ Implemented | Async mpsc writer → batch INSERT to `memory_audit_events`; ⚠️ fire-and-forget with silent drops, no retry/backpressure; only started in PG mode (SQLite loses all audit events) |
| RBAC (role-based access) | 🔴 Non-functional | Hooks exist and `pre_store` checks `"write"` permission, but **roles are never seeded at startup** and the only `assign_role` handler is in the **unmounted** `routers/tenant.rs:202` → `blocking_has_permission` is always `false`. Governance enabled → all store/search return 403. |
| Quota enforcement | 🔴 Never triggers | `QuotaManager` exists but `used` is never incremented: the only increment site is `enterprise_impl.rs:540` `post_store`, but governance middleware only calls pre-hooks, never post-hooks. Unknown tenants default to `allowed: true`. |
| Governance fail mode | ⚠️ Fail-open | Uninitialized hooks / missing tenant → allow; `GOVERNANCE_FAIL_CLOSED` env var exists but defaults to fail-open; `jwt.disabled` shorts before fail-closed check |
| MCP tool signing | ✅ Implemented | HMAC-SHA256 + constant-time comparison, enforced before `call_tool` dispatch; ⚠️ default config has no keys → `list_tools` returns empty, `call_tool` returns 403 for everything (correct fail-closed, but looks broken to integrators) |
| MCP capability authorization | ⚠️ Partial (hardcoded) | `capability::authorize` is wired into the request path, but `granted` is hardcoded to `[Read, Write, Delete]` for all callers — no per-principal least-privilege |
| MCP wasm sandbox (Plane B) | 🟠 Real wasmtime, not wired | `mcp/sandbox.rs` is a real wasmtime implementation with fuel limiting and capability policies, but has **zero references** — `call_tool` executes tools natively, never enters the sandbox |

### Adaptive Intelligence

| Capability | Status | Notes |
|---|---|---|
| Adaptive scheduler | ⚠️ Static heuristic | Fixed coefficients (`ltm=0.8, kg=0.7, mm=0.6`); resource constraint → uniform `*= 0.85`; no learning from telemetry |
| Predictor | ⚠️ Hardcoded | `services/predictor.rs` uses fixed baseline constants; `TrainablePredictor::fit_from_samples` declared but **never implemented**; `confidence_score` hardcoded to `None` |
| Weight adjuster | ⚠️ Static heuristic | Pluggable strategies without online fitting; `performance_impact` is stored but **never read back** |
| Planner sandbox | 🔴 Mock | `execute_step` never executes tools — returns `generate_mock_response` hardcoded JSON; "isolation" is a bool flag, not OS/WASM-level |
| Learning闭环 | 🔴 Disconnected | `adaptive_telemetry` → `feature_pipeline` → `eval_harness` → `strategy_mutator` are four components that are **not connected to each other** and none feed back into scheduler/predictor. `strategy_mutator` daemon is **commented out** in `main.rs` with an honest note that its output is consumed by nothing. `eval_harness::run` returns hardcoded `passed: true, coherence: 1.0`. |

### Multi-Protocol

| Capability | Status | Notes |
|---|---|---|
| HTTP/REST | ✅ Operational | Primary protocol; Axum + JWT; 157 registered routes |
| MCP (Model Context Protocol) | ⚠️ Partial | `list_tools`/`list_resources` functional; signing enforced (fail-closed by default); capability grants are hardcoded; wasm sandbox not wired; MCP write path (`memory_write`/`memory_forget`) bypasses governance entirely (classify returns `None`) |
| gRPC | 🔴 Not implemented (unreachable stub) | `grpc_auth_interceptor` has **zero callers**; no tonic server, no `.proto` files, no `build.rs`; `tonic` dependency is only used for OTLP export |
| WebSocket | 🔴 Not implemented (unreachable stub) | `ws_upgrade_handler` is **never mounted** on any route; inner handler is a TODO that sends Close 1008 on connect |
| A2A (Agent-to-Agent) | 🔴 Not shipping (feature off) | Gated behind `#[cfg(feature = "a2a")]` with `default = []`; handler code is real but **does not compile** in default build; git dependency requires network at build time |

### SDKs & Integration

| Capability | Status | Notes |
|---|---|---|
| Python SDK | ✅ Operational | `remember`, `recall`, `search`, `forget`, `explain`, `feedback`; ⚠️ `api/mcp/initialize` path bug (backend is `/api/initialize`) |
| Python LangChain SDK | ✅ Operational | `AdaptiveMemoryTool`, `AdaptiveMemoryRetriever`, `ChatMessageHistory`; ⚠️ `pyproject.toml` `build-backend` is wrong → `pip install` fails |
| Rust SDK | 🔴 Broken | `lib.rs` only compiles `client` + `models`; `memory.rs`/`knowledge_graph.rs`/`mcp.rs`/`agent.rs` reference non-existent `crate::{Config,Result,Error}` — orphan files. README documents the dead API; examples won't compile. |

### Observability

| Capability | Status | Notes |
|---|---|---|
| OpenTelemetry | ✅ Emitting | Traces exported |
| Prometheus metrics | ✅ Emitting | Outbox pending/dead-letter, quota ratios, request latency |
| Outbox lag / dead-letter metrics | ❌ Not implemented | Needed for operational visibility |
| Quota deny / MCP authz deny metrics | ❌ Not implemented | Needed for governance monitoring |
| Grafana dashboards | ❌ None | Only datasource configured; no alert rules anywhere in `monitoring/` |

### Documentation

| Capability | Status | Notes |
|---|---|---|
| API docs (Scalar) | ✅ Operational | OpenAPI at `/scalar`; ⚠️ hand-written, covers MVP subset only (no agents/workflows/distributed/planner/tenants/security/billing/snapshot/memory-pool/tracing/data-io) |
| OpenAPI schema coverage | ⚠️ Partial | Core memory, KG, MM, MCP, admin routes; many domains missing |
| IMPLEMENTATION_STATUS.md | ✅ Honest | This file — cross-referenced against TEAM_TECH_WALKTHROUGH.md |
| AGENTS.md | ⚠️ Needs update | Missing governance/audit/outbox modules. (Its 5 endpoint rows were re-verified 2026-08-10 and are all real — an earlier revision of this file wrongly claimed AGENTS.md listed nonexistent endpoints.) |
| Agent-card capability claims | ⚠️ Needs audit | Some claims may exceed actual implementation |

---

## Known Gaps (Priority-Ordered)

### P0 — Enterprise Red Lines (blocking production)

1. **D-01** — RLS is inert: all configs connect as Postgres superuser, bypassing RLS entirely. Fix: point `DATABASE_URL` at `aetheris_app` role (already exists), then handle the two admin cross-tenant scan paths that RLS will block.
2. **D-02** — MCP capability grants are hardcoded to full permissions for all callers; wasm sandbox is real code but not wired into `call_tool`
3. **D-03** — Governance is fail-open; quota never triggers (`used` never incremented); RBAC is non-functional (roles can't be assigned); `jwt.disabled` shorts governance entirely
4. **D-04** — Rate limit key uses client-controlled `X-Forwarded-For` (no `ConnectInfo` injection); login brute-force protection shares this gap; `POST /register` has no rate limit at all
5. **D-05** — MCP write path (`memory_write`/`memory_forget`) bypasses governance entirely (classify returns `None` for MCP routes)
6. **D-06** — `db/mm.rs` tenant isolation is fail-open: `begin_optional_tenant_tx` skips tenant scope when `tenant_id` is `None` (11 call sites); `MMRepository::count` is global without tenant filter

### P1 — Reliability & Governance Completion

7. **D-07** — Vector reconciliation scanner is implemented but has zero callers — add a daemon or ops endpoint
8. **D-08** — Outbox retry/idempotency has zero behavioral tests (only key-format unit tests); `ON CONFLICT DO NOTHING`, `claim_batch`, backoff, `reclaim_stale` are untested
9. **D-09** — Audit event store is fire-and-forget with silent drops and no retry; SQLite mode loses all audit events
10. **D-10** — Liveness/readiness probes don't exist (despite CLAUDE.md and README claiming they do); `/v1/health` returns `self_healing` fake data (hardcoded `healthy: true`, 1–4ms fake latency)
11. **D-11** — No alert rules; Grafana has no dashboards; `monitoring/` directory is empty
12. **D-12** — CI only runs Postgres — outbox→Qdrant, KG, Redis-STM, embedding/LLM are all untested in CI; `clippy` and `cargo audit` use `continue-on-error: true` (non-blocking); 6 integration tests are "fake green" (return ok when env vars not set)

### P2 — Protocol & Integration

13. **D-13** — gRPC/WebSocket: unreachable stubs with no server — either implement or remove and sync all docs
14. **D-14** — A2A: real code behind a feature gate that is off by default — decide whether to ship or remove
15. **D-15** — Frontend `auth.ts` has 4 wrong API paths (login/me/logout/captcha); hidden by Umi dev mock
16. **D-16** — Python SDK `api/mcp/initialize` → should be `/api/initialize`; LangChain SDK `build-backend` is wrong; Rust SDK is broken (orphan modules, dead API in README)
17. **D-17** — SQLite mode has no DB-level tenant isolation; production config should explicitly forbid silent downgrade to SQLite

### P3 — Adaptive Intelligence

18. **D-18** — Predictor `fit_from_samples` is declared but never implemented; all adaptive components are static heuristics
19. **D-19** — Learning loop is fully disconnected: telemetry, feature pipeline, eval harness, strategy mutator are not connected to each other and none feed scheduler/predictor
20. **D-20** — Planner sandbox is mock (hardcoded JSON responses, no real tool execution, no OS-level isolation)

---

## Current Phase: Post-P1 Grounding

The P1 governance/outbox/RLS scaffolding is merged (PR-1 through PR-6), but all three are in a *scaffolded-but-not-effective* state:

- **RLS**: migrations exist, but superuser connection makes them no-ops
- **Outbox**: code is solid, but zero behavioral tests
- **Governance**: middleware is wired, but RBAC/quota are non-functional in practice

Completed PRs:

| PR | Scope | Status |
|---|---|---|
| PR-1 | tenant_scope executor (RLS keystone) | ✅ Merged |
| PR-2 | Audit event store + async writer | ✅ Merged |
| PR-3 | RLS migration + wiring (8 tables across 4 memory layers) | ✅ Merged |
| PR-4 | Outbox write path (LTM DB+outbox single TX) | ✅ Merged |
| PR-5 | Outbox worker + idempotent replay | ✅ Merged |
| PR-6 | Governance middleware (RBAC/quota/audit hooks) | ✅ Merged |
| PR-7 | MCP Plane A (signing + capability) | ⬜ Pending |
| PR-8 | MCP Plane B (wasm sandbox) | ⬜ Pending |
| PR-9 | Enterprise license tier gating | ⬜ Pending |

For the full defect list and fix plan, see [implementation defect remediation](docs/artifacts/2026-07-17-implementation-defect-remediation/fix-plan.md).

---

## Build & Test

- **Backend**: `cargo build` / `cargo check` passes
- **Unit tests**: `cargo test` passes (286 passed / 3 failed when no DB; 3 failures are `tenant_scope.rs:71` hard panics, not graceful skips)
- **E2E tests**: `AMS_E2E=1 cargo test --test memory_platform_e2e` (requires PG + Qdrant + Ollama); ⚠️ 6 integration tests are "fake green" — return ok when env vars not set
- **Lint**: `cargo clippy` / `cargo fmt` enforced

---

_This file is a living document. Update after each PR merge and wave completion. When in conflict with [docs/TEAM_TECH_WALKTHROUGH.md](docs/TEAM_TECH_WALKTHROUGH.md), the walkthrough is authoritative._