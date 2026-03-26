# Testing Patterns

**Analysis Date:** 2026-03-26

## Test Framework

**Runner:**
- Backend uses Rust’s built-in test harness with `#[test]` and `#[tokio::test]` embedded directly in source files such as `backend/src/services/scheduler.rs`, `backend/src/services/memory_orchestrator.rs`, and `backend/src/db/memory.rs`.
- Frontend uses Jest via `frontend/ant-design-pro-template/jest.config.ts`, built on `@umijs/max/test`.

**Assertion Library:**
- Backend uses standard Rust assertions like `assert_eq!`, `assert!`, and error expectation patterns in `backend/src/services/scheduler.rs` and `backend/src/services/memory_orchestrator.rs`.
- Frontend uses Jest assertions plus React Testing Library in `frontend/ant-design-pro-template/src/pages/user/login/login.test.tsx`.

**Run Commands:**
```bash
cd backend && cargo test                   # Run backend test suite
cd backend && cargo test test_hello_world  # Run a specific backend test
cd backend && cargo run --example test_memory_search  # Run live integration example against a running server
cd frontend/ant-design-pro-template && npm test       # Run Jest tests
cd frontend/ant-design-pro-template && npm run test:coverage  # Generate frontend coverage report
cd frontend/ant-design-pro-template && npm run test:update    # Refresh Jest snapshots
```

## Test File Organization

**Location:**
- Backend tests are primarily co-located inside production modules under `backend/src/**` behind `#[cfg(test)]`.
- Backend also has one executable integration-style example in `backend/examples/test_memory_search.rs`; it is not a standard `tests/` integration test crate.
- Frontend test code is mostly co-located with the page under test: `frontend/ant-design-pro-template/src/pages/user/login/login.test.tsx`.
- Frontend shared setup lives in `frontend/ant-design-pro-template/tests/setupTests.jsx`.

**Naming:**
- Rust inline modules are named `mod tests` and keep scenario-style function names like `test_apply_preferences_disables_mm_and_kg` in `backend/src/services/scheduler.rs`.
- Frontend uses `*.test.tsx` and snapshot files under `__snapshots__`, as seen in `frontend/ant-design-pro-template/src/pages/user/login/__snapshots__/login.test.tsx.snap`.

**Structure:**
```text
backend/src/<module>.rs          # Production code with #[cfg(test)] mod tests
backend/examples/*.rs            # Manual or live integration exercises
frontend/.../src/pages/**/login.test.tsx
frontend/.../tests/setupTests.jsx
frontend/.../src/pages/**/__snapshots__/*.snap
```

## Test Structure

**Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assess_task_complexity() {
        // arrange + act + assert inline
    }
}
```

```tsx
describe('Login Page', () => {
  beforeAll(async () => {
    server = await startMock({ port: 8000, scene: 'login' });
  });

  it('should show login form', async () => {
    const rootContainer = render(<TestBrowser ... />);
    expect(rootContainer.asFragment()).toMatchSnapshot();
  });
});
```

**Patterns:**
- Backend tests are table-light and direct: build a struct, call a function, assert fields or bounds. Examples: `backend/src/services/analyzer.rs`, `backend/src/services/scheduler.rs`, and `backend/src/db/memory.rs`.
- Async backend tests use `#[tokio::test]` where the production API is async, as in `backend/src/main.rs`, `backend/src/services/memory_orchestrator.rs`, and `backend/src/hoops/rate_limit.rs`.
- Frontend tests render route-aware UI through `TestBrowser` from generated Umi test helpers and interact with the DOM using React Testing Library, as in `frontend/ant-design-pro-template/src/pages/user/login/login.test.tsx`.

## Mocking

**Framework:** Jest mocks plus Umi request-record mock server on the frontend; minimal mocking on the backend.

**Patterns:**
```tsx
server = await startMock({
  port: 8000,
  scene: 'login',
});
```

```jsx
global.localStorage = localStorageMock;
global.ResizeObserver = class ResizeObserver { ... };
```

**What to Mock:**
- Frontend browser APIs and layout dependencies are mocked in `frontend/ant-design-pro-template/tests/setupTests.jsx`, including `localStorage`, `matchMedia`, `Worker`, `URL.createObjectURL`, and `ResizeObserver`.
- Mock HTTP flows with `startMock` when testing template-style pages that depend on Umi request scenes, as in `frontend/ant-design-pro-template/src/pages/user/login/login.test.tsx`.

**What NOT to Mock:**
- Backend unit tests usually avoid mocking internal collaborators and instead instantiate concrete structs directly. `backend/src/services/scheduler.rs` and `backend/src/services/analyzer.rs` test real logic with plain data.
- Do not assume generated frontend service clients are independently tested; they are thin wrappers and currently rely on page-level usage tests instead.

## Fixtures and Factories

**Test Data:**
```rust
fn sample_task_context() -> TaskContext {
    TaskContext { ... }
}
```

```tsx
const historyRef = React.createRef<any>();
render(<TestBrowser historyRef={historyRef} location={{ pathname: '/user/login' }} />);
```

**Location:**
- Backend fixture helpers are usually private functions inside the same test module, for example `sample_task_context`, `base_constraints`, and `default_preferences` in `backend/src/services/memory_orchestrator.rs`.
- Frontend does not have a central fixtures or factories directory; setup is embedded in `frontend/ant-design-pro-template/src/pages/user/login/login.test.tsx` and `frontend/ant-design-pro-template/tests/setupTests.jsx`.

## Coverage

**Requirements:** None enforced.
- `frontend/ant-design-pro-template/package.json` exposes `test:coverage`, but `frontend/ant-design-pro-template/jest.config.ts` does not define `coverageThreshold` or `collectCoverageFrom`.
- Backend has no coverage command or threshold declared in `backend/Cargo.toml` or repository-level config files.

**Current posture:**
- Backend coverage breadth is reasonable for isolated logic: `backend/src/` currently contains 31 `#[cfg(test)]` modules and 102 `#[test]` or `#[tokio::test]` cases.
- Frontend coverage is minimal: one real Jest test file at `frontend/ant-design-pro-template/src/pages/user/login/login.test.tsx` plus one snapshot file.

**View Coverage:**
```bash
cd frontend/ant-design-pro-template && npm run test:coverage
```

## Test Types

**Unit Tests:**
- Dominant backend pattern. Logic-heavy modules test pure or near-pure functions inline, including `backend/src/services/analyzer.rs`, `backend/src/services/predictor.rs`, `backend/src/policy/cost_model.rs`, and `backend/src/services/monitor.rs`.

**Integration Tests:**
- Limited backend integration coverage exists as a live example rather than an automated suite: `backend/examples/test_memory_search.rs` calls the HTTP API with `reqwest` and requires the server plus dependencies to be running.
- Frontend login tests are closer to component-plus-routing integration tests than isolated unit tests because they render through `TestBrowser` and hit a mock server.

**E2E Tests:**
- No dedicated Playwright, Cypress, or browser automation suite was detected.
- The frontend `record` script in `frontend/ant-design-pro-template/package.json` supports Umi request recording, but it is not an end-to-end assertion suite.

## Common Patterns

**Async Testing:**
```rust
#[tokio::test]
async fn test_select_memory_returns_error_when_what_if_fails() {
    let result = select_memory(...).await;
    assert!(result.is_err());
}
```

```tsx
it('should login success', async () => {
  await (await rootContainer.findByText('Login')).click();
  await waitTime(5000);
  await rootContainer.findAllByText('Ant Design Pro');
});
```

**Error Testing:**
```rust
let result = select_memory(...).await;
assert!(result.is_err());
```

- Backend explicitly checks failing branches in `backend/src/services/memory_orchestrator.rs` and boundary behavior in modules like `backend/src/hoops/rate_limit.rs`.
- Frontend error-path assertions are largely absent; current tests focus on happy-path login and snapshot rendering in `frontend/ant-design-pro-template/src/pages/user/login/login.test.tsx`.

## Gaps

- Frontend business pages under `frontend/ant-design-pro-template/src/pages/MemoryManagement`, `MemoryConfig`, `MemoryDecisionTrace`, `MemoryDetails`, `Performance`, `ResourceMonitor`, `TaskAnalysis`, and `WeightHistory` do not have matching test files.
- Frontend request normalization in `frontend/ant-design-pro-template/src/requestErrorConfig.ts` is untested even though it centralizes error display and auth header behavior.
- Frontend memory API clients under `frontend/ant-design-pro-template/src/services/memory/*.ts` are not directly covered and are excluded from Biome linting in `frontend/ant-design-pro-template/biome.json`.
- Backend repository methods that hit the database, especially create/list/update flows in `backend/src/db/memory.rs`, are only lightly covered; most tests validate mapping helpers rather than real database interaction.
- The live example `backend/examples/test_memory_search.rs` depends on a running backend and external services, so it does not protect CI unless explicitly wired into automation.

---

*Testing analysis: 2026-03-26*
