# Testing Patterns

**Analysis Date:** 2026-03-28

## Test Framework

**Backend (Rust):**
- Rust built-in test harness with `#[test]` and `#[tokio::test]`
- Test modules marked with `#[cfg(test)]`
- SQLite in-memory database for tests
- Integration tests in `backend/tests/*.rs`

**Frontend (TypeScript/React):**
- Jest via `@umijs/max/test` (configured in `jest.config.ts`)
- React Testing Library for DOM interaction
- Snapshot testing via `toMatchSnapshot()`
- Mock server via `@@/requestRecordMock`

## Test File Organization

**Backend:**
```
backend/src/
  <module>.rs           # Inline #[cfg(test)] mod tests
backend/tests/
  hash_chain.rs        # Integration tests
  evidence_api.rs       # Integration tests
  evidence_graph.rs    # Integration tests
  snapshot_export.rs   # Integration tests
```

**Frontend:**
```
frontend/ant-design-pro-template/
  src/pages/user/login/
    login.test.tsx              # Page test
    __snapshots__/
      login.test.tsx.snap       # Snapshot
  tests/
    setupTests.jsx              # Global mocks
```

## Test Configuration

**Backend Cargo.toml:**
- No explicit test dependencies; uses standard library `#[test]`
- `#[tokio::test]` for async tests (tokio already in dependencies)

**Frontend jest.config.ts:**
```typescript
export default async (): Promise<any> => {
  const config = await configUmiAlias({
    ...createConfig({ target: 'browser' }),
  });
  return {
    ...config,
    testEnvironmentOptions: { url: 'http://localhost:8000' },
    setupFiles: [...(config.setupFiles || []), './tests/setupTests.jsx'],
  };
};
```

## Run Commands

**Backend:**
```bash
cd backend && cargo test                    # Run all tests
cd backend && cargo test test_name          # Run specific test
cd backend && cargo test --lib              # Unit tests only
cd backend && cargo test --test integration # Specific integration test
```

**Frontend:**
```bash
cd frontend/ant-design-pro-template && npm test              # Run Jest
cd frontend/ant-design-pro-template && npm run test:coverage # Coverage report
cd frontend/ant-design-pro-template && npm run test:update   # Update snapshots
```

## Test Structure

**Rust Unit Test:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assess_task_complexity() {
        let analyzer = TaskCharacteristicAnalyzer::new();
        let result = analyzer.analyze(&task_context);
        assert!(result.is_ok());
    }
}
```

**Rust Async Test:**
```rust
#[tokio::test]
async fn verify_chain_accepts_the_unmodified_persisted_nodes() {
    let _guard = test_guard();
    init_test_db().await;
    let trace = sample_trace("workflow-hash-chain-valid").await;
    let recorded = record_decision_trace_as_evidence(&trace)
        .await
        .expect("persist evidence graph");
    // ...
}
```

**Frontend Test:**
```tsx
describe('Login Page', () => {
  beforeAll(async () => {
    server = await startMock({ port: 8000, scene: 'login' });
  });

  it('should show login form', async () => {
    const rootContainer = render(
      <TestBrowser historyRef={historyRef} location={{ pathname: '/user/login' }} />
    );
    expect(rootContainer.asFragment()).toMatchSnapshot();
  });
});
```

## Mocking

**Backend:**
- Minimal mocking; use concrete structs directly
- Test database uses SQLite in-memory with `OnceLock` singletons for shared state
- Global test lock to prevent parallel DB access

```rust
static DB_PATH: OnceLock<String> = OnceLock::new();
static INIT_DB: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
static TEST_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap()
}
```

**Frontend:**
- Browser APIs mocked in `tests/setupTests.jsx`: `localStorage`, `matchMedia`, `Worker`, `ResizeObserver`
- HTTP mocking via `startMock` with scene files

```tsx
// tests/setupTests.jsx
global.localStorage = localStorageMock;
global.ResizeObserver = class ResizeObserver { ... };
```

## Fixtures and Factories

**Backend:**
Private helper functions in test modules:
```rust
fn sample_task_context(task_id: &str) -> TaskContext {
    TaskContext {
        task_id: task_id.to_string(),
        task_type: TaskType::Task,
        complexity: 0.7,
        // ...
    }
}
```

**Frontend:**
- Inline fixture data in test files
- No centralized fixture factory pattern

## Coverage

**Requirements:** None enforced (no `coverageThreshold` configured)

**Backend:**
- 31 `#[cfg(test)]` modules found in `backend/src/`
- Run `cargo test` for basic coverage

**Frontend:**
- `npm run test:coverage` generates reports but no thresholds
- Minimal coverage: only `login.test.tsx` with snapshots

**View Coverage:**
```bash
cd frontend/ant-design-pro-template && npm run test:coverage
```

## Test Types

**Unit Tests:**
- Dominant pattern in backend
- Modules tested: `analyzer.rs`, `predictor.rs`, `scheduler.rs`, `monitor.rs`, `cost_model.rs`

**Integration Tests:**
- Backend: `backend/tests/*.rs` with real HTTP requests via `tower::ServiceExt::oneshot`
- Frontend: Login test uses `TestBrowser` with mock server

**E2E Tests:**
- Not detected; no Playwright, Cypress, or similar

## CI Pipeline

**Backend CI (`.github/workflows/backend-ci.yml`):**
1. Code quality: `cargo fmt --check`, `cargo clippy`, `cargo doc`
2. Security: `rustsec/audit-check`
3. Build and test: `cargo build`, `cargo test`

**Frontend CI (`.github/workflows/frontend-ci.yml`):**
1. Code quality: `npm run lint`, `tsc --noEmit`
2. Build: `npm run build`

## Common Patterns

**Async Testing (Backend):**
```rust
#[tokio::test]
async fn test_function() {
    let result = async_function().await;
    assert!(result.is_ok());
}
```

**Error Path Testing:**
```rust
#[tokio::test]
async fn verify_chain_rejects_tampering() {
    let result = verify_chain(...).await;
    assert!(!result.verified);
    assert!(verification.violations.iter().any(|item| item.contains("prev_hash")));
}
```

**HTTP Handler Testing:**
```rust
let app = backend::axum_routers::create_router();
let response = app
    .oneshot(
        Request::builder()
            .uri(format!("/api/v1/workflows/{}/evidence", task_id))
            .body(Body::empty())
            .expect("build request"),
    )
    .await
    .expect("serve request");
assert_eq!(response.status(), StatusCode::OK);
```

## Test Gaps

- Frontend business pages (`MemoryManagement`, `MemoryConfig`, `MemoryDecisionTrace`, `Performance`, `ResourceMonitor`, `TaskAnalysis`, `WeightHistory`) lack tests
- Frontend `requestErrorConfig.ts` untested
- Frontend `src/services/memory/*.ts` not directly covered
- Backend DB repository methods lightly tested
- No E2E test framework in use

---

*Testing analysis: 2026-03-28*
