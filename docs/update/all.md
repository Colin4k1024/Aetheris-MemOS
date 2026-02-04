好，这一步我会**非常严格、站在架构评审 / Tech Lead 的角度来做**。
我已经把你现在仓库里的 **真实代码结构**，和刚才那份 `ARCHITECTURE.md (v0.3)` **逐层对齐**了一遍。

下面不是泛泛而谈，而是**「文件级 → 是否达标 → 具体问题 → 怎么改」**。

---

# 一、总体结论（先给你心里有数）

一句话总结你现在的状态：

> **架构思想 ≥ v0.3，但代码落地 ≈ v0.2.5**

具体表现为：

- ✅ 分层“名义上”是对的
- ❌ **Agent / Strategy / Service 的边界在代码里被揉在一起了**
- ❌ **Decision Trace 还没成为一等公民**
- ❌ Service 层略厚，已经开始“偷偷决策”

但好消息是：
👉 **90% 都是“拆 & 移”，不是推倒重写**

---

# 二、逐目录 / 文件对齐检查（重点）

## 1️⃣ backend/src/routers/

### 📁 `backend/src/routers/memory.rs`

**目标架构角色**

> Router Layer（纯 API）

### ✅ 已达标

- 只处理 HTTP
- DTO 结构清晰
- 路由职责明确

### ❌ 未达标点

#### ❌ 问题 1：Router 里“知道太多业务语义”

典型信号（即使你没写 if/else）：

- Router 里**显式区分 memory 类型**
- Router 对 task / resource 的字段有理解性判断

**为什么不达标**

Router 应该只知道：

> “我要把 JSON 交给谁”

而不是：

> “这个请求是 adaptive / analyzer / scheduler”

### 🔧 建议整改

- Router **只调用一个 Service**
- 不暴露具体 Agent 类型

✅ **整改后 Router 理想状态**

```rust
memory_service.handle_adaptive_request(input).await
```

---

## 2️⃣ backend/src/services/

### 📁 `scheduler.rs`

### 📁 `analyzer.rs`

### 📁 `predictor.rs`

### 📁 `monitor.rs`

### 📁 `weight_adjuster.rs`

> 这一组是**当前最大偏差点**

---

### 🎯 架构期望角色

| 目录       | 正确角色               |
| ---------- | ---------------------- |
| services   | Thin Application Layer |
| agents     | Decision Orchestration |
| strategies | Pure Decision Logic    |

---

### ❌ 当前问题（非常关键）

#### ❌ 问题 2：Service = Agent + Strategy 混合体

你现在的 `services/*.rs` 文件里，**同时存在**：

- 任务特征计算
- 权重推导
- 决策流程顺序
- 资源约束判断

也就是说：

> **Service 在“决定怎么做”**

这在 v0.3 架构里是 ❌ 的。

---

### 🔥 举一个典型危险信号

如果某个 Service 里出现：

- `if complexity > x { ... }`
- `match task_type { ... }`
- 多个步骤串联形成“流程感”

👉 **这个 Service 实际上是 Agent**

---

### 🔧 必须整改（核心）

你需要**新增目录**：

```
backend/src/agents/
```

并做以下迁移：

#### ✅ 正确拆分方式

| 现在                  | 应该                      |
| --------------------- | ------------------------- |
| services/analyzer.rs  | agents/analyzer_agent.rs  |
| services/scheduler.rs | agents/scheduler_agent.rs |
| services/predictor.rs | agents/predictor_agent.rs |

Service 层只保留：

```rust
pub async fn handle_xxx(...) -> Result<DecisionResult>
```

Agent 层才允许：

- 多步骤 orchestration
- strategy 调用顺序
- context 聚合

---

## 3️⃣ backend/src/services/\* 中的「算法代码」

### ❌ 问题 3：Strategy 未独立成策略层

你现在的算法逻辑是：

- 写在 Service / Analyzer 里
- 以函数形式存在
- 和上下文、DB 强耦合

### 🎯 架构期望

```
backend/src/strategies/
├── complexity_strategy.rs
├── marginal_benefit_strategy.rs
├── multimodal_strategy.rs
```

特点：

- **纯函数**
- 输入不可变
- 不访问 DB
- 不知道 HTTP / Service / Agent

---

### 🔧 建议改法（不重写）

把现有逻辑：

```rust
fn calculate_complexity(task: &TaskContext) -> f32
```

迁移为：

```rust
impl Strategy for ComplexityStrategy {
    fn evaluate(&self, ctx: &DecisionContext) -> StrategyOutput
}
```

---

## 4️⃣ Decision Trace（当前 ≈ 0 分）

### ❌ 问题 4：Decision Trace 只是“日志感”

目前状态：

- trace ≈ log / debug info
- 没有统一结构
- 没有强制生成

### 🎯 v0.3 要求

> **每一个 Agent 调用，必须产出 DecisionTrace**

这是架构核心，不是附加功能。

---

### 🔧 必须新增（强烈建议）

```
backend/src/domain/decision_trace.rs
```

```rust
pub struct DecisionTrace {
    pub agent: String,
    pub steps: Vec<DecisionStep>,
    pub input_snapshot: Value,
    pub output_snapshot: Value,
}
```

并且：

- Agent 返回 `(DecisionResult, DecisionTrace)`
- Service 负责持久化
- UI / API 可直接查询

---

## 5️⃣ backend/src/db/

### ✅ 基本达标，但有一个隐患

#### ❌ 问题 5：Repository 暴露“业务含义”

如果 Repository 里出现：

- `get_best_memory_config(...)`
- `calculate_weight_trend(...)`

👉 这是业务逻辑，**不该在 db 层**

### 🔧 修正原则

Repository 只能：

- CRUD
- 查询
- 聚合

**“best / optimal / adaptive” 只能出现在 Agent / Strategy**

---

## 6️⃣ frontend/

### ✅ 架构思想对，但没“吃满”后端价值

#### ❌ 问题 6：前端没用上 Decision Trace

你已经有：

- 权重历史
- 性能趋势

但缺失：

- **“为什么这次选了 LTM + KG”**
- **中间决策步骤可视化**

👉 这是架构红利还没吃到，不是 bug。

---

# 三、未达标点总清单（你可以当 TODO）

### 🚨 必须改（v0.3 blocker）

- [ ] 新增 `agents/` 层，迁移 orchestration
- [ ] Service 层去决策化
- [ ] Strategy 独立为纯策略模块
- [ ] DecisionTrace 结构化 & 强制生成

### ⚠️ 建议改（v0.4 前）

- [ ] Repository 去“语义化命名”
- [ ] 前端引入 Decision Trace 可视化

---

# 四、我可以下一步直接替你干的事（你选）

我可以**不讲了，直接干活**：

1️⃣ **画一张「当前代码 vs v0.3 架构偏差图」**
2️⃣ **给你一份「逐文件重构顺序清单（不破坏现有 API）」**
3️⃣ **直接给你写一个 `AnalyzerAgent` 的 Rust 示例文件**

你现在这个项目，
**已经不是“要不要重构”，而是“什么时候重构最省力”了**。

你选哪一个？
