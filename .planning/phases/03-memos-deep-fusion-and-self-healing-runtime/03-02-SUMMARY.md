---
phase: "03"
plan: "02"
subsystem: memory
tags: [weight-decay, semantic-evolution, memory-evolution]
dependency_graph:
  requires: ["03-01"]
  provides: []
  affects: ["weight-system", "memory-evolution"]
tech_stack:
  added: ["weight_decay.rs"]
  patterns: ["exponential-decay", "service-layer"]
key_files:
  created:
    - backend/src/services/weight_decay.rs
  modified:
    - backend/config.toml
    - backend/src/config/mod.rs
    - backend/src/services/mod.rs
    - backend/src/routers/memory.rs
    - backend/src/routers/mod.rs
decisions: []
metrics:
  duration_minutes: 4
  completed_date: "2026-03-28"
  tasks_completed: 3
  files_changed: 6
---

# Phase 03 Plan 02: Weight Decay & Semantic Evolution Summary

## One-liner

Exponential weight decay service with configurable lambda and status API endpoint.

## Completed Tasks

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add decay config and WeightDecayService | ddaf8e3 | config.toml, config/mod.rs, weight_decay.rs, services/mod.rs |
| 2 | Add weight status endpoint | ddaf8e3 | routers/memory.rs, routers/mod.rs |
| 3 | Tests | ddaf8e3 | weight_decay.rs (unit tests) |

## What Was Built

### WeightDecayService (`backend/src/services/weight_decay.rs`)

Exponential decay using formula: `w(t) = w0 * e^(-lambda * t)`

- `WeightDecayService::new(lambda: f64)` constructor
- `apply_decay(entry, age_seconds)` method returning decayed weight
- Unit tests verify: zero-age identity, decay rate, importance scaling, lambda sensitivity

### Configuration

Added `[memory_evolution]` section to `config.toml`:
```toml
[memory_evolution]
decay_lambda = 0.01
```

Added `MemoryEvolutionConfig` struct to `ServerConfig`.

### Endpoint

`GET /api/v1/memory/weights/status` returns:
```json
{ "decay_lambda": 0.01, "active_weights": { "stm": 1.0, "ltm": 0.6, "kg": 0.0, "mm": 0.0 } }
```

## Deviations from Plan

None - plan executed exactly as written.

## Test Results

```
cargo test --lib weight_decay
running 3 tests
test services::weight_decay::tests::test_exponential_decay_formula ... ok
test services::weight_decay::tests::test_decay_with_different_importance ... ok
test services::weight_decay::tests::test_higher_lambda_faster_decay ... ok
test result: ok. 3 passed
```

## Self-Check: PASSED

- [x] `config.toml` contains `[memory_evolution]` section
- [x] `WeightDecayService` applies exponential decay formula
- [x] Endpoint returns JSON with decay_lambda and active weights
- [x] `cargo test weight_decay` passes
- [x] Commit ddaf8e3 verified
