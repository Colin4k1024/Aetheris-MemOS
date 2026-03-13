# 扩展指南

本指南说明如何在不更改核心编排的情况下，使用新的**权重策略**和**智能体**扩展自适应记忆系统。代码参考适用于 Rust 后端。

## 权重策略

权重调整通过 `WeightStrategy` 特质进行插拔。调度器按顺序运行策略链；每个策略接收当前指标并返回更新后的权重和原因。

### 特质和类型

定义于 **`backend/src/services/weight_strategy.rs`**：

- **`WeightStrategy`** — `fn evaluate(&self, metrics: &WeightStrategyMetrics<'_>) -> WeightDelta`
- **`WeightStrategyMetrics`** — `task_profile`、`cost_benefit_ratio`、`base_weights`
- **`WeightDelta`** — `weights: MemoryWeights`、`reasons: AdjustmentReasons`

### 添加新策略

1. 为您的类型实现 `WeightStrategy`（必须是 `Send + Sync`）：

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

2. 在调整器中使用它：

- **默认链**: `DynamicWeightAdjuster::new()` 使用 `MarginalBenefitStrategy` 和 `LinearDecayStrategy`。
- **自定义链**: `DynamicWeightAdjuster::with_strategies(vec![Box::new(MyCustomStrategy), ...])` 并在构建调整器的位置（例如在测试中或功能标志路径中）连接该实例。

内置示例：同文件中的 `MarginalBenefitStrategy`、`LinearDecayStrategy`、`SynergyAwareStrategy`。

---

## 智能体 (MemoryAgent)

核心管道建模为具有 **observe → decide → act** 的智能体。分析器、预测器和调度器实现 `MemoryAgent`，以便系统可以描述为基于智能体的并可扩展（例如，使用 LLM 支持的实现）。

### 特质和类型

定义于 **`backend/src/services/agent.rs`**：

- **`MemoryAgent`** — 关联类型 `Context`、`Observation`、`Decision`、`Action`；方法 `observe`、`decide`、`act`（都返回 futures）。
- **分析器** — `Context = TaskContextInput`，`Observation = AnalyzerObservation`（特征、记忆策略、置信度）。
- **预测器** — `Context = MemoryConfig`，`Observation = MemoryConfig`，`Decision = PredictorDecision`（预测、协同、衰减、分解）。
- **调度器** — `Context = TaskContextBundle`，`Action = MemorySelectionResult`；完整管道在 `adaptive_memory_selection()` 中运行；特质实现是为了统一性。

### 添加或替换智能体

1. 定义您的 context/observation/decision/action 类型（或重用 `agent.rs` 中的现有类型）。
2. 为您的结构实现 `MemoryAgent`；`observe` / `decide` / `act` 可以是同步的（返回 `std::future::ready(...))`）或异步的。
3. 将您的智能体连接到编排层：在构建分析器/预测器/调度器的位置替换默认实现，或添加使用您的智能体的新代码路径（例如新端点）。

现有的**调度器**组合分析器、预测器、监控器和权重调整器。要交换一个智能体，您可以使用您的实现而不是默认实现来构建调度器（或新的编排器）。特质不需要注册表；组合在代码中是显式的。

---

## 决策追踪 API

无需持久化即可检查决策管道：

- **端点**: `POST /api/v1/memory/adaptive/trace`
- **请求体**: 与 `POST /api/v1/memory/adaptive/select` 相同（`task_context`、`resource_constraints`、`preferences`）。
- **响应**: 完整追踪 — `task_id`、`analyzer`（特征、记忆策略、置信度）、`resource_status`、`initial_memory_config`、`predictor`（预测、协同、衰减、分解）、`cost_benefit_ratio`、`weight_adjustment`（adjusted_weights、adjustment_reasons）、`final_result`。

使用此 API 调试或可视化为什么选择给定的记忆配置。前端**记忆决策追踪**页面（路由：`/memory-decision-trace`）调用此 API 并逐步显示管道。

追踪的持久化（数据库表、在自适应或追踪调用时可选保存）正在计划中；请参阅 [ROADMAP.md](ROADMAP.md)。
