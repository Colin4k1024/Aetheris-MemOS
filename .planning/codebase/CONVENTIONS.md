# Coding Conventions

**Analysis Date:** 2026-03-28

## Naming Patterns

**Files:**
- Rust modules use snake_case: `backend/src/services/memory_orchestrator.rs`, `backend/src/db/memory.rs`
- React page directories use PascalCase with `index.tsx`: `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx`
- Frontend service clients use camelCase or domain suffixes: `frontend/ant-design-pro-template/src/services/memory/storageApi.ts`

**Functions:**
- Rust free functions and methods use snake_case: `select_memory_config` in `backend/src/routers/memory.rs`, `adaptive_memory_selection` in `backend/src/services/scheduler.rs`
- TypeScript functions use camelCase with verb prefixes: `listMemoryConfigs`, `getDecisionTraces`, `handleSubmit`

**Variables:**
- Rust local bindings use snake_case: `resource_constraints`, `performance_prediction`, `adjustment_reasons`
- Frontend state uses camelCase: `createModalVisible`, `currentRecord`

**Types:**
- Rust structs/enums use PascalCase: `AdaptiveMemoryScheduler`, `TaskCharacteristicAnalyzer`, `AppError`
- TypeScript types use PascalCase with `API.*` namespaces: `API.MemoryConfigRow`, `API.SelectMemoryRequest`

## Code Style

**Formatting:**
- Backend: `cargo fmt` (rustfmt with 4-space indent, 100 char width)
- Frontend: Biome with spaces, single quotes, `jsxRuntime: reactClassic` (configured in `frontend/ant-design-pro-template/biome.json`)

**Linting:**
- Backend: `cargo clippy` in CI (`backend/.github/workflows/backend-ci.yml`)
- Frontend: `npm run lint` combines `biome lint` + `tsc --noEmit`
- Generated service clients are excluded from Biome linting (`frontend/ant-design-pro-template/biome.json` excludes `src/services/`)

**TypeScript Configuration:**
- Strict mode enabled in `frontend/ant-design-pro-template/tsconfig.json`
- `noImplicitReturns`, `forceConsistentCasingInFileNames` enforced

## Import Organization

**Backend (Rust):**
```rust
// Order: external crates -> std library -> internal crate modules
use axum::extract::{Path, Query};
use axum::Json;
use std::sync::Arc;

use crate::db::{memory::MemoryConfigRepository, weights::WeightHistoryRepository};
use crate::models::*;
use crate::services::*;
```

**Frontend (TypeScript):**
```typescript
// Order: external packages -> internal aliases -> relative imports
import React from 'react';
import { ProCard } from '@ant-design/pro-components';
import { history } from '@umijs/max';
import { getMemoryConfigs } from '@/services/memory/api';
```

**Path Aliases:**
- Frontend: `@/*` maps to `src/*`, `@@/*` maps to generated Umi artifacts

## Error Handling

**Backend Patterns:**

Use `AppError` from `backend/src/error.rs` for HTTP-facing code:

```rust
#[derive(Error, Debug)]
pub enum AppError {
    #[error("not found: `{0}`")]
    NotFound(String),
    #[error("bad request: `{0}`")]
    BadRequest(String),
    #[error("anyhow error:`{0}`")]
    Anyhow(#[from] anyhow::Error),
    // ...
}
```

- `AppError` implements `IntoResponse` and returns `{ code, message, error }` JSON
- Services propagate errors with `?` operator
- Centralize HTTP status mapping in `error.rs`

**Frontend Patterns:**
- `requestErrorConfig.ts` normalizes errors via `errorThrower` and `errorHandler`
- UI shows `message.error(...)` for action failures

## API Design

**Backend (Axum):**

```rust
// Handler signature pattern
pub async fn select_memory_config(
    Json(request): Json<SelectMemoryRequest>,
) -> JsonResult<SelectMemoryResponse>
```

- Use typed extractors: `Json<T>`, `Query<T>`, `Path<T>`, `Extension<T>`
- Define request/response structs with `#[derive(Deserialize, ToSchema)]` / `#[derive(Serialize, ToSchema)]`
- Use `utoipa` for OpenAPI docs

**DTO Naming:**
- Request: `*Request` suffix (e.g., `SelectMemoryRequest`)
- Response: `*Response` suffix (e.g., `SelectMemoryResponse`)
- Query params: `*Query` suffix

## Logging

**Backend:** `tracing` crate with structured macros
```rust
tracing::info!(config_id = %config_id, task_id = %task_id, "selecting memory config");
tracing::error!(msg = %msg, "internal error");
```

**Frontend:** Browser `console` + Ant Design `message`/`notification`
```typescript
message.error('Failed to load memory configs');
```

## Comments

**When to Comment:**
- Label stepwise pipelines: numbered scheduler flow in `scheduler.rs`
- Explain non-obvious business logic
- Document `unsafe` blocks with `// SAFETY:`

**Rust Doc Comments:**
- Use `///` for public API documentation
- Internal code relies on inline comments over doc comments

**TypeScript JSDoc:**
- Generated service clients use endpoint doc comments
- handwritten code rarely uses JSDoc

## Function Design

**Size:**
- Backend service methods can be large orchestration units
- Frontend page components often combine table, modal state, and handlers in one file

**Parameters:**
- Backend: typed structs for request payloads
- Frontend: typed `body`, `params`, and `options` objects

**Return Values:**
- Backend repositories: `Result<T, AppError>` or `Result<Option<T>, AppError>`
- Frontend services: return `request<T>(...)` promises

## Module Design

**Exports:**
- Backend uses glob re-exports in `services/mod.rs`: `pub use analyzer::*;`
- Frontend barrel files: `src/services/memory/index.ts`

**Layering (Backend):**
```
backend/src/routers/*.rs    # HTTP handlers, DTOs, route assembly
backend/src/services/*.rs   # Business logic, orchestration
backend/src/db/*.rs         # Persistence, row-mapping
backend/src/models/*.rs     # Domain types
```

**Layering (Frontend):**
```
src/pages/*/index.tsx      # Page components
src/services/*/            # API clients
src/components/*/          # Shared components
```

## Git Workflow

Documented in `CONTRIBUTING.md`:
1. Fork and create branch from `main` or `dev`
2. Open/link issue for large changes
3. Implement with focused commits
4. Run `cargo test` and `npm test`
5. Open PR with clear title and description

**CI Checks (`.github/workflows/`):**
- Backend CI: `cargo fmt --check`, `cargo clippy`, `cargo test`, security audit
- Frontend CI: `npm run lint`, `tsc --noEmit`, `npm run build`

---

*Convention analysis: 2026-03-28*
