# Aetheris MemOS

<div align="center">

**The Memory Operating System for AI Agents**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Backend CI](https://github.com/Colin4k1024/adaptive-memory-system/actions/workflows/ci.yml/badge.svg)](https://github.com/Colin4k1024/adaptive-memory-system/actions)
[![Rust](https://img.shields.io/badge/Rust-1.89+-orange.svg)](https://www.rust-lang.org)
[![Node](https://img.shields.io/badge/Node-20+-green.svg)](https://nodejs.org)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/Colin4k1024/adaptive-memory-system)

[Quick Start](#quick-start) · [Architecture](#architecture) · [API Docs](#api-documentation) · [Roadmap](#roadmap) · [中文文档](README.zh.md)

</div>

---

Aetheris MemOS is the memory operating system for AI agents.

Unlike traditional stateless LLM systems, MemOS provides a unified cognitive memory layer that enables agents to store, retrieve, reason, and evolve over time.

Most agent stacks today stop at orchestration. They can route prompts, call tools, and chain model invocations, but they still forget. RAG partially addresses this with vector retrieval, yet retrieval alone is not memory. Real memory requires multiple layers, temporal structure, graph reasoning, compression, confidence estimation, and explainable decisions.

MemOS turns memory into infrastructure. It gives agents a persistent kernel spanning short-term memory, long-term memory, knowledge graphs, and multimodal context, exposed through a consistent API and adaptive decision pipeline.

```text
Without MemOS                    With MemOS
-----------------                ----------------------------------
User prompt                      User prompt
    |                                |
    v                                v
[LLM call]                      [Memory Kernel]
    |                            |- Retrieve: triple-hybrid search
    v                            |- Reason: knowledge graph traversal
 Response                        |- Compress: adaptive context packing
(context lost)                   |- Persist: STM -> LTM promotion
                                     |
                                     v
                                 [LLM call]
                                     |
                                     v
                                 Response + memory update
                                 (agent improves over time)
```

## Why MemOS

AI agents need more than prompts and vector stores.

| Problem | MemOS Solution |
|---------|----------------|
| Stateless LLM calls lose context between sessions | Persistent STM and LTM with automatic transfer |
| RAG is limited to similarity search | Triple hybrid retrieval: vector + keyword + graph |
| Memory selection is ad hoc and prompt-dependent | Adaptive scheduler chooses memory strategy per task |
| Context windows overflow as conversations grow | Multi-strategy context compression keeps signal, drops noise |
| Retrieval quality is opaque | Confidence scoring exposes why a memory is trustworthy |
| Agent decisions are hard to audit | Decision traces make retrieval and scheduling explainable |
| Multi-agent systems duplicate knowledge | Tenant-aware cross-agent sharing enables safe reuse |
| Embedding model changes can poison retrieval quality | Vector signature guards prevent cross-model collapse |

## Positioning

```text
Application Layer      -> AI Apps / Agents / Workflows
Runtime Layer          -> LangGraph / AutoGen / Custom Orchestrators
Memory Layer           -> Aetheris MemOS
Model Layer            -> OpenAI / Ollama / vLLM / Azure OpenAI
Infrastructure Layer   -> Postgres / Qdrant / Neo4j / Object Storage
```

MemOS is not another demo chatbot. It is the memory substrate under agent systems.

## Vision

We believe the future of AI is not just better models, but better memory systems.

As models become cheaper and more interchangeable, durable advantage shifts upward into agent runtime, memory coherence, and long-horizon learning. The winner is not the system with the biggest prompt. It is the system that remembers correctly, retrieves selectively, explains its choices, and keeps improving.

Aetheris MemOS is built around that thesis.

## Architecture

> Full interactive diagram: [docs/architecture.drawio](docs/architecture.drawio)

```mermaid
flowchart TD
    A["**① Clients & SDKs**
    React Frontend · Python SDK · Rust SDK
    Anthropic / OpenAI / LangChain · External AI Agents"]

    B["**② Protocol / Transport**
    HTTP/REST (Axum + TLS) · gRPC · WebSocket · MCP (Sandbox + Signing)"]

    C["**③ Auth & Middleware**
    JWT Auth · Rate Limiting · CORS · HTTP Trace · RBAC"]

    D["**④ API Routes**
    /v1/memory · /kg · /mm · /v1/agents · /v1/distributed
    /v1/planner · /v1/security · /v1/workflows · /tenants"]

    E1["Memory Intelligence
    Scheduler · Analyzer · Predictor · Monitor
    Cost Model · Model Router · Weight Adjuster"]

    E2["Memory Processing Pipeline
    Transfer · Ingestion Daemon · Hybrid Search · Fusion
    Embedding · LLM · Rerank · Context Compressor"]

    E3["Security & Integrity
    Prompt Probe · Info Guard · Evidence Graph
    Self-Healing · Vector Guard · Usage Tracker"]

    F["**⑥ Memory Layer Abstractions**
    STM · LTM (Bi-temporal) · KG · MM · Procedural (GraphRAG) · Memory Pool"]

    G1["Runtime & Kernel
    Planner Sandbox · Subagent Pool · Approval Node (HITL) · Fan Nodes"]

    G2["Distributed System
    Consensus · Replication · Sharding · Epoch Manager
    Signaling Bus · Tenant Isolation"]

    H["**⑧ Data Persistence**
    PostgreSQL / SQLite · Qdrant (vector) · Neo4j (graph) · Event Store"]

    I["**⑨ Observability**
    OpenTelemetry · Prometheus"]

    A --> B --> C --> D
    D --> E1 & E2 & E3
    E1 & E2 & E3 --> F
    F --> G1 & G2
    G1 & G2 --> H
    E1 --> I

    style E1 fill:#d5e8d4,stroke:#82b366,color:#1a1a1a
    style E2 fill:#d5e8d4,stroke:#82b366,color:#1a1a1a
    style E3 fill:#f8cecc,stroke:#b85450,color:#1a1a1a
    style F fill:#fff2cc,stroke:#d6b656,color:#1a1a1a
```

## Core Capabilities

### Multi-layer memory kernel

| Layer | Purpose | Backend |
|-------|---------|---------|
| STM | Active conversational and task context | In-process + relational session storage |
| LTM | Durable memory with retrieval metadata | PostgreSQL + Qdrant |
| KG | Entity and relation reasoning with temporal state | PostgreSQL and optional Neo4j |
| MM | Cross-modal memory for non-text artifacts | PostgreSQL + vector indexing |

### Adaptive scheduling

Each task is profiled before memory is selected.

```text
Task
 -> complexity
 -> modality
 -> reasoning depth
 -> context dependency
 -> temporal sensitivity
 -> weighted memory plan
 -> decision trace
```

This allows MemOS to decide when a simple session lookup is enough, when long-term semantic recall matters, and when graph reasoning or multimodal expansion should be used.

### Triple hybrid retrieval

MemOS now supports three fused retrieval modes in a single pipeline:

```text
semantic vector search
+ keyword / BM25 search
+ graph neighborhood traversal
= final fused ranking
```

Endpoint:

```text
POST /api/v1/memory/search/triple
```

### Confidence scoring

Search results can be enriched with multi-dimensional confidence metadata.

| Dimension | Signal |
|-----------|--------|
| Quality | Stored quality score |
| Relevance | Retrieval ranking score |
| Recency | Time-decay adjusted freshness |
| Access | Frequency-normalized usage |
| Completeness | Content coverage heuristic |

Endpoint:

```text
POST /api/v1/memory/search/scored
```

### Context compression

MemOS compresses session context before it reaches the model budget.

| Strategy | Description |
|----------|-------------|
| sliding_window | Keep only the most recent messages |
| importance_prune | Drop low-value context first |
| llm_summary | Summarize accumulated context into one message |
| hierarchical | Summarize older context, preserve recent turns |

Endpoints:

```text
POST /api/v1/memory/storage/compress/session
POST /api/v1/memory/storage/compress/messages
```

### Enterprise multi-tenancy

- Tenant isolation via explicit tenant IDs
- Role-based access control: Member, Admin, SuperAdmin
- Cross-agent search within a tenant boundary
- Shared knowledge configuration for controlled read access
- Quota enforcement for LTM and session usage

## What Is Already Implemented

Recent work completed in this repository includes:

- Unified DB pool with SQLite and PostgreSQL support
- SQLite WAL optimization and async write queue
- Hardware detection and model routing for CUDA, Metal, and Apple Silicon
- Vector space collapse protection through model signature checking
- Proactive memory ingestion and reflection daemon
- Bi-temporal knowledge graph with snapshots, diffs, and contradiction detection
- Triple hybrid search
- Multi-dimensional confidence scoring
- Intelligent context compression
- Adaptive strategy mutation
- Enterprise multi-tenant isolation
- Integrity protection and silent information loss guards

## Repository Layout

```text
adaptive-memory-system/
|- backend/
|  |- src/
|  |  |- routers/
|  |  |- services/
|  |  |- db/
|  |  |- models/
|  |  |- config/
|  |  `- hoops/
|  |- migrations/
|  |- examples/
|  `- docs/
|- frontend/
|  `- ant-design-pro-template/
|- docs/
|- sdks/
|  |- python/
|  `- rust/
`- docker-compose.yml
```

## Quick Start

### Prerequisites

| Dependency | Version | Notes |
|------------|---------|-------|
| Rust | 1.89+ | Required |
| Node.js | 20+ | Required for frontend |
| PostgreSQL | 14+ | Optional if using SQLite mode |
| Qdrant | 1.7+ | Required for vector search features |
| Neo4j | 5.x | Optional for graph deployment mode |
| Ollama | latest | Optional for local embeddings and LLM calls |

### Option A: backend only with SQLite

```bash
git clone https://github.com/Colin4k1024/adaptive-memory-system.git
cd adaptive-memory-system/backend
cargo run
```

Available after startup:

- API: http://127.0.0.1:8008
- Docs: http://127.0.0.1:8008/scalar

### Option B: full stack with Docker Compose

```bash
git clone https://github.com/Colin4k1024/adaptive-memory-system.git
cd adaptive-memory-system
docker-compose up -d
```

Services:

- Backend: http://localhost:8008
- Frontend: http://localhost:8000
- Qdrant: http://localhost:6333

### Option C: local backend + local frontend

```bash
cd backend
cp config.toml.example config.toml
cargo run
```

In another terminal:

```bash
cd frontend/ant-design-pro-template
npm install
npm start
```

## API Documentation

Interactive API docs are available at:

```text
http://127.0.0.1:8008/scalar
```

Key endpoints:

| Endpoint | Method | Description |
|----------|--------|-------------|
| /api/v1/memory/adaptive | POST | Adaptive memory selection |
| /api/v1/memory/search/ltm | POST | Long-term memory search |
| /api/v1/memory/search/hybrid | POST | Vector + keyword hybrid search |
| /api/v1/memory/search/triple | POST | Triple hybrid retrieval |
| /api/v1/memory/search/scored | POST | Retrieval with confidence scoring |
| /api/v1/memory/storage/stm | POST | Write short-term memory |
| /api/v1/memory/storage/ltm | POST | Write long-term memory |
| /api/v1/memory/storage/transfer | POST | Promote STM into LTM |
| /api/v1/memory/storage/compress/session | POST | Compress one session |
| /api/v1/memory/storage/compress/messages | POST | Compress arbitrary messages |
| /api/kg/entities | GET | List knowledge graph entities |
| /api/tenants | GET, POST | Tenant management |

More details are available in [docs/API_ENDPOINTS.md](https://github.com/Colin4k1024/adaptive-memory-system/blob/dev/docs/API_ENDPOINTS.md).

## Configuration

Core runtime configuration lives in backend/config.toml.

```toml
listen_addr = "127.0.0.1:8008"

[db]
url = "sqlite://data/memory.db"

[llm]
base_url = "http://localhost:11434"
model = "llama3"

[embedding]
base_url = "http://localhost:11434"
model = "nomic-embed-text"
dimension = 768
auto_detect = true

[qdrant]
url = "http://localhost:6333"

[rerank]
enabled = false
min_score_threshold = 0.1
candidate_multiplier = 3
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Backend | Rust + Axum + Tokio |
| Relational storage | PostgreSQL / SQLite via SQLx |
| Vector storage | Qdrant |
| Graph storage | Neo4j optional integration |
| Embeddings / LLM | Ollama or compatible model endpoints |
| API docs | OpenAPI + Scalar |
| Frontend | React + Umi + Ant Design Pro |

## Product Roadmap

### Phase 1: Foundation

- [x] Unified DB pool
- [x] Hardware detection and model routing
- [x] SQLite concurrency optimization
- [x] Integrity guards and vector safety

### Phase 2: Intelligent Memory

- [x] Reflection daemon and layered ingestion
- [x] Bi-temporal knowledge graph
- [x] Triple hybrid search

### Phase 3: Reliable Retrieval

- [x] Confidence scoring
- [x] Context compression
- [x] Silent information loss protection

### Phase 4: Scale and Governance

- [x] Adaptive strategy mutation
- [x] Enterprise multi-tenancy

### Phase 5: Ecosystem

- [ ] MemOS protocol for agent interoperability
- [ ] LangGraph and AutoGen adapters
- [ ] Distributed memory federation
- [ ] Hosted Aetheris Cloud control plane

## Aetheris Product Family

The long-term structure is larger than a single repository.

| Product | Role |
|---------|------|
| Aetheris MemOS | Memory operating system for agents |
| Aetheris Runtime | Agent execution runtime |
| Aetheris Graph | Graph-native knowledge substrate |
| Aetheris Control | Governance, observability, and policy plane |
| Aetheris Cloud | Managed hosted platform |

## Development

Backend:

```bash
cd backend
cargo build
cargo test
cargo fmt
cargo clippy
```

Frontend:

```bash
cd frontend/ant-design-pro-template
npm install
npm start
npm run build
npm run lint
```

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md), [CONTRIBUTING.zh.md](CONTRIBUTING.zh.md), and [SECURITY.md](SECURITY.md).

## License

MIT. See [LICENSE](LICENSE).

---

Built for the agentic future.
