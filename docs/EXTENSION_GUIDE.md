# Extension Guide

This guide explains how to extend the adaptive memory system with new **weight strategies** and **agents** without changing core orchestration. The code references are for the Rust backend.

## Weight strategies

Weight adjustment is pluggable via the `WeightStrategy` trait. The scheduler runs a chain of strategies in order; each receives the current metrics and returns updated weights and reasons.

### Trait and types

Defined in **`backend/src/services/weight_strategy.rs`**:

- **`WeightStrategy`** — `fn evaluate(&self, metrics: &WeightStrategyMetrics<'_>) -> WeightDelta`
- **`WeightStrategyMetrics`** — `task_profile`, `cost_benefit_ratio`, `base_weights`
- **`WeightDelta`** — `weights: MemoryWeights`, `reasons: AdjustmentReasons`

### Adding a new strategy

1. Implement `WeightStrategy` for your type (must be `Send + Sync`):

```rust
use crate::services::weight_strategy::{WeightStrategy, WeightStrategyMetrics, WeightDelta};

pub struct MyCustomStrategy;

impl WeightStrategy for MyCustomStrategy {
    fn evaluate(&self, metrics: &WeightStrategyMetrics<'_>) -> WeightDelta {
        let mut weights = metrics.base_weights.clone();
        let mut reasons = crate::models::AdjustmentReasons {
            stm: "Primary memory, always enabled".to_string(),
            ltm: String::new(),
            kg: String::new(),
            mm: String::new(),
        };
        // Your logic here; update weights and reasons.
        WeightDelta { weights, reasons }
    }
}
```

2. Use it in the adjuster:

- **Default chain**: `DynamicWeightAdjuster::new()` uses `MarginalBenefitStrategy` and `LinearDecayStrategy`.
- **Custom chain**: `DynamicWeightAdjuster::with_strategies(vec![Box::new(MyCustomStrategy), ...])` and wire that instance where the adjuster is constructed (e.g. in tests or a feature-flagged path).

Built-in examples: `MarginalBenefitStrategy`, `LinearDecayStrategy`, `SynergyAwareStrategy` in the same file.

---

## Agents (MemoryAgent)

The core pipeline is modeled as agents with **observe → decide → act**. Analyzer, predictor, and scheduler implement `MemoryAgent` so the system can be described as agent-based and extended (e.g. with LLM-backed implementations).

### Trait and types

Defined in **`backend/src/services/agent.rs`**:

- **`MemoryAgent`** — associated types `Context`, `Observation`, `Decision`, `Action`; methods `observe`, `decide`, `act` (all return futures).
- **Analyzer** — `Context = TaskContextInput`, `Observation = AnalyzerObservation` (characteristics, memory_strategy, confidence).
- **Predictor** — `Context = MemoryConfig`, `Observation = MemoryConfig`, `Decision = PredictorDecision` (prediction, synergy, decay, breakdown).
- **Scheduler** — `Context = TaskContextBundle`, `Action = MemorySelectionResult`; the full pipeline runs in `adaptive_memory_selection()`; the trait impl is for uniformity.

### Adding or replacing an agent

1. Define your context/observation/decision/action types (or reuse existing ones in `agent.rs`).
2. Implement `MemoryAgent` for your struct; `observe` / `decide` / `act` can be sync (return `std::future::ready(...)`) or async.
3. Wire your agent into the orchestration layer: either replace the default analyzer/predictor/scheduler where they are constructed, or add a new code path (e.g. a new endpoint) that uses your agent.

The existing **scheduler** composes the analyzer, predictor, monitor, and weight adjuster. To swap one agent, you would construct the scheduler (or a new orchestrator) with your implementation instead of the default. The trait does not require a registry; composition is explicit in code.

---

## Decision Trace API

The decision pipeline can be inspected without persisting:

- **Endpoint**: `POST /api/v1/memory/adaptive/trace`
- **Body**: Same as `POST /api/v1/memory/adaptive` (`task_context`, `resource_constraints`, `preferences`).
- **Response**: Full trace — `task_id`, `analyzer` (characteristics, memory_strategy, confidence), `resource_status`, `initial_memory_config`, `predictor` (prediction, synergy, decay, breakdown), `cost_benefit_ratio`, `weight_adjustment` (adjusted_weights, adjustment_reasons), `final_result`.

Use this to debug or visualize why a given memory configuration was chosen. The frontend **Memory Decision Trace** page (route: `/memory-decision-trace`) calls this API and shows the pipeline step by step.

Persistence of traces (DB table, optional save on adaptive or trace call) is planned; see [ROADMAP.md](ROADMAP.md).
