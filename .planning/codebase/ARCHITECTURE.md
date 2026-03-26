# Architecture

**Analysis Date:** 2026-03-26

## Pattern Overview

**Overall:** Modular monorepo with a Rust backend and a Umi/React frontend. The backend is a layered service/repository application, but it currently exposes two HTTP router trees: the booted Axum router in `backend/src/axum_routers/mod.rs` and a larger, richer router composition in `backend/src/routers/mod.rs` that is still referenced by tests in `backend/src/main.rs`.

**Key Characteristics:**
- Use `backend/src/main.rs` as the backend composition root. It initializes config, databases, background daemons, and the live HTTP router.
- Treat `backend/src/services/` as the business-logic layer. API handlers should delegate orchestration there instead of embedding selection, storage, or search logic in router modules.
- Treat `backend/src/db/` as the persistence boundary. Repositories encapsulate SQLx access and convert rows into domain models from `backend/src/models/`.

## Layers

**Frontend Application Layer:**
- Purpose: Render the operator UI and call backend APIs.
- Location: `frontend/ant-design-pro-template/src/pages/`, `frontend/ant-design-pro-template/src/app.tsx`, `frontend/ant-design-pro-template/config/routes.ts`
- Contains: page components, layout/runtime config, route declarations
- Depends on: generated/manual request clients in `frontend/ant-design-pro-template/src/services/`
- Used by: the Umi runtime configured in `frontend/ant-design-pro-template/config/config.ts`

**Frontend API Client Layer:**
- Purpose: Wrap backend endpoints in typed request helpers.
- Location: `frontend/ant-design-pro-template/src/services/memory/`, `frontend/ant-design-pro-template/src/services/ant-design-pro/`
- Contains: request wrappers such as `selectMemoryConfig` in `frontend/ant-design-pro-template/src/services/memory/api.ts` and list/search helpers in `frontend/ant-design-pro-template/src/services/memory/storageApi.ts`
- Depends on: `request` from `@umijs/max`
- Used by: page components such as `frontend/ant-design-pro-template/src/pages/Dashboard/index.tsx` and `frontend/ant-design-pro-template/src/pages/MemoryDetails/index.tsx`

**HTTP Routing Layer:**
- Purpose: Bind URLs to handlers and serialize request/response DTOs.
- Location: live router in `backend/src/axum_routers/`; richer alternate router tree in `backend/src/routers/`
- Contains: handler modules like `backend/src/axum_routers/memory.rs`, `backend/src/axum_routers/memory_storage.rs`, and `backend/src/routers/memory.rs`
- Depends on: `backend/src/services/`, `backend/src/models/`, `backend/src/error.rs`
- Used by: `backend/src/main.rs` through `axum_routers::create_router()`

**Service / Orchestration Layer:**
- Purpose: Execute adaptive-memory workflows, storage/search pipelines, and background jobs.
- Location: `backend/src/services/`
- Contains: adaptive selection in `backend/src/services/scheduler.rs`, explain/persist orchestration in `backend/src/services/memory_orchestrator.rs`, LTM/STM operations in `backend/src/services/memory_storage.rs`, and hybrid retrieval in `backend/src/services/memory_search.rs`
- Depends on: repositories in `backend/src/db/`, config in `backend/src/config/`, and external clients such as `backend/src/services/qdrant.rs`, `backend/src/services/embedding.rs`, and `backend/src/services/llm.rs`
- Used by: router handlers and startup hooks in `backend/src/main.rs`

**Persistence Layer:**
- Purpose: Own database pools, migrations, and repository queries.
- Location: `backend/src/db/`
- Contains: `MemoryConfigRepository` in `backend/src/db/memory.rs`, `STMRepository` in `backend/src/db/stm.rs`, and other repositories for LTM, KG, MM, decision traces, and weights
- Depends on: `sqlx`, backend config, and model structs
- Used by: service modules and some router modules

**Domain / Model Layer:**
- Purpose: Define shared data contracts across routers, services, and repositories.
- Location: `backend/src/models/`, `backend/src/kernel/`
- Contains: task, memory, resource, performance, and agent types plus lower-level kernel traits/types
- Depends on: serde/sqlx derives
- Used by: all backend layers

## Data Flow

**Adaptive Memory Selection Flow:**

1. Frontend pages call `selectMemoryConfig` or `getDecisionTrace` from `frontend/ant-design-pro-template/src/services/memory/api.ts`.
2. The richer backend handler in `backend/src/routers/memory.rs` deserializes `SelectMemoryRequest` and calls `backend/src/services/memory_orchestrator.rs`.
3. `select_memory` in `backend/src/services/memory_orchestrator.rs` drives `AdaptiveMemoryScheduler` in `backend/src/services/scheduler.rs`.
4. The scheduler coordinates `TaskCharacteristicAnalyzer`, `PerformancePredictionModel`, `ResourceMonitor`, and `DynamicWeightAdjuster`, then persists configs via `MemoryConfigRepository` in `backend/src/db/memory.rs`.
5. Optional trace persistence goes through `DecisionTraceRepository` and `WeightHistoryRepository` in `backend/src/db/decision_trace.rs` and `backend/src/db/weights.rs`.

**Memory Storage and Retrieval Flow:**

1. Frontend detail pages use clients from `frontend/ant-design-pro-template/src/services/memory/storageApi.ts`, `knowledgeGraphApi.ts`, and `multimodalApi.ts`.
2. Storage handlers delegate to `backend/src/services/memory_storage.rs` or repository-backed handlers.
3. `MemoryStorageService::store_ltm` calls the LLM extractor in `backend/src/services/llm.rs`, embedding generation in `backend/src/services/embedding.rs`, vector insert in `backend/src/services/qdrant.rs`, and metadata persistence in `backend/src/db/ltm.rs`.
4. `MemorySearchService::search_ltm` reverses that path: generate a query embedding, search Qdrant, hydrate metadata from `backend/src/db/ltm.rs`, and optionally rerank via `backend/src/services/rerank.rs`.

**Startup / Control Flow:**

1. `backend/src/main.rs` loads configuration from `backend/src/config/mod.rs`.
2. `backend/src/db/mod.rs` initializes PostgreSQL or SQLite and runs migrations.
3. Startup registers background subsystems such as `write_queue`, `hardware_detector`, `vector_guard`, `memory_ingestion`, `information_guard`, `strategy_mutator`, `init_neo4j`, and `memory_transfer`.
4. The process serves the live API using `axum_routers::create_router()` plus CORS middleware from `backend/src/hoops/mod.rs` and `backend/src/web/cors.rs`.

**State Management:**
- Backend state is mostly global singleton style: config uses `OnceLock` in `backend/src/config/mod.rs`, DB pools use `OnceLock` in `backend/src/db/mod.rs`, and several routers cache service instances with `once_cell::sync::Lazy` in `backend/src/routers/memory.rs`.
- Frontend state is page-local React state plus `useRequest` fetch state, as shown in `frontend/ant-design-pro-template/src/pages/Dashboard/index.tsx` and `frontend/ant-design-pro-template/src/pages/MemoryDetails/index.tsx`.

## Key Abstractions

**AdaptiveMemoryScheduler:**
- Purpose: Central coordinator for task analysis, resource evaluation, prediction, and weight adjustment.
- Examples: `backend/src/services/scheduler.rs`, `backend/src/services/memory_orchestrator.rs`
- Pattern: service-level orchestrator composed of smaller strategy/agent objects

**MemoryAgent Trait:**
- Purpose: Standardize the observe -> decide -> act lifecycle for analyzer, predictor, and scheduler services.
- Examples: `backend/src/services/agent.rs`, implementations in `backend/src/services/analyzer.rs`, `backend/src/services/predictor.rs`, `backend/src/services/scheduler.rs`
- Pattern: behavioral interface for pluggable adaptive components

**Repository Pattern:**
- Purpose: Isolate persistence details from workflow logic.
- Examples: `backend/src/db/memory.rs`, `backend/src/db/stm.rs`, `backend/src/db/ltm.rs`
- Pattern: static repository methods over a globally initialized SQLx pool

**Kernel / Runtime Adapter Boundary:**
- Purpose: Provide lower-level memory primitives and external-agent integrations that are broader than the currently booted HTTP API.
- Examples: `backend/src/kernel/traits.rs`, `backend/src/agent/memory_agent.rs`, `backend/src/runtime/mod.rs`
- Pattern: reusable subsystem boundary for agent runtime integration; keep new runtime adapters under `backend/src/runtime/`

## Entry Points

**Backend Server:**
- Location: `backend/src/main.rs`
- Triggers: `cargo run` in `backend/`
- Responsibilities: initialize config, DB, background services, Neo4j, transfer daemons, and serve Axum routes

**Live Backend Router:**
- Location: `backend/src/axum_routers/mod.rs`
- Triggers: called from `backend/src/main.rs`
- Responsibilities: merge live Axum route modules, expose OpenAPI JSON and Scalar UI, and set fallback handling

**Secondary Backend Router Tree:**
- Location: `backend/src/routers/mod.rs`
- Triggers: used directly by tests in `backend/src/main.rs`
- Responsibilities: compose the more complete API surface, including memory, agent, billing, enterprise, visualization, and protected routes

**Frontend Application Bootstrapping:**
- Location: `frontend/ant-design-pro-template/config/config.ts`, `frontend/ant-design-pro-template/src/app.tsx`
- Triggers: `npm start` or `npm run build` in `frontend/ant-design-pro-template/`
- Responsibilities: configure Umi, routes, proxies, layout, initial state, and request base URL

## Error Handling

**Strategy:** Backend handlers return `crate::AppError`-based results, typically through `JsonResult<T>` from `backend/src/main.rs` and `backend/src/error.rs`.

**Patterns:**
- Use repository/service methods that convert lower-level failures into `AppError`, as shown in `backend/src/db/memory.rs` and `backend/src/services/memory_storage.rs`.
- Use rollback or compensating behavior around multi-step writes, as shown by Qdrant rollback in `backend/src/services/memory_storage.rs`.
- Frontend pages mostly log fetch failures locally with `console.error`, as shown in `frontend/ant-design-pro-template/src/pages/MemoryDetails/index.tsx`.

## Cross-Cutting Concerns

**Logging:** Use `tracing` across backend services and repositories, configured from `backend/src/config/log_config.rs` and initialized in `backend/src/main.rs`.
**Validation:** Request typing relies on serde DTOs in route modules such as `backend/src/routers/memory.rs` and `backend/src/axum_routers/memory.rs`; business validation happens in services like `backend/src/services/vector_guard.rs` and scheduler constraint enforcement in `backend/src/services/scheduler.rs`.
**Authentication:** JWT and auth middleware live under `backend/src/hoops/jwt.rs` and are applied in `backend/src/routers/mod.rs`; the live Axum router in `backend/src/axum_routers/mod.rs` is more lightly wrapped and currently omits much of that protected composition.

---

*Architecture analysis: 2026-03-26*
