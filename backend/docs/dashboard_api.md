# Enterprise Dashboard API & Phase 4 Governance Migration

## Overview

This document describes the Dashboard API for enterprise metrics and the Phase 4 migration to move governance logic into hook implementations.

## Dashboard API Endpoints

### Anonymous Aggregate Metrics (Core)

These endpoints provide anonymous aggregate metrics without tenant/user identifiable information:

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/dashboard/metrics` | Aggregate dashboard metrics |
| GET | `/api/v1/dashboard/qps` | Queries per second |
| GET | `/api/v1/dashboard/latency` | Latency percentiles (p50/p95/p99) |
| GET | `/api/v1/dashboard/failures` | Failure rates |

### Enterprise-Specific Aggregations

Enterprise endpoints provide audit/billing summaries:

```json
// GET /api/v1/dashboard/metrics
{
  "aggregate": {
    "qps": 150.5,
    "avg_latency_ms": 12.3,
    "p95_latency_ms": 45.6,
    "success_rate": 0.998,
    "failures": {
      "total": 1000,
      "success": 998,
      "denied": 1,
      "errors": 1,
      "timeouts": 0
    },
    "operations": {
      "store": { "count": 500, "avg_latency_ms": 10.0, ... },
      "search": { "count": 500, "avg_latency_ms": 15.0, ... }
    }
  },
  "enterprise": {
    "audit": {
      "total_events": 1000,
      "denied_events": 5,
      "failed_events": 2
    },
    "billing": {
      "total_api_calls": 50000,
      "estimated_cost": 45.50,
      "overage_count": 0
    }
  },
  "timestamp": 1700000000
}
```

## RBAC on Dashboard Endpoints

Enterprise dashboard endpoints are RBAC-protected. Access requires:
- Valid JWT token or API key
- Appropriate role (Admin/Owner)

### RBAC Roles

| Role | Dashboard Access |
|------|-----------------|
| Owner | Full access |
| Admin | Full access |
| Member | Read-only |
| Reader | Read-only (metrics only) |

## Phase 4 Migration: Governance to Hooks

### Architecture

Phase 4 moves embedded governance logic into hook implementations while core retains interfaces/stubs.

```
┌─────────────────────────────────────────────────────────┐
│                    Core Server                          │
│  ┌─────────────────────────────────────────────────┐  │
│  │              Governance Interfaces              │  │
│  │  - GovernanceHook (trait)                      │  │
│  │  - AuthHook (trait)                            │  │
│  │  - RbacHook (trait)                            │  │
│  │  - NoopHookSet (default)                       │  │
│  └─────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                          │
                          │ Static Injection
                          ▼
┌─────────────────────────────────────────────────────────┐
│              Enterprise HookSet (Closed Source)          │
│  ┌─────────────────────────────────────────────────┐  │
│  │           Hook Implementations                   │  │
│  │  - JwtAuthHookImpl                              │  │
│  │  - RbacHookImpl                                 │  │
│  │  - TenantQuotaHookImpl                          │  │
│  │  - BillingHookImpl                              │  │
│  │  - AuditHookImpl                                │  │
│  └─────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### Core Retention

The core retains:
- Trait definitions (`GovernanceHook`, `AuthHook`, `RbacHook`)
- `NoopHookSet` for default behavior
- Stub implementations that return safe defaults

### Hook Implementations

Enterprise provides:
- Full `GovernanceHookImpl` with license checking
- `JwtAuthHookImpl` with JWT + API key validation
- `RbacHookImpl` with role-based permissions
- `TenantQuotaHookImpl` with quota enforcement

## Operational Guide

### Building Core Server (No Enterprise)

```rust
use crate::hoops::{ServerBuilder, NoopHookSet};

ServerBuilder::new()
    .build();

// Core runs with NoopHookSet - all checks pass by default
```

### Building with Enterprise Hooks

```rust
use crate::hoops::{
    ServerBuilder,
    create_enterprise_hook_set,
};

ServerBuilder::new()
    .with_enterprise_hooks(create_enterprise_hook_set())
    .build();
```

### Dashboard with Enterprise Features

```rust
use crate::routers::dashboard::DashboardState;
use crate::hoops::create_enterprise_hook_set;

let hooks = create_enterprise_hook_set();

let dashboard = DashboardState::new()
    .with_enterprise_hooks(hooks);
```

## Regression Tests

Core behavior is unchanged when running with NoopHookSet:

```rust
#[test]
fn test_noop_hooks_allows_core() {
    let hooks = EnterpriseHookSet::new();

    // All license checks pass (default allow)
    assert!(hooks.check_license("test", LicenseTier::Free));

    // All feature checks pass (default allow)
    assert!(hooks.check_feature("test", "any_feature"));

    // All quota checks pass (unlimited)
    let quota = hooks.check_quota("test", Resource::ApiCalls);
    assert!(quota.allowed);
}
```

### Test Coverage

| Test | Description |
|------|-------------|
| `test_dashboard_state_default` | Dashboard state initializes correctly |
| `test_dashboard_state_with_hooks` | Enterprise hooks can be attached |
| `test_dashboard_metrics_empty` | Empty metrics returns valid response |
| `test_noop_hooks_allows_core` | NoopHookSet allows all operations |

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `METRICS_ENABLED` | Enable metrics collection | `true` |
| `METRICS_WINDOW_MS` | Metrics aggregation window | `10000` |
| `DASHBOARD_RBAC_ENABLED` | Enable RBAC on dashboard | `true` |

### Feature Flags

| Feature | Description |
|---------|-------------|
| `enterprise` | Enable enterprise features |
