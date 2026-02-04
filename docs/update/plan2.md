好，直接进入 **B：Strategy 插件体系设计（可落地、对开源友好）**。
我会分三步来：**设计目标 → 目录结构 → 可运行的最小示例（Rust）**。你可以一边看一边往仓库里加。

---

## 一、你这个项目为什么“必须”上 Strategy Plugin

从开源视角看，你现在的系统有三个**被硬编码锁死的点**：

- 权重调整逻辑（`weight_adjuster.rs`）
- 记忆选择逻辑（`scheduler.rs`）
- 性能评估 / 预测中的策略假设

这会导致：

- 外部贡献者 **只能改你核心代码**
- PR 容易冲突
- 项目看起来像“论文实现”，不像“框架”

👉 **Strategy Plugin 的目标**：

> 让“怎么决策”变成插件，让“怎么编排”保持稳定。

---

## 二、推荐的目录结构（v0.3）

这是我建议你直接采用的结构（不会破坏现有代码，只是重构）：

```
backend/src/
├── strategies/
│   ├── mod.rs
│   ├── weight/
│   │   ├── mod.rs
│   │   ├── linear_decay.rs
│   │   ├── marginal_gain.rs
│   │   └── synergy_aware.rs
│   ├── memory_selection/
│   │   ├── mod.rs
│   │   ├── rule_based.rs
│   │   └── cost_aware.rs
│   └── registry.rs
│
├── services/
│   ├── scheduler.rs        # 只负责 orchestration
│   └── weight_adjuster.rs  # 调用 strategy，不再写算法
```

**核心原则：**

- `services` = **流程控制**
- `strategies` = **决策算法**
- `registry` = **插件发现 / 选择**

---

## 三、核心 Trait 设计（这是关键）

### 1️⃣ WeightAdjustmentStrategy

```rust
pub trait WeightAdjustmentStrategy: Send + Sync {
    fn name(&self) -> &'static str;

    fn adjust(
        &self,
        context: &TaskContext,
        current_weights: &MemoryWeights,
        performance: &PerformanceMetrics,
    ) -> MemoryWeights;
}
```

**设计要点：**

- `name()` 用于前端展示 + trace
- 不依赖数据库
- 输入是纯结构体（方便测试 & LLM 替换）

---

### 2️⃣ 一个示例插件：Linear Decay

```rust
pub struct LinearDecayStrategy;

impl WeightAdjustmentStrategy for LinearDecayStrategy {
    fn name(&self) -> &'static str {
        "linear_decay"
    }

    fn adjust(
        &self,
        _context: &TaskContext,
        current: &MemoryWeights,
        _performance: &PerformanceMetrics,
    ) -> MemoryWeights {
        MemoryWeights {
            stm: current.stm * 0.95,
            ltm: current.ltm * 1.02,
            kg: current.kg,
            mm: current.mm,
        }
    }
}
```

这类代码**非常适合外部贡献者**，不会碰你主干逻辑。

---

## 四、Strategy Registry（插件入口）

这是开源体验的关键点。

```rust
pub struct StrategyRegistry {
    weight_strategies: HashMap<String, Arc<dyn WeightAdjustmentStrategy>>,
}

impl StrategyRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            weight_strategies: HashMap::new(),
        };

        registry.register_weight(LinearDecayStrategy);
        registry.register_weight(MarginalGainStrategy);

        registry
    }

    pub fn register_weight<S>(&mut self, strategy: S)
    where
        S: WeightAdjustmentStrategy + 'static,
    {
        self.weight_strategies
            .insert(strategy.name().to_string(), Arc::new(strategy));
    }

    pub fn get_weight(&self, name: &str) -> Option<Arc<dyn WeightAdjustmentStrategy>> {
        self.weight_strategies.get(name).cloned()
    }
}
```

### 这一步带来的好处

- 前端可以传 `"strategy": "linear_decay"`
- 未来可以：
  - env 配置默认策略
  - LLM 选择策略
  - A/B Test 多策略

---

## 五、Scheduler 如何“降级”为编排器

你现在的 `scheduler.rs` 应该重构成这样：

```rust
let strategy = registry
    .get_weight(config.weight_strategy.as_str())
    .unwrap_or(default_strategy);

let new_weights = strategy.adjust(
    &task_context,
    &current_weights,
    &performance_metrics,
);
```

👉 **Scheduler 不再关心算法细节**
👉 只关心：输入 → 选策略 → 执行 → 记录 trace

---

## 六、对你这个项目的“开源加分点”

加了 Strategy Plugin 后，你可以在 README 写：

> - ✅ Pluggable memory strategies
> - ✅ Easy to extend without touching core logic
> - ✅ Agent & LLM-ready decision architecture

这会直接让项目：

- 从「系统实现」➡「框架」
- 从「个人项目」➡「可协作项目」

---

## 下一步我可以继续帮你做的（强烈建议）

你现在已经在 **v0.3 的正确轨道上了**，接下来三选一：

- **C️⃣ 把现有 weight_adjuster.rs 拆成 Strategy（逐文件改）**
- **D️⃣ 设计 Decision Trace 数据结构（Explainability 核心）**
- **E️⃣ 帮你写一份《如何贡献一个 Strategy》的 CONTRIBUTING.md**

你选一个，我直接给你 **可提交代码级别** 的答案。
