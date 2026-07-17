# Aetheris MemOS — Implementation Status

> Last updated: 2026-07-17 | Phase: P1 grounding (governance + outbox + RLS)
> Branch: `feat/p1-governance-middleware` (PR-1 through PR-6 merged)

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
| KG (knowledge graph) | ✅ Operational | Entity/relation CRUD, tenant-scoped, optional Neo4j |
| MM (multimodal memory) | ✅ Operational | Cross-modal embeddings, tenant-scoped |
| Triple hybrid search | ✅ Operational | Vector + keyword + graph neighborhood fusion |
| Multi-dimensional confidence scoring | ✅ Operational | Quality, relevance, recency, access, completeness |
| Context compression | ✅ Operational | Sliding window, importance prune, LLM summary, hierarchical |
| Background STM→LTM transfer | ✅ Operational | Tenant-scoped daemon cycles |

### Enterprise Reliability

| Capability | Status | Notes |
|---|---|---|
| Schema-level RLS (4 memory layers) | ✅ Implemented | LTM/STM/KG/MM tables; migration expand→backfill→enforce→contract |
| tenant_scope executor | ✅ Implemented | Per-request `SET LOCAL aetheris.tenant_id` via GUC |
| Vector outbox (PG) | ✅ Implemented | DB fact+outbox single TX; async worker delivers to Qdrant; `indexStatus: pending` |
| Outbox worker (idempotent replay) | ✅ Implemented | `FOR UPDATE SKIP LOCKED` claim, exponential backoff, dead-letter, stale reclaim |
| SQLite legacy dual-write | ✅ Retained | Dev mode synchronous Qdrant→DB path; not for production |
| Vector reconciliation scanner | ❌ Not implemented | Missing/orphan/tenant_mismatch/content_hash_mismatch repair |
| DB backup/PITR/HA/alerting | ❌ Planned | ADR-0003; requires ops environment |
| Release rollback drill | ❌ Planned | ADR-0003 |

### Security & Governance

| Capability | Status | Notes |
|---|---|---|
| JWT auth | ✅ Operational | Weak-key fail-fast, login rate limiting |
| Rate limiting | ✅ Operational | Per-IP; XFF-based (needs hardening — D-06) |
| Governance middleware | ✅ Wired | `auth → rate_limit → governance → handler`; classify+pre-hook chain |
| Audit event store | ✅ Implemented | Async mpsc writer → batch INSERT to `memory_audit_events` |
| RBAC (role-based access) | ⚠️ Scaffold | Hooks exist; `pre_store`/`pre_search` only check quota, not role permissions |
| Quota enforcement | ⚠️ Scaffold | `QuotaManager` exists; `used` not incremented → quota never blocks |
| Governance fail mode | ⚠️ Fail-open | Uninitialized hooks / missing tenant → allow; needs configurable fail-closed |
| MCP tool signing | ❌ Not enforced | `call_tool` skips signature verification (D-02) |
| MCP capability authorization | ❌ Not wired | `capability::authorize` has pure logic + unit tests but not in request path |
| MCP wasm sandbox (Plane B) | ❌ Mock | `execute_wasm` returns input unchanged; no isolation |

### Adaptive Intelligence

| Capability | Status | Notes |
|---|---|---|
| Adaptive scheduler | ⚠️ Heuristic | Fixed coefficients; no learning from telemetry |
| Predictor | ⚠️ Hardcoded | `services/predictor.rs` uses fixed baseline constants |
| Weight adjuster | ⚠️ Heuristic | Pluggable strategies without online fitting |
| Planner sandbox | ⚠️ Mock | Dry-run demonstration; not real isolation |

### Multi-Protocol

| Capability | Status | Notes |
|---|---|---|
| HTTP/REST | ✅ Operational | Primary protocol; Axum + JWT |
| MCP (Model Context Protocol) | ⚠️ Partial | `list_tools`/`list_resources` functional; `call_tool` lacks signing/capability |
| gRPC | ❌ Type shell | `protocol/grpc.rs` — no tonic server wiring |
| WebSocket | ❌ Type shell | `protocol/websocket.rs` — no real server |
| A2A (Agent-to-Agent) | ❌ Stub | Feature-gated; fake/simplified responses |

### SDKs & Integration

| Capability | Status | Notes |
|---|---|---|
| Python SDK | ✅ Operational | `remember`, `recall`, `search`, `forget`, `explain`, `feedback` |
| Python LangChain SDK | ✅ Operational | `AdaptiveMemoryTool`, `AdaptiveMemoryRetriever`, `ChatMessageHistory` |
| Rust SDK | ✅ Operational | Basic memory operations, MCP, KG |

### Observability

| Capability | Status | Notes |
|---|---|---|
| OpenTelemetry | ✅ Emitting | Traces exported |
| Prometheus metrics | ✅ Emitting | Standard Axum metrics |
| Outbox lag / dead-letter metrics | ❌ Not implemented | Needed for operational visibility |
| Quota deny / MCP authz deny metrics | ❌ Not implemented | Needed for governance monitoring |

### Documentation

| Capability | Status | Notes |
|---|---|---|
| API docs (Scalar) | ✅ Operational | OpenAPI at `/scalar` |
| OpenAPI schema coverage | ✅ Good | Core memory, KG, MM, MCP, admin, enterprise routes |
| IMPLEMENTATION_STATUS.md | ✅ Honest | This file — replaced false "COMPLETED" claim |
| AGENTS.md | ⚠️ Needs update | Missing governance/audit/outbox modules |
| Agent-card capability claims | ⚠️ Needs audit | Some claims may exceed actual implementation |

---

## Known Gaps (Priority-Ordered)

### P0 — Enterprise Red Lines (blocking production)

1. **D-01** — LTM write path: PG uses outbox (✅ fixed); SQLite still dual-write (acceptable for dev)
2. **D-02** — MCP `call_tool` lacks signing + capability check
3. **D-03** — Wasm sandbox is mock (MCP Plane B)
4. **D-05** — Governance is fail-open; quota doesn't count; RBAC doesn't enforce
5. **D-06** — Rate limit key uses client-controlled `X-Forwarded-For`

### P1 — Reliability & Governance Completion

6. **D-07** — Vector reconciliation scanner not implemented
7. **D-08** — Quota `used` never incremented
8. **D-09** — RBAC `pre_store`/`pre_search` only check quota, not role permissions
9. **D-11** — Governance fail-open (uninitialized hooks → allow)
10. **D-12** — Classify coverage gaps (forget, MCP, adaptive routes)

### P2 — Protocol & Integration

11. **D-16** — gRPC/WebSocket type shells
12. **D-17** — A2A handler fake data
13. **D-19** — Neo4j index initialization incomplete

### P3 — Adaptive Intelligence

14. **D-20** — Predictor hardcoded coefficients
15. **D-21** — Scheduler heuristic selection (no learning)
16. **D-22** — Planner sandbox mock

---

## Current Phase: P1 Grounding

The system is in the **P1 enterprise grounding phase** of the [delivery plan](docs/artifacts/2026-07-16-enterprise-productionization/delivery-plan.md).
Completed PRs:

| PR | Scope | Status |
|---|---|---|
| PR-1 | tenant_scope executor (RLS keystone) | ✅ Merged |
| PR-2 | Audit event store + async writer | ✅ Merged |
| PR-3 | RLS migration + wiring (4 memory layers) | ✅ Merged |
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
- **Unit tests**: `cargo test` passes
- **E2E tests**: `AMS_E2E=1 cargo test --test memory_platform_e2e` (requires PG + Qdrant + Ollama)
- **Lint**: `cargo clippy` / `cargo fmt` enforced

---

_This file is a living document. Update after each PR merge and wave completion._