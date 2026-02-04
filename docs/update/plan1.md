好，选 **A** 是对的，这一步**直接决定这个项目是不是“架构型开源”** 👍
我不写概念了，**直接给你一套能落到 backend 的 Rust 抽象草图**，风格与你现在的 Salvo + services 结构是**可融合的**。

下面内容你可以当作 **v0.3 的第一批可提交代码设计**。

---

## 一、v0.3 核心目标（一句话）

> **把“一堆 service + scheduler”升级为：Agent Runtime + 可组合决策单元**

不引入 LLM，也不破坏现有逻辑。

---

## 二、推荐的新目录结构（backend）

```text
backend/src/
├── agents/                    # v0.3 新增
│   ├── mod.rs
│   ├── runtime.rs             # Agent Runtime
│   ├── analyzer.rs            # AnalyzerAgent
│   ├── predictor.rs           # PredictorAgent
│   └── scheduler.rs           # SchedulerAgent
│
├── decision/
│   ├── mod.rs
│   ├── trace.rs               # 决策可解释性
│   └── context.rs             # 统一上下文
│
├── strategies/                # v0.3 关键扩展点
│   ├── mod.rs
│   ├── weight.rs
│   └── memory.rs
│
├── services/                  # v0.2 逻辑逐步迁移
│   └── (existing code)
```

---

## 三、Agent Runtime（核心抽象）

### 1️⃣ Agent 生命周期 Trait

```rust
// agents/runtime.rs
use async_trait::async_trait;

#[async_trait]
pub trait Agent {
    type Input;
    type Output;

    async fn observe(&self, input: &Self::Input) -> anyhow::Result<()>;
    async fn decide(&self, input: &Self::Input) -> anyhow::Result<Self::Output>;
    async fn act(&self, output: &Self::Output) -> anyhow::Result<()>;
}
```

**设计意图（很重要）：**

- `observe`：收集上下文 / 资源 / 历史
- `decide`：纯决策（可被 trace）
- `act`：副作用（写 DB、更新权重）

> 这一步直接为未来 **LLM Agent / Rule Agent / Hybrid Agent** 打基础。

---

## 四、统一决策上下文（Agent 之间“说人话”）

```rust
// decision/context.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionContext {
    pub task_id: String,
    pub task_complexity: f32,
    pub modality_requirements: Vec<String>,
    pub reasoning_depth: String,
    pub context_dependency: f32,

    pub resource_budget_mb: u32,
    pub latency_budget_ms: u64,
}
```

**原则：**

- 所有 Agent 都只吃这个
- API 层 → 转成 `DecisionContext`
- Agent 不关心 HTTP / DB / JWT

---

## 五、AnalyzerAgent（从你现有 analyzer.rs 演进）

```rust
// agents/analyzer.rs
use async_trait::async_trait;
use crate::agents::runtime::Agent;
use crate::decision::context::DecisionContext;

pub struct AnalyzerAgent;

#[async_trait]
impl Agent for AnalyzerAgent {
    type Input = DecisionContext;
    type Output = TaskAnalysis;

    async fn observe(&self, _input: &Self::Input) -> anyhow::Result<()> {
        Ok(())
    }

    async fn decide(&self, input: &Self::Input) -> anyhow::Result<Self::Output> {
        Ok(TaskAnalysis {
            complexity_score: input.task_complexity,
            requires_multimodal: !input.modality_requirements.is_empty(),
            deep_reasoning: input.reasoning_depth == "deep",
        })
    }

    async fn act(&self, _output: &Self::Output) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct TaskAnalysis {
    pub complexity_score: f32,
    pub requires_multimodal: bool,
    pub deep_reasoning: bool,
}
```

---

## 六、Strategy 插件抽象（v0.3 的“开源钩子”）

### 1️⃣ 权重调整策略

```rust
// strategies/weight.rs
use crate::decision::context::DecisionContext;

pub trait WeightAdjustmentStrategy: Send + Sync {
    fn name(&self) -> &'static str;

    fn adjust_weights(
        &self,
        ctx: &DecisionContext,
        current_weights: &MemoryWeights,
    ) -> MemoryWeights;
}

#[derive(Clone)]
pub struct MemoryWeights {
    pub stm: f32,
    pub ltm: f32,
    pub kg: f32,
    pub mm: f32,
}
```

### 2️⃣ 一个内置策略示例

```rust
pub struct LinearDecayStrategy;

impl WeightAdjustmentStrategy for LinearDecayStrategy {
    fn name(&self) -> &'static str {
        "linear_decay"
    }

    fn adjust_weights(
        &self,
        ctx: &DecisionContext,
        weights: &MemoryWeights,
    ) -> MemoryWeights {
        let factor = 1.0 - ctx.context_dependency * 0.1;
        MemoryWeights {
            stm: weights.stm * factor,
            ..weights.clone()
        }
    }
}
```

> 🔥 这是你吸引外部贡献者的地方
> PR 只要加一个 struct + impl

---

## 七、SchedulerAgent（整合一切）

```rust
// agents/scheduler.rs
pub struct SchedulerAgent {
    pub weight_strategy: Box<dyn WeightAdjustmentStrategy>,
}

impl SchedulerAgent {
    pub fn select_memory(
        &self,
        ctx: &DecisionContext,
        weights: &MemoryWeights,
    ) -> SelectedMemory {
        let adjusted = self.weight_strategy.adjust_weights(ctx, weights);

        if adjusted.stm >= adjusted.ltm {
            SelectedMemory::ShortTerm
        } else {
            SelectedMemory::LongTerm
        }
    }
}

pub enum SelectedMemory {
    ShortTerm,
    LongTerm,
    KnowledgeGraph,
    MultiModal,
}
```

---

## 八、这一步你现在“必须做 / 不必做”的事

### ✅ 现在就值得做

- 新建 `agents/` + `strategies/`
- 把 **Analyzer / Scheduler 的核心逻辑搬进去**
- 保留现有 services，当 adapter 用

### ❌ 现在不必做

- 不用删旧代码
- 不用改 API
- 不用引 LLM
- 不用上 PostgreSQL

---

## 九、下一步我可以继续帮你干的事（直接推进）

你现在已经**站在 v0.3 的门口**了，接下来三条路都很硬核：

- **B️⃣ 把你现有 services 映射到 Agent（逐文件迁移表）**
- **C️⃣ 给你补一个 `DecisionTrace`（前端能画流程图那种）**
- **D️⃣ 帮你写一份 `ARCHITECTURE.md`，直接拉开同类项目差距**

你直接回 **B / C / D**，我继续陪你把这个项目推到「值得被长期维护」的级别。
