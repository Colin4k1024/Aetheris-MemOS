# Codebase Structure

**Analysis Date:** 2026-03-28

## Directory Layout

```
adaptive-memory-system/
├── backend/                              # Rust backend service
│   ├── src/
│   │   ├── main.rs                      # Entry point, startup wiring
│   │   ├── lib.rs                       # Library root, re-exports
│   │   ├── axum_routers/                # Live Axum router (booted by main.rs)
│   │   │   ├── mod.rs                   # Router creation, OpenAPI doc
│   │   │   ├── memory.rs                # Memory endpoints + workflow evidence
│   │   │   ├── memory_search.rs         # Search endpoints
│   │   │   ├── memory_storage.rs        # Storage endpoints
│   │   │   ├── auth.rs                  # Authentication
│   │   │   ├── user.rs                  # User management
│   │   │   ├── knowledge_graph.rs       # KG endpoints
│   │   │   └── multimodal.rs            # Multimodal endpoints
│   │   ├── routers/                     # Alternate richer router tree
│   │   │   ├── mod.rs                   # Root router composition
│   │   │   ├── memory.rs                # Memory handlers
│   │   │   ├── agent.rs                 # Agent handlers
│   │   │   ├── auth.rs                  # Auth handlers
│   │   │   ├── user.rs                  # User handlers
│   │   │   ├── billing.rs               # Billing handlers
│   │   │   ├── dashboard.rs             # Dashboard handlers
│   │   │   ├── enterprise.rs             # Enterprise handlers
│   │   │   ├── knowledge_graph.rs       # KG handlers
│   │   │   ├── memory_pool.rs           # Memory pool handlers
│   │   │   ├── memory_search.rs         # Search handlers
│   │   │   ├── memory_storage.rs        # Storage handlers
│   │   │   ├── metrics.rs               # Metrics handlers
│   │   │   ├── mcp.rs                   # MCP protocol handlers
│   │   │   ├── multimodal.rs            # Multimodal handlers
│   │   │   ├── multi_tenant_router.rs   # Multi-tenant routing
│   │   │   ├── snapshot.rs              # Snapshot handlers
│   │   │   ├── tenant.rs                # Tenant handlers
│   │   │   └── visualization.rs         # Visualization handlers
│   │   ├── services/                     # Business logic
│   │   │   ├── scheduler.rs             # AdaptiveMemoryScheduler
│   │   │   ├── memory_orchestrator.rs   # Orchestration (explain, dry-run, persist)
│   │   │   ├── memory_storage.rs        # STM/LTM storage workflow
│   │   │   ├── memory_search.rs         # Vector/keyword/hybrid search
│   │   │   ├── evidence_graph.rs        # Decision trace as hash chain
│   │   │   ├── analyzer.rs              # TaskCharacteristicAnalyzer
│   │   │   ├── predictor.rs             # PerformancePredictionModel
│   │   │   ├── monitor.rs               # ResourceMonitor
│   │   │   ├── weight_adjuster.rs       # DynamicWeightAdjuster
│   │   │   ├── llm.rs                  # LLM service (Ollama)
│   │   │   ├── embedding.rs            # Embedding service
│   │   │   ├── qdrant.rs               # Qdrant vector DB client
│   │   │   ├── rerank.rs               # Reranking service
│   │   │   ├── memory_transfer.rs      # STM -> LTM transfer
│   │   │   ├── memory_pool.rs          # Memory pool management
│   │   │   ├── consolidation.rs         # Memory consolidation
│   │   │   ├── agent.rs                # Agent runtime
│   │   │   ├── kg.rs                   # Knowledge graph service
│   │   │   ├── ltm.rs                  # Long-term memory service
│   │   │   ├── mm.rs                   # Multimodal memory service
│   │   │   ├── stm.rs                  # Short-term memory service
│   │   │   ├── neo4j.rs               # Neo4j client
│   │   │   ├── metrics.rs              # Metrics collection
│   │   │   ├── mod.rs                  # Service module re-exports
│   │   │   └── [30+ additional services]
│   │   ├── db/                         # Persistence layer
│   │   │   ├── mod.rs                  # DB init, pool management
│   │   │   ├── adapters/               # DB adapter abstractions
│   │   │   ├── memory.rs               # MemoryConfigRepository
│   │   │   ├── stm.rs                  # STMRepository
│   │   │   ├── ltm.rs                  # LTMRepository
│   │   │   ├── kg.rs                   # KGRepository
│   │   │   ├── mm.rs                  # MMRepository
│   │   │   ├── neo4j.rs                # Neo4j initialization
│   │   │   ├── performance.rs          # PerformanceMetricsRepository
│   │   │   ├── weights.rs              # WeightHistoryRepository
│   │   │   ├── decision_trace.rs       # DecisionTraceRepository
│   │   │   ├── evidence_graph.rs       # EvidenceGraphRepository
│   │   │   └── agent.rs                # AgentRepository
│   │   ├── models/                     # Domain models
│   │   │   ├── mod.rs                  # Re-exports
│   │   │   ├── memory.rs               # Memory types
│   │   │   ├── task.rs                # Task types
│   │   │   ├── resource.rs            # Resource types
│   │   │   ├── performance.rs          # Performance types
│   │   │   └── agent.rs               # Agent types
│   │   ├── kernel/                     # Core kernel abstractions
│   │   │   ├── mod.rs                 # Kernel module
│   │   │   ├── traits.rs              # MemoryKernel trait
│   │   │   ├── types.rs               # LayerType, MemoryId, MemoryEntry
│   │   │   └── error.rs               # MemoryError, MemoryResult
│   │   ├── layers/                    # Memory layer implementations
│   │   │   ├── mod.rs
│   │   │   ├── stm_layer.rs           # STM layer
│   │   │   ├── ltm_layer.rs           # LTM layer
│   │   │   ├── kg_layer.rs            # KG layer
│   │   │   └── mm_layer.rs            # MM layer
│   │   ├── config/                    # Configuration
│   │   │   ├── mod.rs                 # Config discovery, ServerConfig
│   │   │   ├── db_config.rs           # Database config
│   │   │   ├── llm_config.rs         # LLM config
│   │   │   ├── embedding_config.rs    # Embedding config
│   │   │   ├── qdrant_config.rs      # Qdrant config
│   │   │   ├── neo4j_config.rs       # Neo4j config
│   │   │   ├── rerank_config.rs      # Rerank config
│   │   │   └── log_config.rs         # Logging config
│   │   ├── agent/                     # Agent abstractions
│   │   │   ├── mod.rs
│   │   │   ├── memory_agent.rs        # MemoryAgent trait
│   │   │   ├── compressor.rs          # MemoryCompressor
│   │   │   ├── merger.rs              # MemoryMerger
│   │   │   └── forgetter.rs           # MemoryForGetter
│   │   ├── runtime/                   # External runtime adapters
│   │   │   ├── mod.rs
│   │   │   ├── openai_adapter.rs
│   │   │   └── anthropic_adapter.rs
│   │   ├── protocol/                  # Protocol definitions
│   │   │   ├── mod.rs                 # MemoryProtocol types
│   │   │   ├── grpc.rs
│   │   │   ├── mcp.rs                 # MCP (Model Context Protocol)
│   │   │   └── websocket.rs
│   │   ├── hoops/                     # Middleware
│   │   │   ├── mod.rs                 # Hoop re-exports
│   │   │   ├── jwt.rs                 # JWT auth middleware
│   │   │   ├── rate_limit.rs          # Rate limiting
│   │   │   ├── cors.rs                # CORS
│   │   │   ├── enterprise.rs          # Enterprise hooks
│   │   │   ├── enterprise_impl.rs
│   │   │   └── enterprise_hooks_v2.rs
│   │   ├── web/                       # Web framework layer
│   │   │   ├── mod.rs
│   │   │   ├── cors.rs
│   │   │   ├── jwt.rs
│   │   │   └── rate_limit.rs
│   │   ├── integrations/              # External integrations
│   │   │   ├── mod.rs
│   │   │   └── oris.rs
│   │   ├── policy/                   # Policy engine
│   │   │   ├── mod.rs
│   │   │   ├── scheduler.rs
│   │   │   └── cost_model.rs
│   │   ├── distributed/              # Distributed systems
│   │   │   ├── mod.rs
│   │   │   ├── node.rs
│   │   │   ├── consensus.rs
│   │   │   ├── replication.rs
│   │   │   └── sharding.rs
│   │   ├── tenant/                   # Multi-tenancy
│   │   │   ├── mod.rs
│   │   │   ├── context.rs
│   │   │   └── quota.rs
│   │   ├── utils/                    # Utilities
│   │   └── error.rs                  # AppError definition
│   ├── migrations/                   # SQLx migrations
│   ├── examples/                     # Example programs
│   └── Cargo.toml
├── frontend/
│   └── ant-design-pro-template/       # Umi 4 + Ant Design Pro
│       ├── config/
│       │   ├── config.ts             # Umi configuration
│       │   ├── routes.ts             # Route declarations
│       │   ├── proxy.ts              # Dev proxy
│       │   └── defaultSettings.ts    # Layout defaults
│       ├── src/
│       │   ├── app.tsx              # App runtime, layout hooks
│       │   ├── global.tsx           # Global styles
│       │   ├── requestErrorConfig.ts # Error handling
│       │   ├── pages/               # Route-level pages
│       │   │   ├── Dashboard/       # Dashboard overview
│       │   │   ├── MemoryConfig/    # Memory config management
│       │   │   ├── MemoryDetails/   # Memory detail lists
│       │   │   ├── MemoryDecisionTrace/ # Trace viewer
│       │   │   ├── MemoryManagement/   # Memory management
│       │   │   ├── TaskAnalysis/   # Task feature analysis
│       │   │   ├── Performance/     # Performance analytics
│       │   │   ├── ResourceMonitor/ # Resource monitoring
│       │   │   ├── WeightHistory/   # Weight adjustment history
│       │   │   └── user/           # Auth pages
│       │   │       └── login/      # Login page
│       │   ├── services/           # API clients
│       │   │   ├── memory/        # Memory API clients
│       │   │   │   ├── api.ts     # Adaptive selection, analysis, prediction
│       │   │   │   ├── storageApi.ts # STM/LTM storage
│       │   │   │   ├── knowledgeGraphApi.ts
│       │   │   │   ├── multimodalApi.ts
│       │   │   │   ├── index.ts
│       │   │   │   └── typings.d.ts
│       │   │   └── ant-design-pro/ # Auth/demo clients
│       │   ├── components/         # Shared components
│       │   │   ├── Footer/
│       │   │   ├── HeaderDropdown/
│       │   │   └── RightContent/
│       │   ├── utils/             # Frontend utilities
│       │   └── locales/          # i18n
│       └── package.json
└── .planning/codebase/              # Generated mapping docs
```

## Directory Purposes

**`backend/src/axum_routers`:**
- Purpose: Live HTTP surface mounted by `backend/src/main.rs`
- Contains: Self-contained Axum route modules (memory, auth, user, memory_search, memory_storage, knowledge_graph, multimodal)
- Key file: `backend/src/axum_routers/mod.rs` (creates router, OpenAPI doc)

**`backend/src/routers`:**
- Purpose: Richer alternate API composition with nested routes, auth, rate limits
- Contains: memory, agent, auth, user, billing, dashboard, enterprise, knowledge_graph, memory_pool, memory_search, memory_storage, metrics, mcp, multimodal, multi_tenant_router, snapshot, tenant, visualization
- Key file: `backend/src/routers/mod.rs` (composes full API surface)

**`backend/src/services`:**
- Purpose: Business logic and orchestration
- Contains: AdaptiveMemoryScheduler, MemoryOrchestrator, MemoryStorageService, MemorySearchService, EvidenceGraphService, analyzers, monitors, external client wrappers
- Key files: `scheduler.rs`, `memory_orchestrator.rs`, `memory_storage.rs`, `memory_search.rs`, `evidence_graph.rs`

**`backend/src/db`:**
- Purpose: Database bootstrap and repository operations
- Contains: Pool/migration setup, per-domain repositories
- Key files: `mod.rs`, `memory.rs`, `stm.rs`, `ltm.rs`, `kg.rs`, `mm.rs`, `decision_trace.rs`, `evidence_graph.rs`

**`backend/src/kernel`:**
- Purpose: Low-level memory primitives and trait definitions
- Contains: MemoryKernel trait, LayerType enum, MemoryId, MemoryEntry, MemoryContent
- Key files: `traits.rs`, `types.rs`

**`backend/src/models`:**
- Purpose: Share serializable types across HTTP, service, repository layers
- Contains: task, memory, resource, performance, agent model definitions
- Key files: `task.rs`, `memory.rs`, `resource.rs`, `performance.rs`

**`backend/src/layers`:**
- Purpose: Bridge kernel traits with repository implementations
- Contains: StmMemoryLayer, LtmMemoryLayer, KgMemoryLayer, MmMemoryLayer
- Pattern: Implement `MemoryKernel` trait for each memory type

**`backend/src/config`:**
- Purpose: Runtime configuration loading
- Contains: server/db/log/LLM/embedding/Qdrant/Neo4j config modules
- Key files: `mod.rs` (config discovery), `db_config.rs`, `llm_config.rs`

**`frontend/ant-design-pro-template/src/pages`:**
- Purpose: Route-level containers with `index.tsx`
- Contains: Dashboard, MemoryConfig, MemoryDetails, MemoryDecisionTrace, TaskAnalysis, Performance, ResourceMonitor, WeightHistory, user/login

**`frontend/ant-design-pro-template/src/services/memory`:**
- Purpose: Typed API clients for backend services
- Contains: `api.ts` (adaptive selection), `storageApi.ts` (STM/LTM), `knowledgeGraphApi.ts`, `multimodalApi.ts`

## Key File Locations

**Entry Points:**
- `backend/src/main.rs`: Backend process entry, startup wiring, live router bootstrap
- `frontend/ant-design-pro-template/src/app.tsx`: Frontend runtime layout, auth redirect, request base URL
- `frontend/ant-design-pro-template/config/config.ts`: Frontend Umi configuration

**Configuration:**
- `backend/src/config/mod.rs`: Config discovery and global server config
- `backend/config.toml`: Local backend configuration (gitignored)

**Core Logic:**
- `backend/src/services/scheduler.rs`: Adaptive memory selection pipeline
- `backend/src/services/memory_orchestrator.rs`: Explain/dry-run/persist orchestration
- `backend/src/services/memory_storage.rs`: STM/LTM storage workflow
- `backend/src/services/memory_search.rs`: Vector/keyword/hybrid retrieval
- `backend/src/services/evidence_graph.rs`: Decision trace recording with hash chains

**API Surface:**
- `backend/src/axum_routers/memory.rs`: Live memory endpoints including workflow evidence
- `backend/src/axum_routers/mod.rs`: Router creation, OpenAPI doc generation

## Naming Conventions

**Files (Backend):**
- Use `snake_case` for Rust modules: `memory_orchestrator.rs`, `decision_trace.rs`
- Use `snake_case` for route handler modules: `memory_storage.rs`, `knowledge_graph.rs`

**Files (Frontend):**
- Use `camelCase` for TypeScript files: `storageApi.ts`, `knowledgeGraphApi.ts`
- Page folders use `PascalCase`: `MemoryDetails/`, `ResourceMonitor/`
- Page entry is `index.tsx`

**Directories:**
- Backend layers use plural: `services/`, `routers/`, `models/`, `db/`
- Frontend pages use PascalCase: `MemoryConfig/`, `WeightHistory/`

## Where to Add New Code

**New Backend API Endpoint:**
- Live route: `backend/src/axum_routers/<domain>.rs`
- Extended route: `backend/src/routers/<domain>.rs`
- Handler: add function, register route
- OpenAPI: add to `#[openapi]` macro in `axum_routers/mod.rs`

**New Backend Service:**
- Location: `backend/src/services/`
- Follow existing patterns: `scheduler.rs` for orchestration, `*_storage.rs` for storage, `*_search.rs` for retrieval
- Export from `backend/src/services/mod.rs`

**New Backend Repository:**
- Location: `backend/src/db/`
- Follow `sqlx::query` / `sqlx::query_as` patterns
- Support both PostgreSQL and SQLite where needed

**New Frontend Page:**
- Implementation: `frontend/ant-design-pro-template/src/pages/<FeatureName>/index.tsx`
- Route registration: `frontend/ant-design-pro-template/config/routes.ts`
- API integration: `frontend/ant-design-pro-template/src/services/<domain>/`

**New Domain Model:**
- Location: `backend/src/models/`
- Add serde/utoipa derives for API compatibility
- Consider adding to kernel types if core abstraction

## Special Directories

**`frontend/ant-design-pro-template/src/.umi`:**
- Purpose: Generated Umi runtime artifacts
- Generated: Yes (by Umi)
- Committed: Yes

**`backend/migrations`:**
- Purpose: SQLx versioned schema migrations
- Generated: No (manual SQL)
- Committed: Yes

**`backend/examples`:**
- Purpose: Manual verification and sample programs
- Generated: No
- Committed: Yes

**`backend/target` and `frontend/ant-design-pro-template/dist`:**
- Purpose: Build artifacts
- Generated: Yes
- Committed: No (in .gitignore)

## Placement Guidance

- Put startup-only wiring in `backend/src/main.rs`; business logic stays in services
- Put HTTP DTO translation and route nesting in router modules; persistence in `db/`
- Prefer extending existing domain service files before creating new top-level directories
- Keep frontend requests behind service wrappers; pages consume wrappers via `useRequest`
- Treat `axum_routers/` and `routers/` as distinct boundaries - use matching tree for your endpoint

---

*Structure analysis: 2026-03-28*
