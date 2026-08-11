# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Memory Management System for AI Agent & LLM workloads, with an adaptive-scheduling roadmap (config selection is currently heuristic/static; learned adaptation is planned for P3). Uses Rust (Axum) backend with React (Ant Design Pro) frontend.

## Commands

### Backend (Rust)

```bash
cd backend
cargo build              # Build the project
cargo run                # Run development server on http://127.0.0.1:8008
cargo test               # Run tests
cargo test <test_name>   # Run a specific test
```

### Frontend (React)

```bash
cd frontend/ant-design-pro-template
npm install              # Install dependencies
npm start                 # Run dev server on http://localhost:8000
npm test                 # Run tests
npm run build            # Production build
npm run lint             # Lint code
```

## Architecture

This is a **monorepo** with two main components:

### Backend (`backend/src/`)

- **routers/** — API endpoint handlers (memory, auth, user, knowledge_graph, memory_search, memory_storage, multimodal)
- **axum_routers/** — Axum-compatible router adapters
- **services/** — Core business logic
  - `scheduler.rs` — Memory scheduler (selects memory config via heuristic/static policy; learned adaptation planned for P3)
  - `analyzer.rs` — Task feature analysis (complexity, modality, reasoning depth)
  - `predictor.rs` — Performance prediction (fixed-coefficient model; fit-from-telemetry planned for P3)
  - `monitor.rs` — Resource monitoring
  - `weight_adjuster.rs` — Dynamic weight adjustment
  - `weight_strategy.rs` — Pluggable weight strategies
  - `agent.rs` — Memory agents (implements MemoryAgent trait)
  - `embedding.rs` — Embedding model service (Ollama)
  - `llm.rs` — LLM service (Ollama)
  - `memory_search.rs` — Memory search (semantic, keyword, hybrid)
  - `memory_storage.rs` — Memory storage management
  - `memory_transfer.rs` — Memory transfer (STM → LTM)
  - `qdrant.rs` — Qdrant vector database client
  - `rerank.rs` — Reranking service
- **db/** — Database repositories
  - `adapters/` — Database adapter implementations
  - `memory.rs` — Memory configuration
  - `performance.rs` — Performance metrics
  - `weights.rs` — Weight history
  - `stm.rs` — Short-term memory
  - `ltm.rs` — Long-term memory
  - `kg.rs` — Knowledge graph
  - `mm.rs` — Multimodal memory
  - `neo4j.rs` — Neo4j graph database
  - `decision_trace.rs` — Decision trace
- **models/** — Data models (memory, task, performance, resource)
- **config/** — Configuration modules
- **hoops/** — Middleware (CORS, JWT auth)
- **kernel/** — Approval nodes, fan nodes, trait definitions for the evidence graph
- **layers/** — Memory layer abstractions (STM, LTM, KG, MM layers)
- **policy/** — Cost model and scheduler policy
- **protocol/** — Protocol implementations (MCP over HTTP is real; gRPC and WebSocket are currently type-only stubs, not yet served — planned for P2)
- **runtime/** — Runtime adapters (Anthropic, OpenAI), planner sandbox (dry-run simulator), subagent pool
- **tenant/** — Multi-tenant context, isolation, and quota management
- **distributed/** — In-process coordination primitives (epoch cancellation, interrupt propagation, lease, workflow signaling). NOT a distributed cluster — HA is delegated to managed infrastructure (see `docs/adr/ADR-0005-ha-infrastructure-selection.md`)
- **mcp/** — MCP protocol (signing verification, proxy; sandbox execution is currently a mock, not real isolation — see `docs/adr/ADR-0004-mcp-sandbox-execution-model.md`)
- **web/** — Web-specific handlers

### Frontend (`frontend/ant-design-pro-template/`)

- Uses Umi 4 + Ant Design Pro 6.0
- Pages in `src/pages/`:
  - `Dashboard/` - Dashboard overview
  - `TaskAnalysis/` - Task feature analysis
  - `MemoryConfig/` - Memory configuration management
  - `MemoryDecisionTrace/` - Decision trace viewer
  - `MemoryDetails/` - Memory details (STM, LTM, KG, MM lists)
  - `MemoryManagement/` - Memory management
  - `Performance/` - Performance analytics
  - `ResourceMonitor/` - Resource monitoring
  - `WeightHistory/` - Weight adjustment history

## API Endpoints

### Memory Search Endpoints

| Method | Endpoint                          | Description         |
| ------ | --------------------------------- | ------------------- |
| GET    | `/api/v1/memory/search/ltm`       | List LTM entries    |
| POST   | `/api/v1/memory/search/ltm`       | Search LTM by query |
| GET    | `/api/kg/entities`                | List KG entities    |
| GET    | `/api/mm/list`                    | List MM entries     |
| GET    | `/api/v1/memory/storage/sessions` | List STM sessions   |

### Frontend Service Files

- `src/services/memory/storageApi.ts` - STM/LTM storage APIs
- `src/services/memory/knowledgeGraphApi.ts` - KG APIs
- `src/services/memory/multimodalApi.ts` - MM APIs

### Coordination & Signaling Endpoints

> In-process coordination only (single node). No self-built cluster — HA is delegated to managed infra.

| Method | Endpoint                                      | Description                        |
| ------ | --------------------------------------------- | ---------------------------------- |
| GET    | `/api/v1/distributed/pool/status`             | Sub-agent pool status              |
| POST   | `/api/v1/distributed/pool/allocate`           | Allocate sub-agent slots           |
| POST   | `/api/v1/distributed/pool/release`            | Release sub-agent slots            |
| GET    | `/api/v1/distributed/signals/{workflow_id}`   | Get signals for a workflow         |
| POST   | `/api/v1/distributed/signals/publish`         | Publish a workflow signal          |

### Tenant Endpoints

| Method | Endpoint                                  | Description                |
| ------ | ----------------------------------------- | -------------------------- |
| GET    | `/api/tenants`                            | List tenants               |
| POST   | `/api/tenants`                            | Register a tenant          |
| POST   | `/api/tenants/{tenant_id}/search`         | Tenant-scoped search       |
| GET    | `/api/tenants/{tenant_id}/sessions`       | List tenant sessions       |
| POST   | `/api/tenants/access/check`               | Check tenant access        |

### Health & System Endpoints

> There is still **no** root `/health`. Orchestrator probes live at `/livez` and
> `/readyz` (`backend/src/routers/probes.rs`), unauthenticated at the root next
> to `/metrics` — a kubelet cannot present a JWT.
>
> `/livez` checks **nothing external on purpose**: a failing liveness probe
> restarts the container, and restarting cannot fix a down database, so probing
> dependencies there would turn a dependency blip into a restart storm.
> `/readyz` is the one that probes dependencies, because failing it only drains
> the instance from the load balancer.

| Method | Endpoint                    | Description                                      |
| ------ | --------------------------- | ------------------------------------------------ |
| GET    | `/livez`                    | Process liveness — always 200 while serving, checks no dependency |
| GET    | `/readyz`                   | Readiness — round-trips PostgreSQL (`SELECT 1`) + Qdrant (gRPC `health_check`); `503` + per-dependency detail on failure |
| GET    | `/api/v1/memory/health`     | Real DB probe (`SELECT 1`), returns `degraded` on failure |
| GET    | `/api/v1/memory/v1/health`  | In-process memory-layer status — **not a dependency probe**; carries `is_dependency_probe: false`. Layer backends are in-memory stubs (see A-6), so `latency_ms` is `null` rather than fabricated |
| GET    | `/metrics`                  | Prometheus metrics (unauthenticated)             |

## Key Patterns

### Adding New API Endpoints

1. Add handler in `backend/src/routers/memory.rs`
2. Implement Axum handlers with typed extractors (`Json`, `Query`, `Path`, `Extension`)
3. Register route in `backend/src/routers/mod.rs`

### Adding New Weight Strategies

Implement `WeightStrategy` trait and add to the adjuster chain. See `backend/src/services/weight_strategy.rs`.

### Adding New Memory Agents

Implement `MemoryAgent` trait for custom analyzer/predictor/scheduler behavior. See `backend/src/services/agent.rs`.

### Database Operations

Use SQLx with compile-time checks:

```rust
sqlx::query!("SELECT * FROM table WHERE id = $1", id)
sqlx::query_as!(Model, "SELECT * FROM table")
```

### Error Handling

Use `AppError` from `backend/src/error.rs` for structured error responses with proper HTTP status codes.

### Multi-Tenant Isolation

Repository queries are scoped by `tenant_id` at the **application layer** today. Use the `TenantContext` extractor to access tenant info in handlers. See `backend/src/tenant/isolation.rs`. Schema-level enforcement (Postgres RLS + `tenant_id NOT NULL`) is planned for P1 — until then isolation depends on every query path passing the correct `tenant_id`.

### MCP Sandbox

Two planes, per `docs/adr/ADR-0004-mcp-sandbox-execution-model.md`. `call_tool`
classifies the requested tool into a plane **before** any Plane A guard runs —
that ordering matters, because Plane A's capability check is deny-by-default
over the first-party tool set and would reject every extension tool as
`UnknownTool` if it ran first.

- **Plane A — trusted first-party** (the 5 `memory_*` tools): signature-verified
  (`mcp/signing.rs`, HMAC-SHA256) at both listing and call time, then capability
  authorization derived from the caller's RBAC role
  (`mcp::capability::capabilities_for_role`), then governance, then **native**
  execution under tenant isolation. Deliberately *not* run through WASM: these
  tools need privileged DB access, so sandboxing them would force wide-open host
  functions — no isolation gain, plus fuel limits and new failure modes.
- **Plane B — untrusted extensions**: routed through `mcp/sandbox_proxy.rs` into
  `mcp/sandbox.rs`, which is a **real wasmtime sandbox** (fuel limits, capability
  policy). The path is wired and tested, but the extension **registry is empty in
  production** — there is no first-party mechanism to register third-party tools
  yet. Unknown tools are rejected and audited.

Still open (see backlog A-2): the production registration source for extension
tools, Plane B signing, and the capability→host-function grant model.
`execute_wasm`'s current capability check requires *all* capabilities to run any
module, which is a placeholder rather than a real least-privilege model.

## Environment Requirements

- Rust: 1.89+
- Node.js: 20+
- PostgreSQL 14+ (via Docker)
- Qdrant (via Docker)
- Neo4j (optional, for knowledge graph)
- Ollama — or any OpenAI-compatible LLM/embedding endpoint (`config.llm` / `config.embedding`). **Not fully optional.** The **embedding** backend is a *hard* dependency on the LTM write and search paths — vectors are generated on the hot path, so LTM writes/search fail if the embedding backend is unreachable. Startup **fails fast** on deterministic embedding misconfiguration (empty `base_url`/`model`, `dimension == 0`, placeholder OpenAI key) via `validate_embedding_config` in `config/mod.rs`; reachability is checked at runtime by `/readyz`, not at startup. The **LLM summary** is degradable: when the LLM backend is unreachable or returns an error status, an LTM write still succeeds with an empty summary marked `summaryStatus: "pending"` for later backfill; *malformed* LLM output is surfaced (500), not silently degraded. See `store_ltm_for_tenant` in `services/memory_storage.rs`.
