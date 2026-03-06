# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Adaptive Memory Management System for AI Agent & LLM workloads. Uses Rust (Salvo) backend with React (Ant Design Pro) frontend.

## Commands

### Backend (Rust)
```bash
cd backend
cargo build              # Build the project
cargo run                # Run development server on http://127.0.0.1:8008
cargo test               # Run tests
cargo test <test_name>   # Run a specific test
```

### Frontend (React)
```bash
cd frontend/ant-design-pro-template
npm install              # Install dependencies
npm start                 # Run dev server on http://localhost:8000
npm test                 # Run tests
npm run build            # Production build
npm run lint             # Lint code
```

## Architecture

This is a **monorepo** with two main components:

### Backend (`backend/src/`)
- **routers/** — API endpoint handlers (memory, auth, user, knowledge_graph, memory_search, memory_storage, multimodal)
- **services/** — Core business logic
  - `scheduler.rs` — Adaptive memory scheduler (selects optimal memory config)
  - `analyzer.rs` — Task feature analysis (complexity, modality, reasoning depth)
  - `predictor.rs` — Performance prediction model
  - `monitor.rs` — Resource monitoring
  - `weight_adjuster.rs` — Dynamic weight adjustment
  - `weight_strategy.rs` — Pluggable weight strategies
  - `agent.rs` — Memory agents (implements MemoryAgent trait)
  - `embedding.rs` — Embedding model service (Ollama)
  - `llm.rs` — LLM service (Ollama)
  - `memory_search.rs` — Memory search (semantic, keyword, hybrid)
  - `memory_storage.rs` — Memory storage management
  - `memory_transfer.rs` — Memory transfer (STM → LTM)
  - `qdrant.rs` — Qdrant vector database client
  - `rerank.rs` — Reranking service
- **db/** — Database repositories
  - `memory.rs` — Memory configuration
  - `performance.rs` — Performance metrics
  - `weights.rs` — Weight history
  - `stm.rs` — Short-term memory
  - `ltm.rs` — Long-term memory
  - `kg.rs` — Knowledge graph
  - `mm.rs` — Multimodal memory
  - `neo4j.rs` — Neo4j graph database
  - `decision_trace.rs` — Decision trace
- **models/** — Data models (memory, task, performance, resource)
- **config/** — Configuration modules
- **hoops/** — Middleware (CORS, JWT auth)

### Frontend (`frontend/ant-design-pro-template/`)
- Uses Umi 4 + Ant Design Pro 6.0
- Pages in `src/pages/`: Dashboard, TaskAnalysis, MemoryConfig, MemoryDecisionTrace, MemoryManagement, Performance, ResourceMonitor, WeightHistory

## Key Patterns

### Adding New API Endpoints
1. Add handler in `backend/src/routers/memory.rs`
2. Use `#[handler]` macro
3. Register route in `backend/src/routers/mod.rs`

### Adding New Weight Strategies
Implement `WeightStrategy` trait and add to the adjuster chain. See `backend/src/services/weight_strategy.rs`.

### Adding New Memory Agents
Implement `MemoryAgent` trait for custom analyzer/predictor/scheduler behavior. See `backend/src/services/agent.rs`.

### Database Operations
Use SQLx with compile-time checks:
```rust
sqlx::query!("SELECT * FROM table WHERE id = $1", id)
sqlx::query_as!(Model, "SELECT * FROM table")
```

### Error Handling
Use `AppError` from `backend/src/error.rs` for structured error responses with proper HTTP status codes.

## Environment Requirements

- Rust: 1.89+
- Node.js: 20+
- PostgreSQL 14+ (via Docker)
- Qdrant (via Docker)
- Neo4j (optional, for knowledge graph)
