# Project Map

## Repository Structure

```
adaptive-memory-system/
├── backend/                    # Rust (Axum) API server
│   ├── src/
│   │   ├── routers/           # API endpoint handlers
│   │   │   ├── memory.rs      # Memory endpoints
│   │   │   ├── auth.rs        # Authentication
│   │   │   ├── user.rs        # User management
│   │   │   ├── knowledge_graph.rs
│   │   │   ├── memory_search.rs
│   │   │   ├── memory_storage.rs
│   │   │   └── multimodal.rs
│   │   ├── services/          # Core business logic
│   │   │   ├── scheduler.rs   # Adaptive memory scheduler
│   │   │   ├── analyzer.rs     # Task feature analysis
│   │   │   ├── predictor.rs    # Performance prediction
│   │   │   ├── monitor.rs      # Resource monitoring
│   │   │   ├── weight_adjuster.rs
│   │   │   ├── weight_strategy.rs
│   │   │   ├── agent.rs        # Memory agents
│   │   │   ├── embedding.rs     # Ollama embeddings
│   │   │   ├── llm.rs           # Ollama LLM
│   │   │   ├── memory_search.rs
│   │   │   ├── memory_storage.rs
│   │   │   ├── memory_transfer.rs
│   │   │   ├── qdrant.rs        # Vector DB
│   │   │   └── rerank.rs
│   │   ├── db/                # Database repositories
│   │   │   ├── memory.rs
│   │   │   ├── performance.rs
│   │   │   ├── weights.rs
│   │   │   ├── stm.rs
│   │   │   ├── ltm.rs
│   │   │   ├── kg.rs
│   │   │   ├── mm.rs
│   │   │   ├── neo4j.rs
│   │   │   └── decision_trace.rs
│   │   ├── models/            # Data models
│   │   ├── config/
│   │   ├── hoops/             # Middleware (CORS, JWT)
│   │   └── error.rs           # AppError
│   └── Cargo.toml
│
├── frontend/ant-design-pro-template/  # React frontend
│   ├── src/
│   │   ├── pages/             # Umi pages
│   │   │   ├── Dashboard/
│   │   │   ├── TaskAnalysis/
│   │   │   ├── MemoryConfig/
│   │   │   ├── MemoryDecisionTrace/
│   │   │   ├── MemoryDetails/
│   │   │   ├── MemoryManagement/
│   │   │   ├── Performance/
│   │   │   ├── ResourceMonitor/
│   │   │   └── WeightHistory/
│   │   ├── services/          # API clients
│   │   │   └── memory/
│   │   │       ├── storageApi.ts
│   │   │       ├── knowledgeGraphApi.ts
│   │   │       └── multimodalApi.ts
│   │   └── models/
│   └── package.json
│
├── docs/                      # Documentation
├── sdks/rust/                 # Rust SDK
└── .github/workflows/         # CI/CD
```

## Key Dependencies

### Backend
- **Web**: Axum 0.8, tower-http
- **Database**: SQLx (PostgreSQL, SQLite), Qdrant (vector), Neo4j (graph)
- **AI**: langchain-rust (Ollama), embedding models
- **Auth**: JWT (jsonwebtoken), argon2

### Frontend
- **Framework**: Umi 4, Ant Design Pro 6.0
- **State**: React hooks
- **API**: Axios/fetch to backend

## Common Commands

### Backend
```bash
cd backend
cargo build
cargo run        # http://127.0.0.1:8008
cargo test
cargo fmt
cargo clippy
```

### Frontend
```bash
cd frontend/ant-design-pro-template
npm install --legacy-peer-deps
npm start       # http://localhost:8000
npm run build
npm run lint
```

## Environment Setup

Requires:
- Rust 1.89+
- Node.js 20+
- PostgreSQL 14+ (Docker)
- Qdrant (Docker)
- Neo4j (optional)
