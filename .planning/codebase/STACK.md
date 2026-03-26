# Technology Stack

**Analysis Date:** 2026-03-26

## Languages

**Primary:**
- Rust 2021 / Rust 1.89+ - backend API server and memory kernel in `backend/Cargo.toml`, `backend/src/main.rs`, and `backend/src/**`.
- TypeScript + TSX - frontend application in `frontend/ant-design-pro-template/package.json`, `frontend/ant-design-pro-template/tsconfig.json`, and `frontend/ant-design-pro-template/src/**`.

**Secondary:**
- Python 3.9+ - auxiliary SDK package in `sdks/python/pyproject.toml` and `sdks/python/adaptive_memory/**`.
- SQL - relational schema and seed data in `backend/migrations/*.sql`, `backend/seed_data.sql`, and `backend/seed_data_extended.sql`.
- Shell - local ops/demo scripts in `backend/scripts/check_services.sh`, `backend/scripts/memory_demo.sh`, and `backend/docker-entrypoint.sh`.
- YAML - Docker and CI definitions in `docker-compose.yml` and `.github/workflows/*.yml`.
- TOML / JSON - runtime and tool configuration in `backend/config.toml.example`, `backend/src/config/mod.rs`, `frontend/ant-design-pro-template/config/config.ts`, and `frontend/ant-design-pro-template/biome.json`.

## Runtime

**Environment:**
- Rust binary runtime via Tokio + Axum in `backend/Cargo.toml` and `backend/src/main.rs`.
- Node.js `>=20.0.0` for the frontend toolchain in `frontend/ant-design-pro-template/package.json`.
- Python package runtime for the SDK in `sdks/python/pyproject.toml`.
- Docker-based local infra in `docker-compose.yml` and container packaging in `backend/Dockerfile`.

**Package Manager:**
- Cargo - backend and Rust SDK in `backend/Cargo.toml` and `sdks/rust/Cargo.toml`.
- npm - frontend app in `frontend/ant-design-pro-template/package.json`.
- setuptools / pip build metadata - Python SDK in `sdks/python/pyproject.toml` and `sdks/python/setup.py`.
- Lockfile: present in `backend/Cargo.lock`, `sdks/rust/Cargo.lock`, and `frontend/ant-design-pro-template/package-lock.json`.

## Frameworks

**Core:**
- Axum `0.8` - HTTP API server in `backend/Cargo.toml` and `backend/src/main.rs`.
- Tower / tower-http - middleware, CORS, static serving, and tracing in `backend/Cargo.toml`, `backend/src/hoops/cors.rs`, and `backend/src/routers/mod.rs`.
- Umi Max `4.6.2` - React application framework and request/runtime layer in `frontend/ant-design-pro-template/package.json`, `frontend/ant-design-pro-template/config/config.ts`, and `frontend/ant-design-pro-template/src/app.tsx`.
- Ant Design Pro Components `2.8.9` + Ant Design `6.0.0` - UI shell and pages in `frontend/ant-design-pro-template/package.json` and `frontend/ant-design-pro-template/src/pages/**`.

**Testing:**
- Rust built-in test framework via `cargo test` in `backend/Cargo.toml`, `.github/workflows/backend-ci.yml`, and `.github/workflows/ci.yml`.
- Jest `30.x` + `@umijs/max/test` + Testing Library in `frontend/ant-design-pro-template/package.json` and `frontend/ant-design-pro-template/jest.config.ts`.
- Pytest is declared for Python SDK development in `sdks/python/pyproject.toml`.

**Build/Dev:**
- SQLx `0.8` with offline metadata in `backend/Cargo.toml`, `backend/.sqlx/`, and `backend/Dockerfile`.
- Utoipa `5` + Scalar UI - OpenAPI generation and docs UI in `backend/Cargo.toml`, `backend/src/axum_routers/mod.rs`, and `backend/src/routers/mod.rs`.
- Rinja `0.3` - HTML templates in `backend/Cargo.toml`, `backend/rinja.toml`, and `backend/views/**`.
- Biome `2.1.1` - frontend lint/format tool in `frontend/ant-design-pro-template/package.json` and `frontend/ant-design-pro-template/biome.json`.
- Husky + commitlint + lint-staged - commit hooks in `frontend/ant-design-pro-template/package.json` and `frontend/ant-design-pro-template/.husky/*`.

## Key Dependencies

**Critical:**
- `sqlx` - relational persistence for PostgreSQL and SQLite in `backend/Cargo.toml` and `backend/src/db/mod.rs`.
- `qdrant-client` - vector search integration in `backend/Cargo.toml` and `backend/src/services/qdrant.rs`.
- `neo4rs` - knowledge graph database access in `backend/Cargo.toml` and `backend/src/db/neo4j.rs`.
- `reqwest` - backend HTTP calls to local model services and SDK HTTP clients in `backend/Cargo.toml`, `backend/src/services/embedding.rs`, `backend/src/services/llm.rs`, `sdks/rust/src/client.rs`, and `sdks/python/adaptive_memory/client.py`.
- `langchain-rust` (git dependency) - Ollama-capable LLM integration surface declared in `backend/Cargo.toml`.
- `jsonwebtoken` - JWT auth in `backend/Cargo.toml` and `backend/src/hoops/jwt.rs`.
- `@umijs/max` - frontend runtime, router, and request layer in `frontend/ant-design-pro-template/package.json`, `frontend/ant-design-pro-template/src/app.tsx`, and `frontend/ant-design-pro-template/src/requestErrorConfig.ts`.

**Infrastructure:**
- `tracing`, `tracing-subscriber`, `tracing-appender` - log pipeline in `backend/Cargo.toml` and `backend/src/config/log_config.rs`.
- `moka` - in-process embedding cache in `backend/Cargo.toml` and `backend/src/services/embedding.rs`.
- `sysinfo` - hardware detection in `backend/Cargo.toml` and `backend/src/services/hardware_detector.rs`.
- `@ant-design/charts` - dashboard visualizations in `frontend/ant-design-pro-template/package.json` and `frontend/ant-design-pro-template/src/pages/**`.
- `gh-pages` - static deployment script for the frontend template in `frontend/ant-design-pro-template/package.json`.

## Configuration

**Environment:**
- Backend config is resolved through Figment from `APP_CONFIG`, `config.toml`, `local.toml`, or `~/.adaptive-memory/config.toml` in `backend/src/config/mod.rs`.
- `DATABASE_URL` overrides the configured database URL in `backend/src/config/mod.rs`; if missing, the backend falls back to local SQLite via `backend/src/config/storage.rs`.
- `APP_`-prefixed env vars are merged globally into backend config in `backend/src/config/mod.rs`; example env-backed fields are documented in `backend/config.toml.example`.
- Storage path overrides use `OPENWEBUI_DATA_DIR` and `ADAPTIVE_MEMORY_DATA_DIR` in `backend/src/config/storage.rs`.
- Frontend runtime modes use `UMI_ENV`, `MOCK`, and `CI` in `frontend/ant-design-pro-template/package.json`, `frontend/ant-design-pro-template/config/config.ts`, and `frontend/ant-design-pro-template/src/app.tsx`.
- No `.env` files were detected within three directory levels of the repository root during this audit.

**Build:**
- Backend build/test/container files: `backend/Cargo.toml`, `backend/Cargo.lock`, `backend/Dockerfile`, `backend/.sqlx/`, `backend/docker-entrypoint.sh`.
- Frontend build/test files: `frontend/ant-design-pro-template/package.json`, `frontend/ant-design-pro-template/package-lock.json`, `frontend/ant-design-pro-template/tsconfig.json`, `frontend/ant-design-pro-template/jest.config.ts`, `frontend/ant-design-pro-template/biome.json`, `frontend/ant-design-pro-template/config/config.ts`.
- Repo CI files: `.github/workflows/backend-ci.yml`, `.github/workflows/frontend-ci.yml`, `.github/workflows/ci.yml`.

## Platform Requirements

**Development:**
- Rust 1.89+ for the backend in `backend/Cargo.toml` and `.github/workflows/backend-ci.yml`.
- Node.js 20+ for the frontend in `frontend/ant-design-pro-template/package.json` and `.github/workflows/frontend-ci.yml`.
- Docker for PostgreSQL, Qdrant, and Neo4j in `docker-compose.yml`.
- Ollama running locally or on the Docker host for LLM, embedding, and rerank features in `backend/config.toml.example`, `backend/docker.toml`, and `backend/scripts/check_services.sh`.

**Production:**
- Backend targets a Linux container built from `backend/Dockerfile` and started via `backend/docker-entrypoint.sh`.
- Frontend has a static-site deployment path through `gh-pages` in `frontend/ant-design-pro-template/package.json`; vendored template workflows also include GitHub Pages and Surge preview automation under `frontend/ant-design-pro-template/.github/workflows/*.yml`.
- No Terraform, Pulumi, Helm, or other infrastructure-as-code stack was detected in the repository root.

---

*Stack analysis: 2026-03-26*
