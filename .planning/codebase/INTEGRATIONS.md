# External Integrations

**Analysis Date:** 2026-03-26

## APIs & External Services

**Model Serving:**
- Ollama - backend LLM generation, embedding generation, and reranking calls.
  - SDK/Client: `reqwest` in `backend/src/services/llm.rs`, `backend/src/services/embedding.rs`, and `backend/src/services/rerank.rs`.
  - Auth: Not detected; the configured base URL is read from backend config in `backend/src/config/llm_config.rs`, `backend/src/config/embedding_config.rs`, and `backend/src/config/rerank_config.rs`.

**Vector Search:**
- Qdrant - long-term memory vector index and similarity search.
  - SDK/Client: `qdrant-client` in `backend/src/services/qdrant.rs`.
  - Auth: Not detected; host/port/collection settings come from backend config in `backend/src/config/qdrant_config.rs`.

**Graph Database:**
- Neo4j - optional graph persistence for knowledge graph operations.
  - SDK/Client: `neo4rs` in `backend/src/db/neo4j.rs`.
  - Auth: username/password loaded from backend config in `backend/src/config/neo4j_config.rs`; do not hardcode production credentials into `backend/config.toml`.

**Documentation UI / Remote Assets:**
- Scalar API Reference CDN - OpenAPI docs page loads `@scalar/api-reference` from a public CDN in `backend/src/axum_routers/mod.rs` and `backend/src/routers/mod.rs`.
- Ant Design / Alipay-hosted assets - default frontend logos/backgrounds and sample schemas are referenced in `frontend/ant-design-pro-template/config/defaultSettings.ts`, `frontend/ant-design-pro-template/src/app.tsx`, and `frontend/ant-design-pro-template/config/config.ts`.

**Protocol / Runtime Interop:**
- MCP (Model Context Protocol) - backend exposes an MCP HTTP surface for memory tools in `backend/src/routers/mcp.rs` and SDK clients consume MCP endpoints in `sdks/rust/src/mcp.rs`, `sdks/rust/src/memory.rs`, and `sdks/python/adaptive_memory/client.py`.
- Oris integration - in-memory runtime integration types exist in `backend/src/integrations/oris.rs`; no external network client is wired for Oris in this repository.

## Data Storage

**Databases:**
- PostgreSQL / pgvector image - primary relational store in Docker and SQLx runtime paths.
  - Connection: `DATABASE_URL` or backend TOML config resolved in `backend/src/config/mod.rs`.
  - Client: `sqlx` in `backend/src/db/mod.rs` and repositories under `backend/src/db/**`.
- SQLite - local-first fallback database when `DATABASE_URL` is not set.
  - Connection: derived by `backend/src/config/storage.rs`.
  - Client: `sqlx` in `backend/src/db/mod.rs`.
- Qdrant - vector store for LTM search.
  - Connection: config section in `backend/src/config/qdrant_config.rs`.
  - Client: `backend/src/services/qdrant.rs`.
- Neo4j - graph store for KG operations.
  - Connection: config section in `backend/src/config/neo4j_config.rs`.
  - Client: `backend/src/db/neo4j.rs`.

**File Storage:**
- Local filesystem only.
  - Persistent app data is resolved under the OS data directory or overrides from `OPENWEBUI_DATA_DIR` / `ADAPTIVE_MEMORY_DATA_DIR` in `backend/src/config/storage.rs`.
  - Write-ahead journal and vector signature files are stored locally by `backend/src/services/information_guard.rs` and `backend/src/services/vector_guard.rs`.
  - Static frontend-like assets and HTML templates are served from `backend/assets/` and `backend/views/`.

**Caching:**
- In-process cache only.
  - `moka` caches embeddings in `backend/src/services/embedding.rs`.
  - In-memory rate limiting state exists in `backend/src/hoops/rate_limit.rs` and `backend/src/web/rate_limit.rs`.
  - No Redis, Memcached, or external distributed cache was detected.

## Authentication & Identity

**Auth Provider:**
- Custom JWT authentication.
  - Implementation: JWT encode/decode in `backend/src/hoops/jwt.rs`, cookie-based login/logout flows in `backend/src/routers/auth.rs` and `backend/src/axum_routers/auth.rs`, and frontend token storage/header injection in `frontend/ant-design-pro-template/src/requestErrorConfig.ts` and `frontend/ant-design-pro-template/src/pages/user/login/index.tsx`.
- API key support is also present for enterprise-style hooks and SDK clients.
  - Implementation: backend auth hook scaffolding in `backend/src/hoops/enterprise_impl.rs` and `backend/src/web/rate_limit.rs`; optional bearer/API-key handling in `sdks/rust/src/client.rs` and `sdks/python/adaptive_memory/client.py`.

## Monitoring & Observability

**Error Tracking:**
- None detected for external SaaS error tracking.

**Logs:**
- Structured local logging via `tracing`, `tracing-subscriber`, and `tracing-appender` in `backend/src/config/log_config.rs`.
- Browser-side request failures are surfaced as Ant Design messages/notifications in `frontend/ant-design-pro-template/src/requestErrorConfig.ts`.

## CI/CD & Deployment

**Hosting:**
- Backend is packaged as a container with `backend/Dockerfile`; local multi-service orchestration is defined in `docker-compose.yml`.
- Frontend includes a static deployment script using `gh-pages` in `frontend/ant-design-pro-template/package.json`.
- Vendored upstream template workflows also target GitHub Pages and Surge previews in `frontend/ant-design-pro-template/.github/workflows/deploy.yml` and `frontend/ant-design-pro-template/.github/workflows/preview-deploy.yml`.

**CI Pipeline:**
- GitHub Actions runs backend Rust checks/build/tests in `.github/workflows/backend-ci.yml` and `.github/workflows/ci.yml`.
- GitHub Actions runs frontend install/lint/build in `.github/workflows/frontend-ci.yml` and `.github/workflows/ci.yml`.
- The vendored template carries additional GitHub Actions for Bun-based CI, coverage upload, preview deploys, and issue automation in `frontend/ant-design-pro-template/.github/workflows/*.yml`.

## Environment Configuration

**Required env vars:**
- `APP_CONFIG` - explicit backend config file override in `backend/src/config/mod.rs`.
- `APP_...` - backend-wide Figment override prefix in `backend/src/config/mod.rs`; examples are documented in `backend/config.toml.example`.
- `DATABASE_URL` - backend primary database connection in `backend/src/config/mod.rs`.
- `OPENWEBUI_DATA_DIR` and `ADAPTIVE_MEMORY_DATA_DIR` - local storage path overrides in `backend/src/config/storage.rs`.
- `UMI_ENV`, `MOCK`, and `CI` - frontend dev/build mode controls in `frontend/ant-design-pro-template/package.json` and `frontend/ant-design-pro-template/config/config.ts`.
- `GITHUB_TOKEN` and `SURGE_TOKEN` - CI secrets referenced by GitHub workflow files in `.github/workflows/backend-ci.yml` and `frontend/ant-design-pro-template/.github/workflows/preview-deploy.yml`.

**Secrets location:**
- Runtime secrets are expected through environment variables or local TOML config files such as `backend/config.toml`, `backend/local.toml`, and `backend/docker.toml`; avoid committing real credentials there.
- CI secrets are provided through GitHub Actions secrets in workflow files under `.github/workflows/` and `frontend/ant-design-pro-template/.github/workflows/`.

## Webhooks & Callbacks

**Incoming:**
- None detected. The repository exposes HTTP APIs and MCP endpoints, but no webhook receiver pattern is configured in routing files under `backend/src/routers/**` or `backend/src/axum_routers/**`.

**Outgoing:**
- Backend outbound HTTP calls go to Ollama in `backend/src/services/llm.rs`, `backend/src/services/embedding.rs`, and `backend/src/services/rerank.rs`.
- Backend outbound gRPC/HTTP2 calls go to Qdrant in `backend/src/services/qdrant.rs`.
- Backend outbound Bolt protocol calls go to Neo4j in `backend/src/db/neo4j.rs`.
- Frontend development mode targets the local backend at `http://127.0.0.1:8008` in `frontend/ant-design-pro-template/src/app.tsx`.
- Frontend non-development fallback and proxy settings still reference template/demo endpoints in `frontend/ant-design-pro-template/src/app.tsx` and `frontend/ant-design-pro-template/config/proxy.ts`; treat those as template carryovers unless intentionally retained.

---

*Integration audit: 2026-03-26*
