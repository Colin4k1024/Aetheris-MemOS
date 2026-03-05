# Adaptive Memory Management System

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Adaptive Memory Management System for Agent & LLM Workloads**

Built on adaptive memory management algorithm design, featuring a Rust (Salvo) backend API service and React (Ant Design Pro) frontend management interface. This project is open-sourced under MIT license. Contributions and forks are welcome.

- **License**: [LICENSE](LICENSE) (MIT)
- **Security**: For vulnerability reporting, see [SECURITY.md](SECURITY.md)
- **Changelog**: [CHANGELOG.md](CHANGELOG.md)

## Project Structure

```
adaptive-memory-system/
├── backend/                    # Rust + Salvo backend service
│   ├── src/
│   │   ├── db/                # Database operations module
│   │   │   ├── memory.rs      # Memory configuration repository
│   │   │   ├── performance.rs # Performance metrics repository
│   │   │   └── weights.rs     # Weight history repository
│   │   ├── services/          # Core service layer
│   │   │   ├── scheduler.rs   # Adaptive memory scheduler
│   │   │   ├── analyzer.rs    # Task characteristic analyzer
│   │   │   ├── predictor.rs   # Performance prediction model
│   │   │   ├── monitor.rs     # Resource monitor
│   │   │   └── weight_adjuster.rs # Dynamic weight adjuster
│   │   ├── routers/           # API route handlers
│   │   │   └── memory.rs      # Memory management endpoints
│   │   ├── models/            # Data models
│   │   └── main.rs            # Application entry point
│   ├── migrations/            # Database migration files
│   └── Cargo.toml             # Rust dependencies
├── frontend/                   # React + Ant Design Pro frontend
│   └── ant-design-pro-template/
│       ├── src/
│       │   ├── pages/         # Page components
│       │   │   ├── Dashboard/ # Dashboard
│       │   │   ├── TaskAnalysis/ # Task analysis
│       │   │   ├── MemoryConfig/ # Memory configuration
│       │   │   ├── Performance/ # Performance monitoring
│       │   │   ├── ResourceMonitor/ # Resource monitoring
│       │   │   └── WeightHistory/ # Weight history
│       │   └── services/      # API service wrappers
│       └── package.json       # Frontend dependencies
└── docs/                       # Design documents
    ├── adaptive_memory_algorithm_design.md
    ├── adaptive_memory_api_specification.md
    └── adaptive_memory_algorithm_visualization.md
```

## Tech Stack

### Backend
- **Framework**: Salvo 0.84
- **Language**: Rust 1.89+
- **Database**: SQLite (using SQLx)
- **Async Runtime**: Tokio
- **Serialization**: Serde
- **Logging**: Tracing
- **Authentication**: JWT (jsonwebtoken)
- **Configuration**: Figment
- **Database**: SQLite as default adapter (local & demo); PostgreSQL/MySQL adapters planned.

### Frontend
- **Framework**: React 19+
- **UI Library**: Ant Design Pro 6.0
- **Chart Library**: @ant-design/charts
- **Build Tool**: Umi 4
- **State Management**: Umi Max
- Frontend designed for visualizing rule-based and (future) LLM-driven memory agents.

## Core Features

### 1. Adaptive Memory Scheduling
- Automatically selects optimal memory configuration based on task characteristics and resource constraints
- Supports Short-Term Memory (STM), Long-Term Memory (LTM), Knowledge Graph (KG), and Multimodal Memory (MM)
- Dynamic weight adjustment mechanism

### 2. Task Characteristic Analysis
- Complexity assessment
- Modality requirement detection
- Reasoning depth evaluation
- Context dependency analysis

### 3. Performance Prediction
- Research-based performance benchmarks
- Diminishing marginal returns compensation
- Synergy calculation
- Resource cost estimation

### 4. Resource Monitoring
- Real-time resource usage monitoring
- Cost-benefit analysis
- Resource optimization suggestions
- Alert mechanisms

### 5. Weight Adjustment History
- Records all weight adjustment operations
- Performance impact tracking
- Trend analysis

## Quick Start

### Environment Requirements

- Rust: 1.89+
- Node.js: 20+
- SQLite 3.x

### Backend Development

```bash
cd backend

# Install dependencies (first run)
cargo build

# Run development server
cargo run

# Server will start at http://127.0.0.1:8008
```

### Frontend Development

```bash
cd frontend/ant-design-pro-template

# Install dependencies (first run)
npm install

# Start development server
npm start

# Application will start at http://localhost:8000
```

## API Usage Examples

### 1. Adaptive Memory Selection

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/adaptive" \
  -H "Content-Type: application/json" \
  -d '{
    "task_context": {
      "task_id": "task_001",
      "task_type": "query",
      "complexity": 0.75,
      "modality_requirements": ["text", "image"],
      "temporal_scope": "medium",
      "reasoning_depth": "deep",
      "context_dependency": 0.6,
      "user_id": "user_1",
      "agent_id": "agent_1"
    },
    "resource_constraints": {
      "max_memory_usage_mb": 1024,
      "max_cpu_usage_percent": 80,
      "max_response_time_ms": 2000,
      "storage_quota_percent": 90
    },
    "preferences": {
      "prioritize_efficiency": true,
      "prioritize_coherence": false,
      "enable_multimodal": true,
      "enable_reasoning": true
    }
  }'
```

### 2. Analyze Task Characteristics

```bash
curl -X POST "http://127.0.0.1:8008/api/v1/memory/analyzer/task-characteristics" \
  -H "Content-Type: application/json" \
  -d '{
    "task_context": {
      "content": "Please analyze this complex multimodal data",
      "modality": ["text", "image"],
      "context_history": []
    }
  }'
```

### 3. Get Resource Status

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/monitor/resources"
```

### 4. Get Weight Adjustment History

```bash
curl -X GET "http://127.0.0.1:8008/api/v1/memory/weights/history"
```

## Database Schema

The system uses SQLite database with the following main tables:

### Memory Configurations Table (memory_configurations)
- Stores memory configurations for users and agents
- Supports multiple configuration types (default, custom, optimized)
- Records enable status and parameters for each memory layer

### Performance Metrics Table (performance_metrics)
- Records system performance metrics
- Supports time-range queries
- Provides aggregation and statistics

### Weight Adjustment History Table (weight_adjustment_history)
- Records all weight adjustment operations
- Includes before/after weight comparison
- Records performance impact and adjustment reasons

### Other Tables
- `context_sessions` - Short-term memory sessions
- `context_messages` - Context messages
- `knowledge_entries` - Long-term memory entries
- `entities` - Knowledge graph entities
- `relations` - Knowledge graph relations
- `multimodal_entries` - Multimodal memory entries

## Development Guide

### Adding New API Endpoints

1. Add new handler function in `backend/src/routers/memory.rs`
2. Use `#[endpoint]` macro to mark the function
3. Register the route in `backend/src/routers/mod.rs`

### Adding New Database Operations

1. Add methods in the corresponding repository module (`db/memory.rs`, `db/performance.rs`, `db/weights.rs`)
2. Use SQLx for type-safe queries
3. Add appropriate error handling

### Frontend Page Development

1. Create new page under `frontend/ant-design-pro-template/src/pages/`
2. Use Ant Design Pro components and charts library
3. Configure routes in `config/routes.ts`

## Testing

### Backend Testing

```bash
cd backend
cargo test
```

### Frontend Testing

```bash
cd frontend/ant-design-pro-template
npm test
```

## Deployment

### Docker Deployment

```bash
# Build images
docker-compose build

# Start services
docker-compose up -d
```

### Production Configuration

1. Configure environment variables (database connection, JWT secret, etc.)
2. Set log level
3. Configure TLS/HTTPS
4. Set resource limits

## Performance Optimization

- Database connection pool configured (max 10 connections)
- Frontend charts use virtual scrolling for large datasets
- API responses use appropriate caching strategies
- Structured logging format for analysis

## Troubleshooting

### Database Connection Failed
- Check if database file path is correct
- Confirm database file permissions
- Check detailed error messages in logs

### API Request Failed
- Check if backend service is running
- Confirm API path and port are correct
- Check browser console and network requests

### Frontend Build Error
- Remove node_modules and reinstall
- Check if Node.js version meets requirements
- Check detailed errors in build logs

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for build instructions, testing, submission process, and extension points (Strategies and Agents). For extending new Strategies or Agents, see [EXTENSION_GUIDE.md](docs/EXTENSION_GUIDE.md).

## License & Open Source

This project is open-sourced under [MIT License](LICENSE). You are free to use, modify, and distribute this software under the license terms. For security issues, please report as specified in [SECURITY.md](SECURITY.md).

## Related Documentation

- [Changelog (CHANGELOG)](CHANGELOG.md) — Version updates and changes
- [Security Policy (SECURITY)](SECURITY.md) — Vulnerability reporting and supported versions
- [Code of Conduct (CODE_OF_CONDUCT)](CODE_OF_CONDUCT.md) — Community participation guidelines
- [Open Source Checklist (OPEN_SOURCE_CHECKLIST)](docs/OPEN_SOURCE_CHECKLIST.md) — Open source readiness and pre-release tasks
- [Architecture (ARCHITECTURE)](docs/ARCHITECTURE.md) — Why adaptive / Why agent-like / Decision pipeline
- [Roadmap (ROADMAP)](docs/ROADMAP.md) — Version planning and ecosystem alignment
- [Use Cases (USE_CASES)](docs/USE_CASES.md) — LLM Agent, Multimodal, Cost-aware routing
- [Contributing (CONTRIBUTING)](CONTRIBUTING.md) — Build, Test, PR, Extension points
- [Extension Guide (EXTENSION_GUIDE)](docs/EXTENSION_GUIDE.md) — Adding WeightStrategy / MemoryAgent
- [Salvo vs Axum Selection](docs/why-salvo-vs-axum.md)
- [Algorithm Design Document](docs/adaptive_memory_algorithm_design.md)
- [API Specification Document](docs/adaptive_memory_api_specification.md)
- [Algorithm Visualization](docs/adaptive_memory_algorithm_visualization.md)
