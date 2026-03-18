# Open-Core Boundary

This document defines the boundary between the open-source Core and closed-source Enterprise modules.

## Overview

The Adaptive Memory System follows an **open-core** model:

- **Core**: MIT Licensed, permanently open-source memory kernel and extension hooks
- **Enterprise**: Commercial licensed, closed-source governance, billing, and advanced features

## Core Modules (MIT License)

These modules are permanently open-source and will always remain available under the MIT license.

### Memory Kernel

| Module | Path | Description |
|--------|------|-------------|
| Kernel Traits | `src/kernel/` | Unified memory interface (Memory trait, types, errors) |
| Memory Layers | `src/layers/stm/` | Short-term memory management |
| Memory Layers | `src/layers/ltm/` | Long-term memory storage |
| Memory Layers | `src/layers/kg/` | Knowledge graph storage |
| Memory Layers | `src/layers/mm/` | Multimodal memory storage |

### Agent Runtime

| Module | Path | Description |
|--------|------|-------------|
| Memory Agent | `src/agent/` | Agent runtime integration |
| Runtime Adapters | `src/runtime/` | OpenAI/Anthropic SDK adapters |
| Policy Engine | `src/policy/` | Scheduler and cost model |

### Core Services

| Module | Path | Description |
|--------|------|-------------|
| Analyzer | `src/services/analyzer.rs` | Task feature analysis |
| Predictor | `src/services/predictor.rs` | Performance prediction |
| Scheduler | `src/services/scheduler.rs` | Adaptive memory scheduler |
| Weight Strategies | `src/services/weight_strategy.rs` | Pluggable weight strategies |
| Embedding | `src/services/embedding.rs` | Embedding model service (Ollama) |
| LLM Service | `src/services/llm.rs` | LLM service (Ollama) |
| Memory Search | `src/services/memory_search.rs` | Semantic/keyword/hybrid search |
| Memory Storage | `src/services/memory_storage.rs` | Storage management |
| Memory Transfer | `src/services/memory_transfer.rs` | STM→LTM transfer pipeline |

### Core Routers

| Module | Path | Description |
|--------|------|-------------|
| Memory API | `src/routers/memory.rs` | Core memory operations |
| Memory Search | `src/routers/memory_search.rs` | Search endpoints |
| Memory Storage | `src/routers/memory_storage.rs` | Storage endpoints |
| Knowledge Graph | `src/routers/knowledge_graph.rs` | KG endpoints |
| Multimodal | `src/routers/multimodal.rs` | Multimodal endpoints |
| Memory Pool | `src/routers/memory_pool.rs` | Multi-agent collaboration |

### Infrastructure

| Module | Path | Description |
|--------|------|-------------|
| Middleware | `src/hoops/` | CORS, JWT, rate limiting |
| Tenant Context | `src/tenant/` | Basic multi-tenant context |
| Configuration | `src/config/` | Application configuration |
| Database | `src/db/` | PostgreSQL repositories |

## Enterprise Modules (Commercial License)

These modules are commercial features requiring an Enterprise license.

### Governance & Compliance

| Module | Path | Description |
|--------|------|-------------|
| Governance Hook | `src/hoops/enterprise.rs` | Static injection host for governance |
| Audit Logging | `src/services/audit.rs` | Comprehensive audit trail |
| RBAC | `src/services/rbac.rs` | Role-based access control |

### Billing & metering

| Module | Path | Description |
|--------|------|-------------|
| Usage Tracker | `src/services/usage_tracker.rs` | Usage metering |
| Billing API | `src/routers/billing.rs` | Billing endpoints |
| Quota Management | `src/services/quota.rs` | Resource quota enforcement |

### Enterprise Features

| Module | Path | Description |
|--------|------|-------------|
| Cluster Management | `src/routers/enterprise.rs` | Node registration, leader election |
| Sharding | `src/services/enterprise.rs` | Data sharding management |
| Dashboard API | `src/routers/visualization.rs` | Enterprise dashboard endpoints |

## Static Injection

Enterprise features are injected at compile-time using the Server Builder pattern:

```rust
use crate::hoops::{EnterpriseHookSet, GovernanceHook, ServerBuilder};

// Define custom governance implementation
struct MyGovernance;

impl GovernanceHook for MyGovernance {
    fn check_license(&self, tenant_id: &str, tier: LicenseTier) -> bool {
        // Custom license check
        true
    }
    // ... other methods
}

// Build server with enterprise hooks
ServerBuilder::new()
    .with_governance(MyGovernance)
    .with_auth(MyAuth)
    .with_rbac(MyRbac)
    .build();
```

## Feature Flags

The project uses Cargo feature flags to control enterprise features:

```toml
# Cargo.toml
[features]
default = []
enterprise = []
```

- **Default build**: Core only, no enterprise dependencies
- **Enterprise build**: Full feature set with enterprise hooks

```bash
# Core build only
cargo build

# Full enterprise build
cargo build --features enterprise
```

## CI/CD

### Core-Only CI

The core repository includes CI that ensures the open-source build has **no dependencies** on enterprise code:

```yaml
# .github/workflows/ci-core-only.yml
cargo build --no-default-features
cargo test --no-default-features
```

This ensures that the Core remains truly open and can be used independently.

## License

- **Core**: [MIT License](../../LICENSE)
- **Enterprise**: Commercial Enterprise License (contact sales)

---

Last updated: 2026-03-18
