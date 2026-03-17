# Roadmap

This project evolves in **architecture-first iterations**. Each version focuses on clarity, extensibility, and agent-readiness.

See [why-axum.md](why-axum.md) for web framework migration notes and [ARCHITECTURE.md](ARCHITECTURE.md) for design.

---

## v0.2 — Stable Rule-Based Adaptive Memory (Completed)

- **Backend**: Rule-based scheduler, analyzer, predictor, monitor, weight adjuster; PostgreSQL + Qdrant baseline; REST API.
- **Frontend**: Dashboard, task analysis, memory config, performance, resource monitor, weight history.
- **Docs**: Algorithm design, API spec, system visualization.

---

## v0.3 — Extensible & Agent-Ready (Completed)

**Theme:** Make the system composable, explainable, and open for extension.

### Done

- **Agent-oriented core** — `MemoryAgent` trait (observe → decide → act); Analyzer, Predictor, and Scheduler implement it. See [ARCHITECTURE.md](ARCHITECTURE.md).
- **Strategy plugin system** — `WeightStrategy` trait; built-in strategies (MarginalBenefit, LinearDecay, SynergyAware); weight adjuster composes strategies. See [EXTENSION_GUIDE.md](EXTENSION_GUIDE.md).
- **Decision trace (API + UI + persistence)** — `POST /api/v1/memory/adaptive/trace` and Memory Decision Trace page for step-by-step pipeline inspection (analyzer → predictor → weight adjustment → result), with DB persistence support.
- **Storage baseline alignment** — PostgreSQL as relational baseline, Qdrant for vectors, Neo4j optional for graph scenarios.
- **Documentation** — ARCHITECTURE, ROADMAP, USE_CASES, why-axum; CONTRIBUTING and EXTENSION_GUIDE.
- **Axum backend migration** — Backend now runs on Axum; keep API compatibility and continue ecosystem alignment (see [why-axum.md](why-axum.md)).

### Phase 2: Cognitive Architecture (Completed)

- **Dynamic importance scoring** — Multi-factor importance evaluator with LLM-as-a-Judge support
- **Fractal decay mechanism** — Bio-inspired adaptive forgetting with tier-specific decay rates
- **Consolidation pipeline** — Sleep-like memory consolidation with compression and restructuring
- **Bi-temporal tracking** — Version history and time-travel queries for LTM and KG

### Phase 3: Ecosystem Integration (Completed)

- **Oris integration** — Context snapshots, task persistence, checkpoints and rollback
- **Aetheris multi-tenant** — Tenant management, quota control, RBAC (Owner/Admin/Member/Reader)
- **Memory pool** — Multi-agent collaborative memory sharing with Private/Shared/Public visibility

### Phase 4: Enterprise & Commercialization (Completed)

- **Enterprise cluster** — Node registration, leader election, data sharding
- **Visualization APIs** — Timeline, graph, heatmap, dashboard endpoints for frontend widgets
- **Billing system** — Usage tracking, quota management, resource metering

### Planned

- **Observability** — trace_id, decision span, OpenTelemetry-compatible export; metrics correlation.
- **Repository adapter trait** — Abstract persistence behind traits; runtime adapter selection (optional).

---

## v0.4 (Planned)

---

## v0.5 — Memory Kernel 正式版 (Q1)

**Theme:** Unified Memory Kernel Architecture

### Architecture Modules

| Module        | Location                                      | Status      |
| ------------- | --------------------------------------------- | ----------- |
| Memory Kernel | `src/kernel/` (traits.rs, types.rs, error.rs) | Implemented |
| Memory Layers | `src/layers/` (stm, ltm, kg, mm)              | Implemented |
| Policy Engine | `src/policy/` (scheduler.rs, cost_model.rs)   | Implemented |

### Planned

- **Kernel integration** — Refactor scheduler to use unified kernel interface
- **Redis STM cache** — Replace in-memory STM with Redis backend
- **Qdrant integration** — Vector search for LTM layer
- **Neo4j integration** — Graph queries for KG layer

---

## v0.6 — Agent Runtime Integration (Q2)

**Theme:** Native Agent Runtime SDK

### Architecture Modules

| Module           | Location                                     | Status      |
| ---------------- | -------------------------------------------- | ----------- |
| Memory Agent     | `src/agent/` (compressor, merger, forgetter) | Implemented |
| Runtime Adapters | `src/runtime/` (openai, anthropic)           | Implemented |

### Planned

- **OpenAI Agents SDK** — Complete adapter implementation
- **Anthropic Claude** — Complete adapter implementation
- **LangChain adapter** — New adapter for LangChain ecosystem
- **LLM compression** — Smart summarization for STM→LTM

---

## v0.7 — Production API Gateway (Q3)

**Theme:** Enterprise-Ready APIs

### Architecture Modules

| Module       | Location                                  | Status      |
| ------------ | ----------------------------------------- | ----------- |
| Protocol     | `src/protocol/` (grpc, websocket)         | Implemented |
| Multi-Tenant | `src/tenant/` (context, quota, isolation) | Implemented |

### Planned

- **gRPC service** — tonic-based gRPC API
- **WebSocket** — Real-time memory subscriptions
- **Multi-Tenant** — Complete tenant isolation
- **Authentication** — JWT + API Key auth middleware

---

## v0.8 — Distributed Cluster (Q4)

**Theme:** Horizontally Scalable Memory

### Architecture Modules

| Module      | Location                                                    | Status      |
| ----------- | ----------------------------------------------------------- | ----------- |
| Distributed | `src/distributed/` (node, replication, sharding, consensus) | Implemented |

### Planned

- **Node discovery** — Cluster membership and heartbeats
- **Replication** — Multi-replica sync
- **Sharding** — Consistent hash-based sharding
- **Consensus** — Leader election (Raft)

---

## v1.0 — Agent Memory OS (Next Year Q1)

**Theme:** Production-Ready Memory Operating System

| Feature           | Status                     |
| ----------------- | -------------------------- |
| Memory Kernel     | Production-ready           |
| Agent Integration | OpenAI/Anthropic/LangChain |
| Protocol          | gRPC/REST/WS + Auth        |
| Multi-Tenant      | Full tenant isolation      |
| Distributed       | Cluster support            |
| Observability     | Prometheus + Tracing       |

---

## Database Adapters

- **PostgreSQL** — Current relational baseline for configs/metrics/traces and memory metadata.
- **Qdrant** — Current vector search backend for LTM.
- **Neo4j** — Optional graph backend (advanced KG scenarios).
- **Redis** — STM cache layer (planned v0.5).
- **MySQL** — Alternative relational adapter (planned).
