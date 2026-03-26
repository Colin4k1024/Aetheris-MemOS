# Coding Conventions

**Analysis Date:** 2026-03-26

## Naming Patterns

**Files:**
- Rust modules use snake_case file names such as `backend/src/services/memory_orchestrator.rs`, `backend/src/db/memory.rs`, and `backend/src/routers/memory.rs`.
- React pages and feature folders use PascalCase directory names with `index.tsx` entry files, such as `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx` and `frontend/ant-design-pro-template/src/pages/ResourceMonitor/index.tsx`.
- Frontend service client files use camelCase or domain-specific suffixes, such as `frontend/ant-design-pro-template/src/services/memory/storageApi.ts` and `frontend/ant-design-pro-template/src/services/memory/knowledgeGraphApi.ts`.

**Functions:**
- Rust free functions and methods use snake_case: `select_memory_config` in `backend/src/routers/memory.rs`, `adaptive_memory_selection` in `backend/src/services/scheduler.rs`, and `row_to_memory_config` in `backend/src/db/memory.rs`.
- TypeScript functions use camelCase and usually start with verbs for API calls and handlers: `listMemoryConfigs` in `frontend/ant-design-pro-template/src/services/memory/api.ts`, `getDecisionTraces` in `frontend/ant-design-pro-template/src/services/memory/storageApi.ts`, and `handleSubmit` in `frontend/ant-design-pro-template/src/pages/user/login/index.tsx`.

**Variables:**
- Rust local bindings use snake_case and keep domain terms intact, for example `resource_constraints`, `performance_prediction`, and `adjustment_reasons` in `backend/src/services/scheduler.rs`.
- Frontend local state and refs use camelCase with explicit intent, for example `createModalVisible`, `editModalVisible`, and `currentRecord` in `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx`.

**Types:**
- Rust structs and enums use PascalCase, such as `AdaptiveMemoryScheduler`, `TaskCharacteristicAnalyzer`, `AppError`, and `MemoryConfigRepository` in `backend/src/services/scheduler.rs`, `backend/src/services/analyzer.rs`, `backend/src/error.rs`, and `backend/src/db/memory.rs`.
- TypeScript component and API types use PascalCase and `API.*` namespaces, such as `MemoryManagement`, `API.MemoryConfigRow`, and `API.SelectMemoryRequest` in `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx` and `frontend/ant-design-pro-template/src/services/memory/api.ts`.

## Code Style

**Formatting:**
- Frontend formatting is driven by Biome plus `.editorconfig`: `frontend/ant-design-pro-template/biome.json` sets spaces and single quotes, and `frontend/ant-design-pro-template/.editorconfig` sets 2-space indentation and LF line endings.
- TypeScript is strict by default in `frontend/ant-design-pro-template/tsconfig.json`, including `strict`, `noImplicitReturns`, and `forceConsistentCasingInFileNames`.
- No repo-level `rustfmt.toml` or `clippy.toml` was detected, so backend style follows standard Rust formatting conventions as seen in `backend/src/main.rs` and `backend/src/error.rs`.

**Linting:**
- Frontend linting runs through `npm run lint` in `frontend/ant-design-pro-template/package.json`, which combines `npx @biomejs/biome lint` with `tsc --noEmit`.
- Biome intentionally excludes generated and API-heavy areas including `src/services`, `mock`, `dist`, and `.umi` in `frontend/ant-design-pro-template/biome.json`.
- Generated frontend service clients start with `// @ts-ignore` and `/* eslint-disable */` in `frontend/ant-design-pro-template/src/services/memory/api.ts` and `frontend/ant-design-pro-template/src/services/memory/storageApi.ts`; preserve that pattern for generated files rather than hand-formatting them.

## Import Organization

**Order:**
1. External packages first, for example `axum`, `serde`, and `tracing` in `backend/src/main.rs`, and `@ant-design/pro-components`, `@umijs/max`, and `antd` in `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx`.
2. Standard library imports appear near the top when needed, such as `std::sync::Arc` in `backend/src/routers/memory.rs`.
3. Internal crate or alias imports come last, such as `crate::db::*`, `crate::services::*`, and `@/services/memory/api` in `backend/src/routers/memory.rs` and `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx`.

**Path Aliases:**
- Frontend shared code uses `@/*` for `src/*` and `@@/*` for generated Umi artifacts as configured in `frontend/ant-design-pro-template/tsconfig.json`.
- Prefer `@/services/...` and `@/components/...` in handwritten frontend code, as shown in `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx` and `frontend/ant-design-pro-template/src/pages/user/login/index.tsx`.

## Error Handling

**Patterns:**
- Backend HTTP-facing code should return `crate::AppError` or the `JsonResult<T>` alias from `backend/src/main.rs`; router handlers in `backend/src/routers/memory.rs` follow this consistently.
- Centralize HTTP status mapping in `backend/src/error.rs`. `AppError` implements `IntoResponse`, logs by severity, and returns a stable `{ code, message, error }` JSON body.
- Services usually propagate errors with `?` and convert serialization failures explicitly, for example `persist_trace_record` and `list_decision_traces` in `backend/src/services/memory_orchestrator.rs`.
- Frontend request failures are normalized in `frontend/ant-design-pro-template/src/requestErrorConfig.ts` through `errorThrower`, `errorHandler`, and request/response interceptors. UI code typically shows `message.error(...)` on local action failure, as in `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx`.

## Logging

**Framework:** `tracing` on the backend, browser `console` plus Ant Design `message`/`notification` on the frontend.

**Patterns:**
- Backend logs use structured macros with fields when possible, such as `info!(config_id = %config_id, task_id = %task_context.task_id, ...)` in `backend/src/services/scheduler.rs`.
- Frontend runtime code still uses direct `console.log`, `console.warn`, and `console.error` in `frontend/ant-design-pro-template/src/app.tsx` and `frontend/ant-design-pro-template/src/pages/user/login/index.tsx`; follow the existing style inside runtime hooks, but prefer user-facing `message`/`notification` for visible failures.

## Comments

**When to Comment:**
- Backend comments are used to label stepwise pipelines and operational intent, such as the numbered scheduler flow in `backend/src/services/scheduler.rs` and startup sequencing in `backend/src/main.rs`.
- Frontend comments mostly explain runtime exceptions or removed template behavior, such as auth redirect notes in `frontend/ant-design-pro-template/src/app.tsx` and test environment shims in `frontend/ant-design-pro-template/tests/setupTests.jsx`.

**JSDoc/TSDoc:**
- Generated service clients use concise endpoint doc comments directly above exported functions, for example in `frontend/ant-design-pro-template/src/services/memory/api.ts` and `frontend/ant-design-pro-template/src/services/memory/storageApi.ts`.
- Rust code relies more on inline comments than doc comments for internal logic.

## Function Design

**Size:**
- Backend service methods can be large orchestration units. `adaptive_memory_selection` in `backend/src/services/scheduler.rs` is the main example; keep heavy sequencing in services, not in low-level repositories.
- Frontend page components often combine table definitions, modal state, and submit handlers in one file, as in `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx`.

**Parameters:**
- Backend APIs prefer typed structs for request and query payloads, such as `SelectMemoryRequest`, `AnalyzeTaskRequest`, and `ListTracesQuery` in `backend/src/routers/memory.rs`.
- Frontend service functions accept typed `body`, `params`, and loose `options` objects, consistently forwarding `...(options || {})` in `frontend/ant-design-pro-template/src/services/memory/api.ts`.

**Return Values:**
- Backend repositories return `Result<T, AppError>` or `Result<Option<T>, AppError>`, seen in `backend/src/db/memory.rs`.
- Frontend service helpers return `request<T>(...)` promises and let callers shape UX around the resolved data, as in `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx`.

## Module Design

**Exports:**
- Backend groups modules by responsibility under `routers/`, `services/`, `db/`, `models/`, and related domains, declared from `backend/src/main.rs`.
- Frontend uses barrel exports sparingly. `frontend/ant-design-pro-template/src/services/memory/index.ts` re-exports memory client modules, while most page folders export through `index.tsx`.

**Barrel Files:**
- Use barrel files only where the repository already does so, such as `frontend/ant-design-pro-template/src/services/memory/index.ts` and `frontend/ant-design-pro-template/src/components/index.ts`.

## Layering Rules

- Put HTTP extraction, DTOs, and route-level assembly in `backend/src/routers/*.rs` or `backend/src/axum_routers/*.rs`. `backend/src/routers/memory.rs` shows the preferred handler shape with `Json`, `Query`, and typed response structs.
- Put reusable business logic and orchestration in `backend/src/services/*.rs`. `backend/src/services/memory_orchestrator.rs`, `backend/src/services/scheduler.rs`, and `backend/src/services/analyzer.rs` hold the system behavior that handlers call.
- Put persistence and row-mapping code in `backend/src/db/*.rs`. `backend/src/db/memory.rs` is the pattern for SQL access, row structs, and conversion helpers.
- Frontend pages in `frontend/ant-design-pro-template/src/pages/*` should call service clients from `frontend/ant-design-pro-template/src/services/*` instead of issuing raw requests inline. `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx` follows this.
- The layering is pragmatic rather than strict: router modules sometimes import repositories directly in addition to services, as `backend/src/routers/memory.rs` does. Match the local file’s pattern instead of forcing a new abstraction style.

## Consistency Notes

- Backend naming is highly consistent: snake_case functions, PascalCase types, typed DTOs, and `AppError` propagation appear throughout `backend/src/`.
- Frontend handwritten code is consistent about alias imports, `React.FC` components, and Ant Design Pro patterns, but generated client files are intentionally exempt from normal linting and formatting in `frontend/ant-design-pro-template/biome.json`.
- Frontend request/response naming is mixed by necessity: page components use camelCase props and state, while API payloads preserve backend field names or `serde`-driven names. `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx` explicitly maps UI fields to API fields before submission.

---

*Convention analysis: 2026-03-26*
