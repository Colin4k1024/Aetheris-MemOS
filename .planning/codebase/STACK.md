# Technology Stack

**Analysis Date:** 2026-03-28

## Languages

**Primary:**
- Rust 1.89+ - Backend API server, services, database integrations (`backend/Cargo.toml`, `backend/src/main.rs`)
- TypeScript 5.6.3 - Frontend React application (`frontend/ant-design-pro-template/package.json`)

**Secondary:**
- Python 3.9+ - SDK package (`sdks/python/pyproject.toml`)
- SQL - Database queries via SQLx compile-time checks (`backend/migrations/*.sql`)
- Shell - Ops scripts (`backend/scripts/*.sh`)
- YAML - Docker and CI definitions (`docker-compose.yml`, `.github/workflows/*.yml`)
- TOML - Runtime configuration (`backend/config.toml.example`, `backend/src/config/*.rs`)

## Runtime

**Backend:**
- Tokio 1.x - Async runtime for Rust (`backend/Cargo.toml`)
- Node.js 20+ - Frontend development and build (`frontend/ant-design-pro-template/package.json`)

**Package Manager:**
- Cargo - Rust dependencies, lockfile: `backend/Cargo.lock`, `sdks/rust/Cargo.lock`
- npm 10+ - Node dependencies, lockfile: `frontend/ant-design-pro-template/package-lock.json`
- pip/setuptools - Python SDK (`sdks/python/pyproject.toml`)

## Frameworks

**Backend:**
- Axum 0.8 - Web framework, HTTP server, routing (`backend/src/main.rs`, `backend/src/axum_routers/`)
- Tower 0.5 - Middleware composition (`backend/Cargo.toml`)
- Tower-HTTP 0.6 - CORS, tracing, static files (`backend/src/hoops/cors.rs`)
- SQLx 0.8 - Async database toolkit with compile-time query checks (`backend/src/db/mod.rs`)
- Utoipa 5 - OpenAPI/Swagger documentation (`backend/src/axum_routers/mod.rs`)
- Rinja 0.3 - HTML templates (`backend/views/**`)

**Frontend:**
- Umi 4 (Max) 4.6.2 - React application framework (`frontend/ant-design-pro-template/package.json`)
- Ant Design Pro Components 2.8.9 - Enterprise UI components (`frontend/ant-design-pro-template/src/pages/**`)
- Ant Design 6.0.0 - Core UI library (`frontend/ant-design-pro-template/package.json`)
- React 19.1 - UI framework

**Testing:**
- Rust built-in `#[test]`, `#[tokio::test]` - Backend testing (`backend/Cargo.toml`)
- Jest 30 - Frontend testing (`frontend/ant-design-pro-template/package.json`, `frontend/ant-design-pro-template/jest.config.ts`)
- Pytest - Python SDK testing (`sdks/python/pyproject.toml`)

**Build/Dev:**
- Biome 2.1.1 - Linting and formatting (`frontend/ant-design-pro-template/biome.json`)
- Husky + commitlint - Commit hooks (`frontend/ant-design-pro-template/package.json`)
- SQLx offline mode - Compile-time query verification (`backend/.sqlx/`)

## Key Dependencies

**Critical:**

| Package | Version | Purpose |
|---------|---------|---------|
| `axum` | 0.8 | HTTP server and routing |
| `tokio` | 1 | Async runtime |
| `sqlx` | 0.8 | PostgreSQL/SQLite with compile-time checks |
| `serde` | 1 | Serialization/deserialization |
| `qdrant-client` | 1.7 | Vector database client |
| `neo4rs` | 0.8 | Neo4j graph database client |
| `reqwest` | 0.12 | HTTP client for Ollama API calls |
| `langchain-rust` | git | LLM/embedding integration (Ollama) |
| `jsonwebtoken` | 10 | JWT authentication |
| `validator` | 0.20 | Input validation |
| `ulid` | 1 | Unique ID generation |

**Infrastructure:**

| Package | Version | Purpose |
|---------|---------|---------|
| `tracing` | 0.1 | Structured logging |
| `tracing-subscriber` | 0.3 | Log formatting and filtering |
| `chrono` | 0.4 | Date/time handling |
| `moka` | 0.11 | In-memory caching for embeddings |
| `argon2` | 0.5 | Password hashing |
| `time` | 0.3 | Time handling |
| `dotenvy` | 0.15 | Environment variable loading |
| `sysinfo` | 0.29 | Hardware detection for model routing |

**Frontend:**

| Package | Version | Purpose |
|---------|---------|---------|
| `@ant-design/icons` | 6.1 | Icon library |
| `@ant-design/charts` | 2.0.3 | Dashboard visualizations |
| `dayjs` | 1.11 | Date formatting |
| `classnames` | 2.5.1 | Conditional classnames |

## Configuration

**Environment:**
- Backend config via Figment from `APP_CONFIG`, `config.toml`, `local.toml`, or `~/.adaptive-memory/config.toml` (`backend/src/config/mod.rs`)
- `DATABASE_URL` overrides database URL; fallback to SQLite if not set (`backend/src/config/storage.rs`)
- `APP_`-prefixed env vars merged globally (`backend/config.toml.example`)
- `OPENWEBUI_DATA_DIR` and `ADAPTIVE_MEMORY_DATA_DIR` for storage paths (`backend/src/config/storage.rs`)
- Frontend uses `UMI_ENV`, `MOCK`, `CI` environment variables (`frontend/ant-design-pro-template/config/config.ts`)

**Build:**
- Backend: `backend/Cargo.toml`, `backend/Cargo.lock`, `backend/Dockerfile`
- Frontend: `frontend/ant-design-pro-template/package.json`, `frontend/ant-design-pro-template/tsconfig.json`, `frontend/ant-design-pro-template/biome.json`
- CI: `.github/workflows/backend-ci.yml`, `.github/workflows/frontend-ci.yml`

## Platform Requirements

**Development:**
- Rust 1.89+ (`backend/Cargo.toml`)
- Node.js 20+ (`frontend/ant-design-pro-template/package.json`)
- Docker for PostgreSQL (pgvector), Qdrant, Neo4j (`docker-compose.yml`)
- Ollama running locally for LLM/embedding features (`backend/config.toml.example`)

**Production:**
- Linux container via `backend/Dockerfile`
- PostgreSQL 14+ or SQLite for relational data
- Qdrant for vector storage
- Neo4j (optional) for graph traversal

## Database Systems

**Relational (Primary):**
- PostgreSQL 14+ via `pgvector/pgvector:pg16` Docker image (`docker-compose.yml`)
- SQLite (embedded, development fallback) - WAL mode enabled
- Both via SQLx with compile-time query verification (`backend/src/db/mod.rs`)
- Tables: STM sessions, LTM metadata, weights history, performance metrics, decision traces, evidence graph

**Vector Database:**
- Qdrant (latest) - For semantic search and LTM vector storage
- Client: `qdrant-client` 1.7 (`backend/src/services/qdrant.rs`)
- Collection: `long_term_memory` (configurable)
- Ports: REST 6333, gRPC 6334

**Graph Database:**
- Neo4j 5.x (optional) - For knowledge graph with bi-temporal state
- Client: `neo4rs` 0.8 (`backend/src/db/neo4j.rs`)
- Ports: HTTP 7474, Bolt 7687
- Auth: `neo4j` / configured password

**In-Memory Cache:**
- Moka 0.11 - Embedding result caching (`backend/src/services/embedding.rs`)
- Capacity: 10000 entries
- TTL: 24 hours

---

*Stack analysis: 2026-03-28*
