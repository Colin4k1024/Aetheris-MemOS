# Architecture

**Analysis Date:** 2026-03-28

## Pattern Overview

**Overall:** Modular monorepo with a Rust/Axum backend and a React/Umi frontend. The backend follows a layered service/repository architecture with a pluggable kernel boundary. Two HTTP router trees exist: the live Axum router in `backend/src/axum_routers/mod.rs` and a richer alternate router composition in `backend/src/routers/mod.rs`.

**Key Characteristics:**
- Backend composition root is `backend/src/main.rs` - initializes config, databases, background daemons, and the HTTP router
- Business logic lives in `backend/src/services/` - API handlers delegate orchestration there
- Persistence is encapsulated in `backend/src/db/` - repositories convert rows to domain models
- Domain models shared across layers in `backend/src/models/` and `backend/src/kernel/`

## High-Level Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        Frontend (React/Umi)                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │   Pages/     │  │  Services/   │  │    Components/       │  │
│  │ Dashboard    │  │  memory/     │  │  Header, Footer      │  │
│  │ MemoryConfig │  │  storageApi  │  │  RightContent        │  │
│  │ MemoryDetails│  │  kgApi       │  │                      │  │
│  │ TaskAnalysis │  │  mmApi       │  │                      │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
└────────────────────────────┬────────────────────────────────────┘
                             │ HTTP/REST
┌────────────────────────────▼────────────────────────────────────┐
│                    Backend (Rust/Axum)                           │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                   HTTP Routing Layer                      │   │
│  │  axum_routers/  (live)     routers/ (richer alternate)  │   │
│  │  memory, auth, user,        memory, agent, enterprise,   │   │
│  │  memory_search,             billing, metrics, tenant,     │   │
│  │  knowledge_graph, mm        visualization                │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│  ┌───────────────────────────▼─────────────────────────────┐   │
│  │                   Service Layer                            │   │
│  │  AdaptiveMemoryScheduler  │  MemoryOrchestrator           │   │
│  │  TaskCharacteristicAnalyzer│  MemoryStorageService         │   │
│  │  PerformancePredictionModel│  MemorySearchService          │   │
│  │  ResourceMonitor           │  EvidenceGraphService         │   │
│  │  DynamicWeightAdjuster     │  QdrantClient, LLM, Embedding│   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│  ┌───────────────────────────▼─────────────────────────────┐   │
│  │                   Domain Layer                             │   │
│  │  models/ (API types)        kernel/ (core traits/types)    │   │
│  │  task, memory, resource,   MemoryKernel trait, LayerType, │   │
│  │  performance, agent        MemoryId, MemoryEntry          │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│  ┌───────────────────────────▼─────────────────────────────┐   │
│  │                 Persistence Layer                          │   │
│  │  db/                                                       │   │
│  │  memory.rs (MemoryConfig)   stm.rs (ShortTermMemory)      │   │
│  │  ltm.rs (LongTermMemory)   kg.rs (KnowledgeGraph)        │   │
│  │  mm.rs (Multimodal)        evidence_graph.rs (Evidence)  │   │
│  │  decision_trace.rs          weights.rs                     │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│  ┌───────────────────────────▼─────────────────────────────┐   │
│  │               External Services                            │   │
│  │  PostgreSQL/SQLite (sqlx)  │  Qdrant (vector search)    │   │
│  │  Neo4j (graph DB)          │  Ollama (LLM/Embedding)     │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Layers

**Frontend Application Layer:**
- Purpose: Render operator UI and call backend APIs
- Location: `frontend/ant-design-pro-template/src/pages/`, `frontend/ant-design-pro-template/src/app.tsx`
- Contains: page components, layout/runtime config, route declarations
- Depends on: generated/manual request clients in `frontend/ant-design-pro-template/src/services/`
- Used by: Umi runtime in `frontend/ant-design-pro-template/config/config.ts`

**Frontend API Client Layer:**
- Purpose: Wrap backend endpoints in typed request helpers
- Location: `frontend/ant-design-pro-template/src/services/memory/`
- Contains: `api.ts` (selectMemoryConfig, getDecisionTrace), `storageApi.ts` (STM/LTM operations), `knowledgeGraphApi.ts`, `multimodalApi.ts`
- Depends on: `request` from `@umijs/max`
- Used by: page components like `Dashboard/index.tsx`, `MemoryDetails/index.tsx`

**HTTP Routing Layer:**
- Purpose: Bind URLs to handlers and serialize request/response DTOs
- Location: live router in `backend/src/axum_routers/`; alternate router tree in `backend/src/routers/`
- Contains: handler modules for memory, auth, user, memory_search, memory_storage, knowledge_graph, multimodal
- Depends on: `backend/src/services/`, `backend/src/models/`, `backend/src/error.rs`
- Used by: `backend/src/main.rs` through `axum_routers::create_router()`

**Service / Orchestration Layer:**
- Purpose: Execute adaptive-memory workflows, storage/search pipelines, and background jobs
- Location: `backend/src/services/`
- Key components:
  - `scheduler.rs` - `AdaptiveMemoryScheduler` for task analysis, resource evaluation, prediction
  - `memory_orchestrator.rs` - explain/dry-run/persist orchestration, calls evidence graph
  - `memory_storage.rs` - STM/LTM storage workflow with LLM extraction, embedding, Qdrant
  - `memory_search.rs` - vector, keyword, and hybrid retrieval
  - `evidence_graph.rs` - decision trace recording with hash chain verification
- Depends on: repositories in `backend/src/db/`, config in `backend/src/config/`, external clients (qdrant, embedding, llm)

**Kernel / Abstraction Layer:**
- Purpose: Provide lower-level memory primitives and agent integrations
- Location: `backend/src/kernel/`, `backend/src/agent/`
- Contains: `kernel/traits.rs` (MemoryKernel trait), `kernel/types.rs` (LayerType, MemoryId, MemoryEntry), `agent/memory_agent.rs` (MemoryAgent interface)
- Pattern: reusable subsystem boundary for agent runtime integration

**Persistence Layer:**
- Purpose: Own database pools, migrations, and repository queries
- Location: `backend/src/db/`
- Key repositories: `memory.rs` (MemoryConfig), `stm.rs`, `ltm.rs`, `kg.rs`, `mm.rs`, `decision_trace.rs`, `weights.rs`, `evidence_graph.rs`
- Depends on: `sqlx`, backend config, model structs
- Used by: service modules

**Domain / Model Layer:**
- Purpose: Define shared data contracts across routers, services, and repositories
- Location: `backend/src/models/`, `backend/src/kernel/`
- Contains: task, memory, resource, performance, agent types plus kernel traits/types
- Depends on: serde/sqlx derives

## Data Flow

**Adaptive Memory Selection Flow:**

1. Frontend calls `selectMemoryConfig` from `frontend/ant-design-pro-template/src/services/memory/api.ts`
2. Backend handler in `backend/src/routers/memory.rs` deserializes `SelectMemoryRequest` and calls `memory_orchestrator::select_memory`
3. `select_memory` in `backend/src/services/memory_orchestrator.rs` drives `AdaptiveMemoryScheduler`
4. The scheduler coordinates `TaskCharacteristicAnalyzer`, `PerformancePredictionModel`, `ResourceMonitor`, and `DynamicWeightAdjuster`
5. Results are persisted via `MemoryConfigRepository` and optionally via `DecisionTraceRepository` + evidence graph

**Memory Storage and Retrieval Flow:**

1. Frontend detail pages use clients from `frontend/ant-design-pro-template/src/services/memory/storageApi.ts`, `knowledgeGraphApi.ts`, `multimodalApi.ts`
2. Storage handlers delegate to `backend/src/services/memory_storage.rs`
3. `MemoryStorageService::store_ltm` calls LLM extractor, embedding generation, Qdrant insert, and metadata persistence
4. `MemorySearchService::search_ltm` generates query embedding, searches Qdrant, hydrates metadata, optionally reranks

**Workflow Evidence Recording Flow:**

1. When `persist_trace=true` in selection request, `memory_orchestrator::persist_trace_record` is called
2. `record_decision_trace_as_evidence` in `backend/src/services/evidence_graph.rs` creates a hash chain
3. Evidence nodes are built from decision trace steps, edges link sequential nodes
4. `EvidenceGraphRepository` persists runs, nodes, and edges to database
5. `verify_chain` validates hash chain integrity and returns `WorkflowEvidenceVerification`
6. Evidence is exposed via `GET /api/v1/workflows/{id}/evidence`

**Startup / Control Flow:**

1. `backend/src/main.rs` loads configuration from `backend/src/config/mod.rs`
2. `backend/src/db/mod.rs` initializes PostgreSQL or SQLite and runs migrations
3. Startup registers background subsystems: `write_queue`, `hardware_detector`, `vector_guard`, `memory_ingestion`, `information_guard`, `strategy_mutator`, `init_neo4j`, `memory_transfer`
4. Process serves API using `axum_routers::create_router()` with CORS middleware

## Key Abstractions

**AdaptiveMemoryScheduler:**
- Purpose: Central coordinator for task analysis, resource evaluation, prediction, weight adjustment
- Location: `backend/src/services/scheduler.rs`, `backend/src/services/memory_orchestrator.rs`
- Pattern: service-level orchestrator composed of smaller strategy/agent objects

**MemoryAgent Trait:**
- Purpose: Standardize the observe -> decide -> act lifecycle for analyzer, predictor, scheduler
- Location: `backend/src/services/agent.rs`, implementations in analyzer, predictor, scheduler services
- Pattern: behavioral interface for pluggable adaptive components

**EvidenceGraphService:**
- Purpose: Record decision traces as tamper-evident hash chains for audit/compliance
- Location: `backend/src/services/evidence_graph.rs`, `backend/src/db/evidence_graph.rs`
- Pattern: event sourcing lite - each decision step becomes a node with SHA256 hash linking
- Exported via: `GET /api/v1/workflows/{id}/evidence`

**Repository Pattern:**
- Purpose: Isolate persistence details from workflow logic
- Location: `backend/src/db/memory.rs`, `backend/src/db/stm.rs`, `backend/src/db/ltm.rs`, etc.
- Pattern: static repository methods over globally initialized SQLx pool

**MemoryKernel Trait:**
- Purpose: Unified interface for memory operations across different storage backends
- Location: `backend/src/kernel/traits.rs`
- Operations: store, retrieve, search, update, delete, evict, stats
- Implemented by: layer implementations in `backend/src/layers/`

## Entry Points

**Backend Server:**
- Location: `backend/src/main.rs`
- Triggers: `cargo run` in `backend/`
- Responsibilities: initialize config, DB, background services, Neo4j, transfer daemons, serve Axum routes

**Live Backend Router:**
- Location: `backend/src/axum_routers/mod.rs`
- Triggers: called from `backend/src/main.rs`
- Responsibilities: merge live Axum route modules, expose OpenAPI JSON and Scalar UI, fallback handling

**Secondary Backend Router Tree:**
- Location: `backend/src/routers/mod.rs`
- Triggers: used by tests in `backend/src/main.rs`
- Responsibilities: compose more complete API surface including memory, agent, billing, enterprise

**Frontend Application Bootstrapping:**
- Location: `frontend/ant-design-pro-template/config/config.ts`, `frontend/ant-design-pro-template/src/app.tsx`
- Triggers: `npm start` or `npm run build` in `frontend/ant-design-pro-template/`
- Responsibilities: configure Umi, routes, proxies, layout, initial state, request base URL

## Error Handling

**Strategy:** Backend handlers return `crate::AppError`-based results through `JsonResult<T>` from `backend/src/error.rs`.

**Patterns:**
- Repository/service methods convert lower-level failures into `AppError`
- Multi-step writes use rollback or compensating behavior (e.g., Qdrant rollback in memory_storage)
- Frontend pages log fetch failures locally with `console.error`

**Error Types:**
- `Public(String)` - user-facing error messages
- `Internal(String)` - internal errors logged but not exposed
- `Unauthorized(String)` - authentication failures
- `Forbidden(String)` - authorization failures
- `NotFound(String)` - resource not found
- `BadRequest(String)` - validation errors

## Cross-Cutting Concerns

**Logging:** Use `tracing` across backend services and repositories, configured from `backend/src/config/log_config.rs`

**Validation:** Request typing relies on serde DTOs in route modules; business validation in services like `vector_guard.rs`

**Authentication:** JWT middleware in `backend/src/hoops/jwt.rs`, applied in router configuration

**CORS:** CORS middleware from `backend/src/hoops/cors.rs` and `backend/src/web/cors.rs`

---

*Architecture analysis: 2026-03-28*
