# Codebase Structure

**Analysis Date:** 2026-03-26

## Directory Layout

```text
adaptive-memory-system/
├── backend/                         # Rust backend service, DB migrations, scripts, assets
│   ├── src/
│   │   ├── axum_routers/            # Live Axum router modules booted by `main.rs`
│   │   ├── routers/                 # Larger alternate router tree with richer API composition
│   │   ├── services/                # Business logic and orchestration
│   │   ├── db/                      # SQLx repositories and DB initialization
│   │   ├── models/                  # Shared API/domain models
│   │   ├── config/                  # Runtime configuration loading
│   │   ├── agent/                   # Agent-memory runtime integration abstractions
│   │   ├── kernel/                  # Low-level memory kernel traits and types
│   │   ├── runtime/                 # External runtime adapters
│   │   └── main.rs                  # Backend entry point
│   ├── migrations/                  # SQL migrations
│   ├── examples/                    # Example and manual test programs
│   └── views/                       # HTML templates for login/embedded pages
├── frontend/
│   └── ant-design-pro-template/     # Umi 4 + Ant Design Pro frontend
│       ├── config/                  # Umi config, routes, proxy, settings
│       ├── src/pages/               # Route-level pages
│       ├── src/services/            # API clients grouped by backend domain
│       ├── src/components/          # Shared layout/UI components
│       └── src/app.tsx              # Runtime app and layout hooks
└── .planning/codebase/              # Generated codebase mapping docs
```

## Directory Purposes

**`backend/src/axum_routers`:**
- Purpose: Keep the live HTTP surface mounted by `backend/src/main.rs`.
- Contains: self-contained Axum route modules such as `backend/src/axum_routers/memory.rs`, `backend/src/axum_routers/memory_search.rs`, and `backend/src/axum_routers/auth.rs`
- Key files: `backend/src/axum_routers/mod.rs`

**`backend/src/routers`:**
- Purpose: Keep the richer API composition that wires auth, rate limits, nested route groups, and more complete handler logic.
- Contains: domain router modules for memory, agent, enterprise, billing, metrics, tenant, and visualization
- Key files: `backend/src/routers/mod.rs`, `backend/src/routers/memory.rs`, `backend/src/routers/agent.rs`

**`backend/src/services`:**
- Purpose: Put workflow logic here. New schedulers, storage/search logic, orchestration, and daemons belong here.
- Contains: orchestrators, analyzers, resource monitors, vector/LLM integration wrappers, and background workers
- Key files: `backend/src/services/scheduler.rs`, `backend/src/services/memory_orchestrator.rs`, `backend/src/services/memory_storage.rs`, `backend/src/services/memory_search.rs`

**`backend/src/db`:**
- Purpose: Centralize DB bootstrap and repository operations.
- Contains: `mod.rs` for pool/migration setup plus per-table/per-domain repositories
- Key files: `backend/src/db/mod.rs`, `backend/src/db/memory.rs`, `backend/src/db/stm.rs`, `backend/src/db/ltm.rs`

**`backend/src/models`:**
- Purpose: Share serializable types across HTTP, service, and repository layers.
- Contains: task, memory, resource, performance, and agent model definitions
- Key files: `backend/src/models/mod.rs`, `backend/src/models/task.rs`, `backend/src/models/memory.rs`

**`backend/src/config`:**
- Purpose: Keep all runtime configuration loaders and config structs.
- Contains: server/db/log/LLM/embedding/Qdrant/Neo4j config modules
- Key files: `backend/src/config/mod.rs`, `backend/src/config/db_config.rs`, `backend/src/config/llm_config.rs`

**`backend/src/agent`, `backend/src/kernel`, `backend/src/runtime`, `backend/src/policy`:**
- Purpose: House broader platform abstractions that are not the main HTTP request path.
- Contains: kernel traits, memory-agent interfaces, runtime adapters, and policy engine code
- Key files: `backend/src/agent/memory_agent.rs`, `backend/src/kernel/traits.rs`, `backend/src/runtime/mod.rs`, `backend/src/policy/mod.rs`

**`frontend/ant-design-pro-template/config`:**
- Purpose: Define routing, build/runtime config, proxying, and default layout settings.
- Contains: Umi config and route declarations
- Key files: `frontend/ant-design-pro-template/config/config.ts`, `frontend/ant-design-pro-template/config/routes.ts`, `frontend/ant-design-pro-template/config/proxy.ts`

**`frontend/ant-design-pro-template/src/pages`:**
- Purpose: Put route-level containers here. Each feature page uses its own folder with `index.tsx`.
- Contains: dashboard, task analysis, memory config, memory details, monitoring, and auth pages
- Key files: `frontend/ant-design-pro-template/src/pages/Dashboard/index.tsx`, `frontend/ant-design-pro-template/src/pages/MemoryConfig/index.tsx`, `frontend/ant-design-pro-template/src/pages/MemoryDetails/index.tsx`

**`frontend/ant-design-pro-template/src/services`:**
- Purpose: Keep request wrappers by API domain.
- Contains: memory clients, Ant Design Pro auth/demo clients, and generated swagger examples
- Key files: `frontend/ant-design-pro-template/src/services/memory/api.ts`, `frontend/ant-design-pro-template/src/services/memory/storageApi.ts`, `frontend/ant-design-pro-template/src/services/memory/index.ts`

## Key File Locations

**Entry Points:**
- `backend/src/main.rs`: backend process entry, startup wiring, and live router bootstrap
- `frontend/ant-design-pro-template/src/app.tsx`: frontend runtime layout, auth redirect, and request base URL
- `frontend/ant-design-pro-template/config/config.ts`: frontend app-level Umi configuration

**Configuration:**
- `backend/src/config/mod.rs`: config discovery and global server config
- `backend/config.toml`: local backend configuration file present in repo
- `frontend/ant-design-pro-template/config/routes.ts`: route-to-page mapping

**Core Logic:**
- `backend/src/services/scheduler.rs`: adaptive memory selection pipeline
- `backend/src/services/memory_orchestrator.rs`: explain/dry-run/persist orchestration around selection
- `backend/src/services/memory_storage.rs`: STM/LTM storage workflow
- `backend/src/services/memory_search.rs`: vector, keyword, and hybrid retrieval workflow

**Testing:**
- `backend/src/main.rs`: basic router smoke test module
- `backend/src/services/analyzer.rs`: focused unit tests for analyzer behavior
- `backend/src/services/predictor.rs`: focused unit tests for prediction logic
- `backend/examples/`: manual and exploratory backend examples

## Naming Conventions

**Files:**
- Use Rust snake_case module files in the backend, such as `backend/src/services/memory_orchestrator.rs` and `backend/src/db/decision_trace.rs`.
- Use page folders with `index.tsx` in the frontend, such as `frontend/ant-design-pro-template/src/pages/MemoryDecisionTrace/index.tsx`.
- Use TypeScript API wrapper files named by domain or feature, such as `frontend/ant-design-pro-template/src/services/memory/storageApi.ts`.

**Directories:**
- Use plural feature directories for backend layers: `backend/src/services`, `backend/src/routers`, `backend/src/models`, `backend/src/db`.
- Use PascalCase page directories for frontend route containers: `frontend/ant-design-pro-template/src/pages/Dashboard`, `frontend/ant-design-pro-template/src/pages/ResourceMonitor`.

## Where to Add New Code

**New Backend API Feature:**
- Primary code: add the handler to the router tree you are extending. For live Axum boot, use `backend/src/axum_routers/`. For the richer nested API surface, use `backend/src/routers/`.
- Tests: add unit tests near the touched service or handler module, following the `#[cfg(test)]` pattern in `backend/src/services/analyzer.rs` and `backend/src/services/predictor.rs`.

**New Backend Business Logic:**
- Implementation: `backend/src/services/`
- Persistence support: `backend/src/db/`
- Shared request/response/domain types: `backend/src/models/`

**New Frontend Page:**
- Implementation: create `frontend/ant-design-pro-template/src/pages/<FeatureName>/index.tsx`
- Route registration: add an entry in `frontend/ant-design-pro-template/config/routes.ts`
- API integration: add or extend clients in `frontend/ant-design-pro-template/src/services/<domain>/`

**Utilities:**
- Shared backend helpers: `backend/src/utils/`
- Shared frontend helpers: `frontend/ant-design-pro-template/src/utils/`

## Special Directories

**`frontend/ant-design-pro-template/src/.umi`:**
- Purpose: generated Umi runtime artifacts
- Generated: Yes
- Committed: Yes in current state

**`backend/migrations`:**
- Purpose: versioned schema changes for SQLx-backed stores
- Generated: No
- Committed: Yes

**`backend/examples`:**
- Purpose: manual verification and sample usage programs
- Generated: No
- Committed: Yes

**`backend/target` and `frontend/ant-design-pro-template/dist`:**
- Purpose: build artifacts
- Generated: Yes
- Committed: Yes in current state; do not place new source files here

## Placement Guidance

- Put startup-only wiring in `backend/src/main.rs`; do not place business logic there.
- Put HTTP DTO translation and route nesting in router modules; keep persistence and orchestration in `backend/src/services/` and `backend/src/db/`.
- Prefer extending existing domain service files before creating new top-level backend directories. For example, memory retrieval belongs beside `backend/src/services/memory_search.rs`, not in `backend/src/utils/`.
- Keep frontend requests behind service wrappers in `frontend/ant-design-pro-template/src/services/`; pages such as `frontend/ant-design-pro-template/src/pages/Dashboard/index.tsx` already consume those wrappers via `useRequest`.
- Treat `backend/src/axum_routers/` and `backend/src/routers/` as distinct module boundaries. If you add a new endpoint, place it in the tree that matches the actual server path you need to expose.

---

*Structure analysis: 2026-03-26*
