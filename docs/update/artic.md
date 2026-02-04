太好了，**E 是最值钱的一步** 💎
下面我直接给你一份**可以原样放进仓库、对外不丢脸、对内能指导开发的 `ARCHITECTURE.md`**。

这是 **v0.3 级别**的架构文档，不是 README 重复说明，而是**“为什么这样设计”**。

---

下面是**可直接新建为 `docs/ARCHITECTURE.md` 的完整内容**👇

# Architecture Overview — Adaptive Memory System (v0.3)

## 1. Design Goals

The Adaptive Memory System is designed to support **intelligent, explainable, and evolvable memory orchestration** for AI agents under resource constraints.

The core architectural goals are:

- **Adaptivity**: Dynamically select memory configurations based on task and resource context
- **Explainability**: Every decision must be traceable and inspectable
- **Extensibility**: New strategies and agents can be added without refactoring existing code
- **Separation of Concerns**: Clear boundaries between API, orchestration, decision logic, and persistence

---

## 2. High-Level Architecture

```
┌────────────┐
│   Client   │
└─────┬──────┘
      │ HTTP / JSON
┌─────▼──────┐
│   Router   │   (API Layer)
└─────┬──────┘
      │
┌─────▼──────┐
│  Service   │   (Application Layer)
└─────┬──────┘
      │ delegates
┌─────▼───────────────┐
│       Agent         │   (Decision Orchestration)
└─────┬───────────────┘
      │ applies
┌─────▼───────────────┐
│     Strategy        │   (Pluggable Policies)
└─────┬───────────────┘
      │ produces
┌─────▼───────────────┐
│  Decision Trace     │   (Explainability Core)
└─────┬───────────────┘
      │ persists
┌─────▼───────────────┐
│       DB            │   (Infrastructure)
└─────────────────────┘
```

---

## 3. Layer Responsibilities

### 3.1 Router Layer

**Responsibility**

- HTTP request/response handling
- Input validation and serialization
- Error mapping

**Constraints**

- Must not contain business logic
- Must not perform decision-making

---

### 3.2 Service Layer (Thin Application Layer)

**Responsibility**

- Coordinate request lifecycle
- Call appropriate Agent
- Persist results and traces

**Characteristics**

- Stateless
- Minimal logic (ideally < 50 lines per method)
- Replaceable without affecting core behavior

---

### 3.3 Agent Layer (Decision Orchestration)

**Responsibility**

- Orchestrate decision workflow
- Aggregate inputs from multiple sources
- Invoke strategies in a deterministic order

**Key Concepts**

- Agents represent _behavioral units_
- One agent = one responsibility domain
  - AnalyzerAgent
  - PredictorAgent
  - SchedulerAgent

Agents **do not** persist data directly.

---

### 3.4 Strategy Layer (Pluggable Policy System)

**Responsibility**

- Encapsulate decision logic
- Operate on immutable context
- Produce structured outputs

**Design Principles**

- Strategy = pure logic
- No side effects
- Easily testable in isolation

**Example**

- Linear decay strategy
- Marginal benefit strategy
- Multimodal preference strategy

---

### 3.5 Decision Trace (Explainability Core)

**Responsibility**

- Capture _why_ a decision was made
- Record intermediate reasoning steps
- Enable visualization and auditing

**Design Choice**
DecisionTrace is treated as a **first-class domain object**, not a debug artifact.

---

### 3.6 Data Access Layer

**Responsibility**

- Persist domain entities
- Execute queries
- Handle migrations

**Constraints**

- No business logic
- No orchestration
- Strict schema ownership

---

## 4. Core Domain Models

| Model           | Purpose                         |
| --------------- | ------------------------------- |
| TaskContext     | Task characteristics and intent |
| ResourceContext | System and cost constraints     |
| DecisionResult  | Final decision output           |
| DecisionTrace   | Step-by-step reasoning          |
| WeightSnapshot  | Historical weight states        |

---

## 5. Key Architectural Patterns

### 5.1 Agent + Strategy Pattern

Agents orchestrate, strategies decide.

This avoids:

- Monolithic service logic
- Hard-coded if/else trees
- Untraceable decision paths

---

### 5.2 Trace-Driven Design

Every decision produces a trace.

This enables:

- Debugging without logs
- UI-level introspection
- Future automated optimization

---

### 5.3 Infrastructure Isolation

- Database logic is isolated
- Storage engines can be swapped
- Core logic remains stable

---

## 6. Evolution Path

### v0.3 (Current)

- Agent abstraction
- Strategy system
- Decision tracing

### v0.4 (Planned)

- Strategy auto-tuning
- Decision replay
- Policy A/B testing

### v0.5 (Future)

- LLM-assisted strategy synthesis
- Cross-agent memory sharing
- Adaptive cost optimization

---

## 7. Non-Goals

The system intentionally avoids:

- Embedding model inference logic
- Hard dependency on any LLM provider
- Tight coupling between UI and decision logic

---

## 8. Summary

The Adaptive Memory System is designed as a **decision-first architecture**.

Memory is not treated as storage,
but as a dynamic, explainable, and optimizable resource.

This architecture ensures long-term maintainability,
research extensibility,
and production-grade reliability.

---

如果你愿意，我可以下一步直接帮你：

- **把这份文档和你现在的代码逐文件对齐，标出“未达标点”**
- 或 **自动生成一个 `ARCHITECTURE_DIAGRAM.md` + SVG 图**
- 或 **写一版“为什么不用传统 Service 架构”的对外说明（适合博客/README）**

你现在这个项目，**已经非常接近“可以被别人 fork 学习”的状态了**。
