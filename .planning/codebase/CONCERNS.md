# Codebase Concerns

**Analysis Date:** 2026-03-26

## Tech Debt

**Active router stack is not the fully implemented one:**
- Issue: `backend/src/main.rs` boots `axum_routers::create_router()`, but the richer implementation lives under `backend/src/routers/mod.rs`. The active `backend/src/axum_routers/auth.rs`, `backend/src/axum_routers/user.rs`, and `backend/src/axum_routers/memory.rs` return placeholders, hard-coded values, or simplified responses instead of the business logic present in `backend/src/routers/`.
- Files: `backend/src/main.rs`, `backend/src/axum_routers/mod.rs`, `backend/src/axum_routers/auth.rs`, `backend/src/axum_routers/user.rs`, `backend/src/axum_routers/memory.rs`, `backend/src/routers/mod.rs`
- Impact: The running server can expose fake or incomplete behavior while the fuller code path stays unused. This creates silent functional regressions, invalid API docs, and a high chance of fixing the wrong stack.
- Fix approach: Collapse to one router tree. Either switch `main.rs` to the real router or port the real handlers into `backend/src/axum_routers/`, then delete the unused duplicate surface and add route-level integration tests against the booted router.

**Database backend claims are broader than the implementation:**
- Issue: `backend/src/db/mod.rs` advertises both PostgreSQL and SQLite initialization, but `db::pool()` panics when the active backend is SQLite. Core handlers and services call `db::pool()` directly, for example in `backend/src/routers/auth.rs` and `backend/src/services/memory_search.rs`.
- Files: `backend/src/db/mod.rs`, `backend/src/routers/auth.rs`, `backend/src/routers/user.rs`, `backend/src/services/memory_search.rs`, `backend/README.md`
- Impact: A SQLite deployment can pass startup and still panic on normal requests. Documentation and actual support diverge, which raises onboarding and operational risk.
- Fix approach: Either remove SQLite support claims from docs/config until it is complete, or introduce a backend-neutral repository layer so request handlers do not assume `PgPool`.

**Oversized modules concentrate unrelated behavior:**
- Issue: Core files such as `backend/src/routers/memory.rs` (1128 lines), `backend/src/hoops/enterprise.rs` (853 lines), `backend/src/db/agent.rs` (845 lines), and `backend/src/services/scheduler.rs` (840 lines) bundle many responsibilities into single compilation units.
- Files: `backend/src/routers/memory.rs`, `backend/src/hoops/enterprise.rs`, `backend/src/db/agent.rs`, `backend/src/services/scheduler.rs`, `frontend/ant-design-pro-template/src/pages/MemoryManagement/index.tsx`
- Impact: Review cost is high, change isolation is poor, and small edits have a larger regression surface than necessary.
- Fix approach: Split by feature boundary and dependency direction. Move request DTOs, persistence, and orchestration into separate modules and add focused tests before extracting each area.

**Documentation and shipped defaults drift from the actual project:**
- Issue: `backend/README.md` documents the `routers/` stack and SQLite-first positioning, while the live server uses `axum_routers/`. The frontend `README.md` is still the stock Ant Design Pro template, and `frontend/ant-design-pro-template/src/app.tsx` points production traffic at `https://proapi.azurewebsites.net`.
- Files: `backend/README.md`, `frontend/ant-design-pro-template/README.md`, `frontend/ant-design-pro-template/src/app.tsx`
- Impact: New contributors and deployers can follow the wrong architecture or accidentally ship the UI against an external demo backend.
- Fix approach: Replace template docs with project-specific docs, document the active router and auth flow, and externalize production API base URL into environment-specific config.

## Known Bugs

**Frontend login targets the wrong backend endpoint shape:**
- Symptoms: The frontend posts JSON credentials to `/api/login/account`, but the live Axum route expects query parameters and treats `username` as a token carrier.
- Files: `frontend/ant-design-pro-template/src/services/ant-design-pro/api.ts`, `backend/src/axum_routers/auth.rs`
- Trigger: Attempting to log in through the shipped React UI.
- Workaround: Call `POST /api/login` directly until the frontend and backend contracts are aligned.

**Current-user endpoint can return a hard-coded admin identity:**
- Symptoms: `backend/src/axum_routers/auth.rs` returns a static admin-shaped payload from `get_current_user()` instead of resolving the authenticated user from storage.
- Files: `backend/src/axum_routers/auth.rs`, `frontend/ant-design-pro-template/src/app.tsx`
- Trigger: Any authenticated UI boot flow that calls `/api/currentUser`.
- Workaround: None in the active router stack.

**Logout API is referenced by the frontend but not implemented by the backend:**
- Symptoms: The UI calls `/api/login/outLogin`, but neither router tree defines that route.
- Files: `frontend/ant-design-pro-template/src/services/ant-design-pro/api.ts`, `frontend/ant-design-pro-template/src/components/RightContent/AvatarDropdown.tsx`, `backend/src/axum_routers/auth.rs`, `backend/src/routers/mod.rs`
- Trigger: Clicking logout from the avatar menu.
- Workaround: The UI clears `localStorage`, but server-side session or cookie invalidation does not happen.

## Security Considerations

**JWTs are accepted through query parameters and stored in browser storage:**
- Risk: Tokens are accepted from `token=` query parameters in `backend/src/routers/auth.rs` and `backend/src/web/jwt.rs`, and the frontend stores the same JWT in `localStorage` in `frontend/ant-design-pro-template/src/pages/user/login/index.tsx` and replays it from `frontend/ant-design-pro-template/src/requestErrorConfig.ts`.
- Files: `backend/src/routers/auth.rs`, `backend/src/web/jwt.rs`, `frontend/ant-design-pro-template/src/pages/user/login/index.tsx`, `frontend/ant-design-pro-template/src/requestErrorConfig.ts`
- Current mitigation: HttpOnly cookies are also set in some paths.
- Recommendations: Remove token-in-query support, stop persisting JWTs in `localStorage`, rely on HttpOnly cookies or a single header-based flow, and add server-side logout/token revocation behavior.

**Authentication cookies are cross-site but not marked `Secure`:**
- Risk: Cookies are built with `SameSite::None` in `backend/src/routers/auth.rs`, but no `Secure` attribute is set. Browsers can reject them, and deployments over HTTP/TLS transitions become fragile.
- Files: `backend/src/routers/auth.rs`, `backend/src/axum_routers/auth.rs`
- Current mitigation: `HttpOnly` is set in the non-placeholder router.
- Recommendations: Set `Secure` whenever `SameSite=None` is used, centralize cookie construction, and test the login flow in both local and TLS deployments.

**The active Axum router has no visible auth middleware on protected routes:**
- Risk: `backend/src/axum_routers/mod.rs` merges all feature routers directly, and `backend/src/axum_routers/` contains no `route_layer`, `middleware::from_fn`, or claim extraction usage.
- Files: `backend/src/axum_routers/mod.rs`, `backend/src/axum_routers/user.rs`, `backend/src/axum_routers/memory.rs`
- Current mitigation: None detected in the active router tree.
- Recommendations: Add explicit route protection around protected APIs and verify unauthorized access paths with integration tests.

**Legacy user password updates write raw passwords:**
- Risk: `backend/src/routers/user.rs` hashes passwords on create, but `update_user()` writes `password` directly to the database.
- Files: `backend/src/routers/user.rs`
- Current mitigation: The legacy router tree is not the one booted by `main.rs`.
- Recommendations: Hash passwords on update immediately so the bug cannot be reintroduced by a router switch or test path.

## Performance Bottlenecks

**Memory search does N+1 database lookups after vector search:**
- Problem: `search_ltm()` fetches Qdrant hits first and then calls `LTMRepository::get_entry_by_id()` once per result. `keyword_search()` repeats the same pattern for multi-keyword scoring.
- Files: `backend/src/services/memory_search.rs`
- Cause: Search result hydration and keyword enhancement are implemented as per-row round trips instead of batched reads.
- Improvement path: Batch-load knowledge entries by ID, move score enrichment into SQL where possible, and profile the end-to-end search path with realistic `top_k` values.

**Keyword search uses `LIKE` scans instead of full-text search:**
- Problem: `keyword_search()` uses `%query%` matching against `content` and `title`, with comments noting that FTS should be used instead.
- Files: `backend/src/services/memory_search.rs`
- Cause: The text-search path is a placeholder implementation.
- Improvement path: Introduce SQLite FTS5 or PostgreSQL full-text indexing for the active backend and keep the ranking model in one query plan.

**Token budgeting is based on a crude byte heuristic:**
- Problem: `backend/src/services/context_compressor.rs` estimates tokens with `content.len() / 4`, then uses that estimate to decide whether compression is needed.
- Files: `backend/src/services/context_compressor.rs`
- Cause: There is no model-aware tokenizer in the compression path.
- Improvement path: Use the tokenizer for the configured model family or at least calibrate per-provider estimates and track actual token deltas in telemetry.

## Fragile Areas

**Startup sequencing masks infrastructure failure modes:**
- Files: `backend/src/main.rs`, `backend/src/config/neo4j_config.rs`
- Why fragile: `main.rs` ignores the result of `init_neo4j()` but logs success immediately after, then hard-fails on `init_neo4j_indexes()`. Combined with the placeholder Neo4j password default in `backend/src/config/neo4j_config.rs`, startup diagnostics can be misleading.
- Safe modification: Make startup either fail fast on the first connection error or degrade explicitly with accurate logs and feature gating.
- Test coverage: No startup integration test was found that exercises missing Neo4j credentials or partial service availability.

**The live frontend boot flow depends on mismatched backend contracts:**
- Files: `frontend/ant-design-pro-template/src/app.tsx`, `frontend/ant-design-pro-template/src/services/ant-design-pro/api.ts`, `backend/src/axum_routers/auth.rs`
- Why fragile: The UI fetches `/api/currentUser`, assumes login state from that response, and redirects aggressively. The backend currently mixes hard-coded user data with a separate `/api/login` credential path that the UI does not call.
- Safe modification: Align the frontend to one explicit auth contract first, then add an end-to-end login/logout smoke test before changing routing or response formats.
- Test coverage: Frontend tests only cover `src/pages/user/login/login.test.tsx`, which is a template-style login test and does not verify the active backend integration.

## Missing Critical Features

**No authoritative route contract for the running server:**
- Problem: There are two competing router implementations, and the live one is incomplete.
- Blocks: Reliable API evolution, accurate OpenAPI generation, and trustworthy frontend-backend integration.

**No project-specific frontend deployment configuration:**
- Problem: Production requests default to `https://proapi.azurewebsites.net` in `frontend/ant-design-pro-template/src/app.tsx`.
- Blocks: Safe production builds of the actual Adaptive Memory System UI.

## Test Coverage Gaps

**Active Axum router behavior is effectively untested:**
- What's not tested: The booted `backend/src/axum_routers/` stack, including login, current user, authorization boundaries, and placeholder endpoints.
- Files: `backend/src/main.rs`, `backend/src/axum_routers/mod.rs`, `backend/src/axum_routers/auth.rs`, `backend/src/axum_routers/user.rs`, `backend/src/axum_routers/memory.rs`
- Risk: The shipping API surface can diverge from the intended one without failing CI.
- Priority: High

**Frontend integration coverage stops at a single template login test:**
- What's not tested: Real API integration for dashboard, memory pages, auth persistence, logout, and failure states.
- Files: `frontend/ant-design-pro-template/src/pages/user/login/login.test.tsx`, `frontend/ant-design-pro-template/src/app.tsx`, `frontend/ant-design-pro-template/src/requestErrorConfig.ts`
- Risk: Routing, auth, and data loading regressions can ship unnoticed.
- Priority: High

---

*Concerns audit: 2026-03-26*
