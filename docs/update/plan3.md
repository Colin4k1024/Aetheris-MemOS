好，**C 是最关键的一步**：把 v0.3 从“理念正确”推进到 **“一次可控重构”**。
下面我给你的是一份 **可执行、可分 PR、不会把项目炸掉的「v0.3 必改清单」**，严格基于你现在这个仓库结构。

---

## v0.3 必改清单（按模块拆解）

# v0.3 Required Changes Checklist

This checklist maps the **current implementation** to the **v0.3 extensible architecture**.
The goal is controlled refactoring with minimal disruption.

---

## 1. Backend — Core Architecture Refactor (High Priority)

### 1.1 New Core Abstraction Layer (Must Add)

**Add new directory:**

```
backend/src/core/
├── agent.rs            # Agent trait & lifecycle
├── context.rs          # TaskContext / ResourceContext
├── decision.rs         # DecisionResult / DecisionTrace
├── mod.rs
```

**Why**

- Currently: logic is service-driven
- v0.3: logic becomes agent-driven, services become orchestrators

---

### 1.2 Analyzer / Predictor / Scheduler (Must Change)

#### Files to refactor:

```
backend/src/services/
├── analyzer.rs
├── predictor.rs
├── scheduler.rs
```

#### Required changes:

- Extract pure logic into:
  - `AnalyzerAgent`
  - `PredictorAgent`
  - `SchedulerAgent`

- Services become thin wrappers:
  - Input validation
  - Agent invocation
  - Trace collection

❌ **Anti-pattern to remove**

- Services directly mutating shared state
- Hard-coded weight logic inside scheduler

---

## 2. Strategy Plugin System (Core v0.3 Feature)

### 2.1 Strategy Trait Definitions (Must Add)

**Add directory:**

```
backend/src/strategies/
├── weight/
│   ├── mod.rs
│   ├── strategy.rs          # WeightAdjustmentStrategy trait
│   ├── linear_decay.rs      # Default impl
│   └── marginal_benefit.rs  # Example advanced impl
├── memory/
│   ├── mod.rs
│   └── selection.rs
```

**Mandatory refactor**

- Move all weight calculation logic **out of**:

  ```
  services/weight_adjuster.rs
  ```

- That file should only orchestrate strategies.

---

### 2.2 Weight Adjuster (Must Change)

#### File:

```
backend/src/services/weight_adjuster.rs
```

#### Changes:

- Replace internal logic with:
  - Strategy registry
  - Strategy execution pipeline

- Allow runtime strategy switching (config-driven)

---

## 3. Decision Trace & Explainability (High Value)

### 3.1 Backend Models (Must Add)

**Add file:**

```
backend/src/models/decision_trace.rs
```

Include:

- task_id
- analyzer_output
- predictor_output
- selected_strategy
- weight_before / after
- final_decision

---

### 3.2 Persistence Layer (Must Extend)

#### Files to update:

```
backend/src/db/
├── performance.rs
├── weights.rs
```

Add:

- Decision trace persistence
- Query by task_id / time range

---

## 4. API Layer Changes (Medium Priority)

### 4.1 New API Endpoints (Must Add)

#### File:

```
backend/src/routers/memory.rs
```

Add endpoints:

- `GET /memory/decision-trace/{task_id}`
- `GET /memory/decision-trace?from=&to=`

❌ Do NOT break existing APIs
✔️ Add-only policy for v0.3

---

## 5. Frontend — Minimal but Strategic Updates

### 5.1 New Page (Recommended)

**Add page:**

```
frontend/src/pages/DecisionTrace/
```

Features:

- Timeline view of decision steps
- Weight change visualization
- Strategy name & explanation

---

### 5.2 Existing Pages (Minor Changes)

| Page          | Action                          |
| ------------- | ------------------------------- |
| Dashboard     | Add decision trace entry point  |
| WeightHistory | Link to trace detail            |
| Performance   | Correlate with decision outcome |

---

## 6. Documentation (Must Update for Open Source)

### 6.1 New Docs (Must Add)

```
docs/
├── ARCHITECTURE.md          # v0.3 architecture overview
├── STRATEGY_PLUGIN_GUIDE.md # How to add strategies
└── AGENT_MODEL.md           # Agent abstraction explanation
```

---

### 6.2 Existing Docs (Must Update)

| File                                | Required Update                |
| ----------------------------------- | ------------------------------ |
| README.md                           | Add v0.3 positioning & roadmap |
| adaptive_memory_algorithm_design.md | Map to Agent/Strategy model    |
| API_USAGE_GUIDE.md                  | Add trace endpoints            |

---

## 7. Explicit Non-Goals (Important)

❌ No LLM integration in v0.3
❌ No database migration breaking changes
❌ No frontend redesign

v0.3 is about **architecture clarity**, not intelligence inflation.

---

## Suggested PR Breakdown

1. PR-1: Core agent & context abstraction
2. PR-2: Strategy system refactor
3. PR-3: Decision trace & persistence
4. PR-4: API & frontend trace UI
5. PR-5: Documentation & cleanup

---

### 接下来我可以继续帮你三件事（都很实在）👇

**你选一个，我直接给代码或文档草稿：**

- **D️⃣ 给你画一张 v0.3 后端真实目录结构图（重构后）**
- **E️⃣ 写 `STRATEGY_PLUGIN_GUIDE.md` 的完整初版**
- **F️⃣ 把 PR-1 的 agent.rs / decision.rs 直接写成可编译 Rust 文件**

你这个项目现在已经不是“练习项目”了，是**可以认真做成技术名片的系统**。我会按这个级别继续陪你打磨。
