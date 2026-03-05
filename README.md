# Adaptive Memory Management System

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Backend CI](https://github.com/Colin4k1024/adaptive-memory-system/actions/workflows/ci.yml/badge.svg)](https://github.com/Colin4k1024/adaptive-memory-system/actions)
[![Rust Version](https://img.shields.io/badge/Rust-1.89+-blue.svg)](https://www.rust-lang.org)
[![Node Version](https://img.shields.io/badge/Node-20+-green.svg)](https://nodejs.org)

**Adaptive Memory Management System for Agent & LLM Workloads**

Built on adaptive memory management algorithm design, featuring a Rust (Salvo) backend API service and React (Ant Design Pro) frontend management interface. This project is open-sourced under MIT license. Contributions and forks are welcome.

- **License**: [LICENSE](LICENSE) (MIT)
- **Security**: For vulnerability reporting, see [SECURITY.md](SECURITY.md)
- **Changelog**: [CHANGELOG.md](CHANGELOG.md)
- **Documentation**: [docs/](docs/)

## Table of Contents

- [Project Structure](#project-structure)
- [Tech Stack](#tech-stack)
- [Core Features](#core-features)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [API Documentation](#api-documentation)
- [Deployment](#deployment)
- [Configuration](#configuration)
- [Development](#development)
- [License](#license)

## Project Structure

```
adaptive-memory-system/
├── backend/                         # Rust + Salvo backend service
│   ├── src/
│   │   ├── db/                    # Database operations
│   │   │   ├── memory.rs           # Memory config repository
│   │   │   ├── stm.rs              # Short-term memory
│   │   │   ├── ltm.rs               # Long-term memory
│   │   │   ├── kg.rs                # Knowledge graph
│   │   │   ├── mm.rs                # Multimodal memory
│   │   │   ├── performance.rs        # Performance metrics
│   │   │   └── weights.rs           # Weight history
│   │   ├── services/                # Core services
│   │   │   ├── scheduler.rs          # Adaptive scheduler
│   │   │   ├── analyzer.rs           # Task analyzer
│   │   │   ├── predictor.rs          # Performance prediction
│   │   │   ├── monitor.rs            # Resource monitor
│   │   │   ├── memory_storage.rs     # Memory storage
│   │   │   ├── memory_search.rs      # Memory search
│   │   │   └── memory_transfer.rs    # Memory transfer
│   │   ├── layers/                   # Memory layers
│   │   │   ├── stm_layer.rs         # STM layer
│   │   │   ├── ltm_layer.rs          # LTM layer
│   │   │   ├── kg_layer.rs           # KG layer
│   │   │   └── mm_layer.rs           # MM layer
│   │   ├── routers/                 # API routes
│   │   ├── kernel/                   # Core traits & types
│   │   ├── policy/                   # Policy & scheduling
│   │   ├── agent/                    # Memory agents
│   │   └── main.rs                   # Entry point
│   ├── migrations/                   # Database migrations
│   └── Cargo.toml
├── frontend/                         # React + Ant Design Pro
│   └── ant-design-pro-template/
│       ├── src/
│       │   ├── pages/               # Page components
│       │   │   ├── Dashboard/       # Dashboard
│       │   │   ├── MemoryConfig/    # Memory configuration
│       │   │   ├── MemoryDecisionTrace/  # Decision trace
│       │   │   ├── MemoryManagement/ # Memory management
│       │   │   ├── Performance/      # Performance analysis
│       │   │   ├── ResourceMonitor/ # Resource monitoring
│       │   │   ├── TaskAnalysis/    # Task analysis
│       │   │   └── WeightHistory/    # Weight history
│       │   ├── services/            # API services
│       │   ├── utils/               # Utility functions
│       │   └── config/              # Configuration
│       └── package.json
├── docs/                            # Documentation
├── .github/workflows/               # CI/CD pipelines
└── docker/                         # Docker configurations
```

## Tech Stack

### Backend
| Component | Technology |
|-----------|------------|
| Framework | Salvo 0.84 |
| Language | Rust 1.89+ |
| Runtime | Tokio |
| Database | PostgreSQL, Neo4j, Qdrant |
| Serialization | Serde |
| Logging | Tracing |
| Authentication | JWT |
| API Docs | OpenAPI |

### Frontend
| Component | Technology |
|-----------|------------|
| Framework | React 19+ |
| UI Library | Ant Design Pro 6.0 |
| Charts | @ant-design/charts |
| Build Tool | Umi 4 |
| State | Umi Max |

## Core Features

### 1. Four-Layer Memory Architecture

| Layer | Description | Storage |
|-------|-------------|---------|
| **STM** | Short-Term Memory | In-Memory HashMap |
| **LTM** | Long-Term Memory | PostgreSQL + Qdrant |
| **KG** | Knowledge Graph | PostgreSQL / Neo4j |
| **MM** | Multimodal Memory | PostgreSQL + Qdrant |

### 2. Adaptive Memory Scheduling
- Automatically selects optimal memory configuration based on task characteristics
- 10-step decision process: Analysis → Prediction → Weight Adjustment → Selection
- Dynamic weight adjustment mechanism
- Decision trace for explainability

### 3. Task Characteristic Analysis
- Complexity assessment
- Modality requirement detection
- Reasoning depth evaluation
- Context dependency analysis

### 4. Performance Prediction
- Research-based performance baselines
- Diminishing marginal returns compensation
- Synergy calculation between memory types
- Resource cost estimation

### 5. Resource Monitoring
- Real-time CPU, memory, storage monitoring
- Cost-benefit analysis
- Auto-generated alerts
- Optimization suggestions

### 6. NER (Named Entity Recognition)
- LLM-based entity extraction
- Entity typing (PERSON, ORG, LOC, EVENT, etc.)
- Relation extraction

### 7. Security
- JWT authentication
- Rate limiting (sliding window algorithm)
- Input validation

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Frontend (React)                         │
│  Dashboard | MemoryConfig | Performance | ResourceMonitor │
└─────────────────────────┬───────────────────────────────────┘
                          │ HTTP/HTTPS
┌─────────────────────────▼───────────────────────────────────┐
│                    Backend (Salvo)                           │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │   Routers    │  │   Services   │  │   Kernel      │  │
│  │  API Layer   │  │  Business    │  │  Traits       │  │
│  └──────────────┘  └──────────────┘  └────────────────┘  │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │   Policy     │  │    Agent     │  │    Layers      │  │
│  │  Scheduling  │  │   Memory     │  │  STM/LTM/KG/MM │  │
│  └──────────────┘  └──────────────┘  └────────────────┘  │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────────┐
│                      Database Layer                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐  │
│  │PostgreSQL │  │  Neo4j   │  │  Qdrant  │  │ SQLite  │  │
│  │ Relations │  │   Graph   │  │  Vector  │  │   Local │  │
│  └──────────┘  └──────────┘  └──────────┘  └─────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### Environment Requirements

| Component | Version |
|----------|---------|
| Rust | 1.89+ |
| Node.js | 20+ |
| PostgreSQL | 14+ (optional) |
| Qdrant | 1.7+ (optional) |
| Neo4j | 5.x (optional) |

### Backend

```bash
cd backend

# Build
cargo build

# Run
cargo run

# Server starts at http://127.0.0.1:8008
```

### Frontend

```bash
cd frontend/ant-design-pro-template

# Install dependencies
npm install

# Development
npm start

# Production build
npm run build

# App runs at http://localhost:8000
```

### Docker

```bash
# Using Docker Compose
docker-compose up -d

# Or build manually
docker build -t adaptive-memory-backend ./backend
docker build -t adaptive-memory-frontend ./frontend
```

## API Documentation

### Base URL
```
http://127.0.0.1:8008
```

### Authentication

```bash
# Login
POST /api/login
{
  "username": "admin",
  "password": "admin"
}

# Response
{
  "token": "eyJhbGciOiJIUzI1NiIs..."
}
```

### Core Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/memory/adaptive` | Adaptive memory selection |
| POST | `/api/v1/memory/analyzer/task-characteristics` | Task analysis |
| POST | `/api/v1/memory/predictor/performance` | Performance prediction |
| GET | `/api/v1/memory/monitor/resources` | Resource monitoring |
| POST | `/api/v1/memory/storage/ltm` | Store LTM |
| POST | `/api/v1/memory/search/hybrid` | Hybrid search |
| POST | `/api/v1/memory/kg/entities` | Create entity |
| POST | `/api/v1/memory/mm/store` | Store multimodal |

See [docs/API_ENDPOINTS.md](docs/API_ENDPOINTS.md) for complete API documentation.

### OpenAPI Docs

- Scalar UI: `http://127.0.0.1:8008/scalar`
- OpenAPI JSON: `http://127.0.0.1:8008/api-doc/openapi.json`

## Deployment

### Prerequisites

1. PostgreSQL database (for production)
2. Qdrant (for vector search)
3. Neo4j (optional, for knowledge graph)

### Configuration

Edit `backend/config.toml`:

```toml
listen_addr = "0.0.0.0:8008"

[db]
url = "postgres://user:password@localhost:5432/memory"

[jwt]
secret = "your-secret-key"
expiry = 3600

[llm]
base_url = "http://localhost:11434"
model = "llama3"
```

### Production Build

```bash
# Backend
cd backend
cargo build --release
./target/release/adaptive-memory

# Frontend
cd frontend/ant-design-pro-template
npm run build
```

### Docker Deployment

```bash
docker-compose -f docker-compose.prod.yml up -d
```

## Configuration

### Backend Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `listen_addr` | Server address | `127.0.0.1:8008` |
| `db.url` | Database URL | SQLite |
| `jwt.secret` | JWT secret | - |
| `jwt.expiry` | Token expiry (seconds) | 3600 |
| `llm.base_url` | Ollama API URL | `http://localhost:11434` |
| `llm.model` | LLM model | `llama3` |

### Rate Limiting

Default: 100 requests per minute
Configurable in routers.

## Development

### Running Tests

```bash
# Backend
cd backend
cargo test

# Frontend
cd frontend/ant-design-pro-template
npm test
```

### Code Quality

```bash
# Backend
cd backend
cargo fmt
cargo clippy

# Frontend
cd frontend/ant-design-pro-template
npm run lint
```

### CI/CD

GitHub Actions workflows are in `.github/workflows/`:
- `ci.yml` - Full CI pipeline
- `backend-ci.yml` - Backend-specific CI
- `frontend-ci.yml` - Frontend-specific CI

## License

MIT License - see [LICENSE](LICENSE) for details.

---

For more documentation, see the [docs/](docs/) directory.
