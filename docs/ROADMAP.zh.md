# 路线图

本项目以**架构优先迭代**的方式演进。每个版本都专注于清晰度、可扩展性和智能体就绪性。

关于 Web 框架迁移说明，请参阅 [why-axum.md](why-axum.md)；关于设计，请参阅 [ARCHITECTURE.md](ARCHITECTURE.md)。

---

## v0.2 — 稳定的基于规则的自适应记忆（已完成）

- **后端**: 基于规则的调度器、分析器、预测器、监控器、权重调整器；PostgreSQL + Qdrant 基线；REST API。
- **前端**: 仪表盘、任务分析、记忆配置、性能、资源监控、权重历史。
- **文档**: 算法设计、API 规范、系统可视化。

---

## v0.3 — 可扩展与智能体就绪（进行中）

**主题：** 使系统可组合、可解释，并开放扩展。

### 已完成

- **面向智能体的核心** — `MemoryAgent` 特质（observe → decide → act）；分析器、预测器和调度器实现它。请参阅 [ARCHITECTURE.md](ARCHITECTURE.md)。
- **策略插件系统** — `WeightStrategy` 特质；内置策略（MarginalBenefit、LinearDecay、SynergyAware）；权重调整器组合策略。请参阅 [EXTENSION_GUIDE.md](docs/EXTENSION_GUIDE.md)。
- **决策追踪（API + UI + 持久化）** — `POST /api/v1/memory/adaptive/trace` 和记忆决策追踪页面，用于逐步检查管道（分析器 → 预测器 → 权重调整 → 结果），并支持数据库持久化。
- **存储基线对齐** — PostgreSQL 作为关系数据基线，Qdrant 用于向量检索，Neo4j 为可选图数据库。
- **文档** — ARCHITECTURE、ROADMAP、USE_CASES、why-axum；CONTRIBUTING 和 EXTENSION_GUIDE。

### 计划中

- **可观测性** — trace_id、决策跨度、OpenTelemetry 兼容导出；指标关联。
- **仓储适配器特质** — 将持久化抽象在特质后；运行时适配器选择（可选）。

---

## v0.4（计划中）

- **可选的 LLM 集成** — 在 `MemoryAgent` 后可插拔的 LLM 驱动的分析器或预测器。
- **Axum 后端迁移完成** — 后端已切换为 Axum，并保持 API 兼容；后续继续进行生态对齐（请参阅 [why-axum.md](why-axum.md)）。

---

## v0.5 — 记忆内核正式版（Q1）

**主题：** 统一记忆内核架构

### 架构模块

| 模块     | 位置                                          | 状态   |
| -------- | --------------------------------------------- | ------ |
| 记忆内核 | `src/kernel/` (traits.rs, types.rs, error.rs) | 已实现 |
| 记忆层   | `src/layers/` (stm, ltm, kg, mm)              | 已实现 |
| 策略引擎 | `src/policy/` (scheduler.rs, cost_model.rs)   | 已实现 |

### 计划中

- **内核集成** — 重构调度器以使用统一的内核接口
- **Redis STM 缓存** — 用 Redis 后端替换内存中的 STM
- **Qdrant 集成** — LTM 层的向量搜索
- **Neo4j 集成** — KG 层的图查询

---

## v0.6 — 智能体运行时集成（Q2）

**主题：** 原生智能体运行时 SDK

### 架构模块

| 模块         | 位置                                         | 状态   |
| ------------ | -------------------------------------------- | ------ |
| 记忆智能体   | `src/agent/` (compressor, merger, forgetter) | 已实现 |
| 运行时适配器 | `src/runtime/` (openai, anthropic)           | 已实现 |

### 计划中

- **OpenAI 智能体 SDK** — 完成适配器实现
- **Anthropic Claude** — 完成适配器实现
- **LangChain 适配器** — LangChain 生态系统的新适配器
- **LLM 压缩** — STM→LTM 的智能摘要

---

## v0.7 — 生产级 API 网关（Q3）

**主题：** 企业级 API

### 架构模块

| 模块   | 位置                                      | 状态   |
| ------ | ----------------------------------------- | ------ |
| 协议   | `src/protocol/` (grpc, websocket)         | 已实现 |
| 多租户 | `src/tenant/` (context, quota, isolation) | 已实现 |

### 计划中

- **gRPC 服务** — 基于 tonic 的 gRPC API
- **WebSocket** — 实时记忆订阅
- **多租户** — 完成租户隔离
- **认证** — JWT + API Key 认证中间件

---

## v0.8 — 分布式集群（Q4）

**主题：** 水平可扩展记忆

### 架构模块

| 模块   | 位置                                                        | 状态   |
| ------ | ----------------------------------------------------------- | ------ |
| 分布式 | `src/distributed/` (node, replication, sharding, consensus) | 已实现 |

### 计划中

- **节点发现** — 集群成员和心跳
- **复制** — 多副本同步
- **分片** — 基于一致性哈希的分片
- **共识** — 领导者选举（Raft）

---

## v1.0 — 智能体记忆操作系统（明年 Q1）

**主题：** 生产级记忆操作系统

| 功能       | 状态                       |
| ---------- | -------------------------- |
| 记忆内核   | 生产就绪                   |
| 智能体集成 | OpenAI/Anthropic/LangChain |
| 协议       | gRPC/REST/WS + 认证        |
| 多租户     | 完全租户隔离               |
| 分布式     | 集群支持                   |
| 可观测性   | Prometheus + Tracing       |

---

## 数据库适配器

- **PostgreSQL** — 当前关系数据基线（配置、指标、追踪和记忆元数据）。
- **Qdrant** — 当前 LTM 向量检索后端。
- **Neo4j** — 可选图数据库（高级 KG 场景）。
- **Redis** — STM 缓存层（计划 v0.5）。
- **MySQL** — 关系型替代适配器（计划中）。
