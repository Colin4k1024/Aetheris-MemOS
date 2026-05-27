# External Integrations

**Analysis Date:** 2026-03-28

## APIs & External Services

**LLM Service (Ollama):**
- Purpose: Content summarization, structured extraction, reranking
  - Implementation: `backend/src/services/llm.rs`
  - API endpoint: `/api/generate` on Ollama
  - Config: `backend/src/config/llm_config.rs`
  - Auth: No auth (local service on localhost:11434)
  - Default model: llama3

**Embedding Service (Ollama):**
- Purpose: Generate text embeddings for semantic search
  - Implementation: `backend/src/services/embedding.rs`
  - API endpoint: `/api/embeddings` on Ollama
  - Config: `backend/src/config/embedding_config.rs`
  - Auth: No auth (local service on localhost:11434)
  - Default model: nomic-embed-text
  - Caching: Moka in-memory cache (10000 entries, 24h TTL)

**Rerank Service (Ollama):**
- Purpose: Re-rank search results using LLM relevance scoring
  - Implementation: `backend/src/services/rerank.rs`
  - API endpoint: `/api/generate` on Ollama
  - Config: `backend/src/config/rerank_config.rs`
  - Auth: No auth (local service on localhost:11434)
  - Default model: bge-reranker-base

## Data Storage

**PostgreSQL:**
- Type: Relational database with vector support (pgvector)
  - Image: `pgvector/pgvector:pg16` (`docker-compose.yml`)
  - Ports: `5432:5432`
  - Connection: Configured via `DATABASE_URL` or `db.url` in config
  - Client: SQLx 0.8 (`backend/src/db/mod.rs`)
  - Purpose: STM sessions, LTM metadata, weights, performance metrics, decision traces

**SQLite:**
- Type: Embedded database (development fallback)
  - Path: `data/memory.db` (WAL mode enabled)
  - Client: SQLx 0.8 with SQLite support
  - Purpose: Same as PostgreSQL when running without PostgreSQL
  - Config: `backend/src/config/storage.rs`

**Qdrant:**
- Type: Vector database
  - Image: `qdrant/qdrant:latest` (`docker-compose.yml`)
  - Ports: `6333:6333` (REST), `6334:6334` (gRPC)
  - Connection: `http://localhost:6334` via config
  - Client: `qdrant-client` 1.7 (`backend/src/services/qdrant.rs`)
  - Purpose: LTM vector storage, semantic search
  - Collection: `long_term_memory` (configurable)

**Neo4j:**
- Type: Graph database (optional)
  - Image: `neo4j:5` (`docker-compose.yml`)
  - Ports: `7474:7474` (HTTP), `7687:7687` (Bolt)
  - Connection: Configured via `backend/config.toml` or `APP_NEO4J_PASSWORD`
  - Client: `neo4rs` 0.8 (`backend/src/db/neo4j.rs`)
  - Purpose: Knowledge graph entities and relationships
  - Auth: `neo4j` / password

**In-Memory Cache:**
- Type: Moka cache
  - Purpose: Embedding result caching
  - Location: `backend/src/services/embedding.rs`
  - Capacity: 10000 entries
  - TTL: 24 hours

## Authentication & Identity

**JWT Authentication:**
- Implementation: `backend/src/hoops/jwt.rs`, `backend/src/web/jwt.rs`
- Algorithm: RS256/RSA via `jsonwebtoken` with `rust_crypto` feature
- Secret: Configured via `jwt.secret` or `APP_JWT_SECRET` env var
- Expiry: 3600 seconds (configurable)
- Cookie-based auth also supported

**Password Hashing:**
- Algorithm: Argon2
- Implementation: `backend/src/services/agent_identity.rs`

**Enterprise Features:**
- API key support in `backend/src/hoops/enterprise_impl.rs`
- Rate limiting in `backend/src/hoops/rate_limit.rs`

## Monitoring & Observability

**Logging:**
- Framework: `tracing` + `tracing-subscriber` + `tracing-appender`
- Outputs: stdout, rolling daily log file
- Config: `backend/src/config/log_config.rs`
- Format: JSON with timestamps for production, pretty for dev
- Env filter for log level control

**Frontend Error Handling:**
- Browser-side failures surfaced as Ant Design notifications
- Location: `frontend/ant-design-pro-template/src/requestErrorConfig.ts`

## CI/CD & Deployment

**Hosting:**
- Docker Compose for local development (`docker-compose.yml`)
- Backend container via `backend/Dockerfile`
- Frontend static deployment via `gh-pages`

**CI Pipeline:**
- GitHub Actions: `.github/workflows/backend-ci.yml`, `.github/workflows/frontend-ci.yml`
- Backend: cargo build, cargo test
- Frontend: npm install, npm run lint, npm run build

## Environment Configuration

**Critical env vars:**

| Variable | Description | Example |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | `postgres://memory:memory@localhost:5432/memory` |
| `APP_JWT_SECRET` | JWT signing secret | `your_strong_random_secret` |
| `APP_NEO4J_PASSWORD` | Neo4j password | `password` |
| `APP_CONFIG` | Backend config file override | `/path/to/config.toml` |

**Config file (`backend/config.toml`):**
```toml
listen_addr = "127.0.0.1:8008"

[db]
url = "postgres://memory:memory@localhost:5432/memory"

[jwt]
secret = "REPLACE_WITH_STRONG_SECRET_OR_USE_APP_JWT_SECRET"

[llm]
base_url = "http://localhost:11434"
model = "llama3"

[embedding]
base_url = "http://localhost:11434"
model = "nomic-embed-text"

[qdrant]
host = "localhost"
port = 6334

[neo4j]
host = "localhost"
port = 7687
username = "neo4j"
password = "REPLACE_WITH_YOUR_NEO4J_PASSWORD"
```

## Frontend-Backend Communication

**Protocol:** REST over HTTP
**Base URL:** `http://localhost:8008` (backend)

**Key Endpoints:**

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/v1/memory/storage/stm` | POST | Store short-term memory |
| `/api/v1/memory/storage/ltm` | POST | Store long-term memory |
| `/api/v1/memory/search/ltm` | GET/POST | List/search LTM |
| `/api/v1/memory/search/hybrid` | POST | Hybrid vector + keyword search |
| `/api/v1/memory/search/triple` | POST | Triple hybrid retrieval |
| `/api/v1/memory/search/scored` | POST | Retrieval with confidence scoring |
| `/api/kg/entities` | GET | List KG entities |
| `/api/tenants` | GET/POST | Tenant management |
| `/scalar` | GET | OpenAPI docs (Scalar UI) |

**Frontend Service Files:**
- `frontend/ant-design-pro-template/src/services/memory/storageApi.ts` - STM/LTM APIs
- `frontend/ant-design-pro-template/src/services/memory/knowledgeGraphApi.ts` - KG APIs
- `frontend/ant-design-pro-template/src/services/memory/multimodalApi.ts` - MM APIs
- `frontend/ant-design-pro-template/src/services/memory/index.ts` - API exports

## SDK Integrations

**Rust SDK:**
- Location: `sdks/rust/`
- Package: `adaptive-memory`
- Dependencies: `reqwest`, `serde`, `tokio`, `thiserror`, `anyhow`
- File: `sdks/rust/Cargo.toml`

**Python SDK:**
- Location: `sdks/python/`
- Build: `pyproject.toml` based
- File: `sdks/python/pyproject.toml`

## Service Communication Flow

```
Frontend (React) - Port 8000
    |
    v HTTP REST
Backend (Axum/Rust) - Port 8008
    |
    +---> PostgreSQL:5432 - Relational data (SQLx)
    +---> Qdrant:6334 - Vector storage (gRPC)
    +---> Neo4j:7687 - Graph data (Bolt)
    +---> Ollama:11434 - LLM/Embeddings (HTTP)
```

## Webhooks & Callbacks

**Incoming:**
- None detected. Repository exposes HTTP APIs and MCP endpoints only.

**Outgoing:**
- Backend -> Ollama: LLM calls, embedding generation, reranking (`backend/src/services/*.rs`)
- Backend -> Qdrant: Vector search operations (`backend/src/services/qdrant.rs`)
- Backend -> Neo4j: Graph traversal and updates (`backend/src/db/neo4j.rs`)

## Documentation

**API Documentation:**
- Scalar UI at `/scalar` endpoint
- OpenAPI spec generated via Utoipa (`backend/src/axum_routers/mod.rs`)

**Scalar CDN:**
- `@scalar/api-reference` loaded from public CDN (`backend/src/axum_routers/mod.rs`)

---

*Integration audit: 2026-03-28*
