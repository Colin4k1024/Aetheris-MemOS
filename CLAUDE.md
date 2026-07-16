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
| GET    | `/api/v1/distributed/epoch/status`            | Epoch / active-context status      |
| GET    | `/api/v1/distributed/pool/status`             | Sub-agent pool status              |
| POST   | `/api/v1/distributed/pool/allocate`           | Allocate sub-agent slots           |
| POST   | `/api/v1/distributed/pool/release`            | Release sub-agent slots            |
| GET    | `/api/v1/distributed/signals/{workflow_id}`   | Get signals for a workflow         |
| POST   | `/api/v1/distributed/signals/publish`         | Publish a workflow signal          |

### Tenant & Quota Endpoints

| Method | Endpoint                          | Description              |
| ------ | --------------------------------- | ------------------------ |
| GET    | `/api/v1/tenant/context`         | Get current tenant context |
| GET    | `/api/v1/tenant/quota`           | Get tenant quota status  |
| POST   | `/api/v1/tenant/quota/reset`     | Reset quota counters    |

### Health & System Endpoints

| Method | Endpoint                          | Description              |
| ------ | --------------------------------- | ------------------------ |
| GET    | `/api/v1/health`                  | Full system health check |
| GET    | `/api/v1/health/liveness`         | Liveness probe           |
| GET    | `/api/v1/health/readiness`        | Readiness probe          |

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

Tool *listing* is signature-verified (`backend/src/mcp/signing.rs`, HMAC-SHA256). **Tool execution is not yet sandboxed**: `backend/src/mcp/sandbox.rs` is currently a mock (returns input unchanged) and is not wired into the call path — `call_tool` runs the first-party memory tools natively under tenant isolation. Real WASM isolation, call-time signing, and capability authorization are planned; see `docs/adr/ADR-0004-mcp-sandbox-execution-model.md`.

## Environment Requirements

- Rust: 1.89+
- Node.js: 20+
- PostgreSQL 14+ (via Docker)
- Qdrant (via Docker)
- Neo4j (optional, for knowledge graph)
