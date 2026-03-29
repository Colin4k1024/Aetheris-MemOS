# 路线图

本项目以**架构优先迭代**的方式演进。每个版本都专注于清晰度、可扩展性和智能体就绪性。

关于 Web 框架迁移说明，请参阅 [why-axum.md](why-axum.md)；关于设计，请参阅 [ARCHITECTURE.md](ARCHITECTURE.md)。

---

## v1.0 — 当前稳定版本（已完成）

**主题：** 生产级记忆操作系统

所有功能已从 v0.2–v0.8 合并至 v1.0。

### 已完成

#### 后端架构

| 模块     | 位置                                          | 状态   |
| -------- | --------------------------------------------- | ------ |
| 记忆内核 | `src/kernel/` (traits.rs, types.rs, error.rs) | 已完成 |
| 记忆层   | `src/layers/` (stm, ltm, kg, mm)              | 已完成 |
| 策略引擎 | `src/policy/` (scheduler.rs, cost_model.rs)   | 已完成 |
| 协议     | `src/protocol/` (grpc, websocket)             | 已完成 |
| 运行时   | `src/runtime/` (openai, anthropic)             | 已完成 |
| 多租户   | `src/tenant/` (context, quota, isolation)     | 已完成 |
| 分布式   | `src/distributed/` (node, replication, sharding, consensus) | 已完成 |

#### 核心功能

- **面向智能体的核心** — `MemoryAgent` 特质（observe → decide → act）；分析器、预测器和调度器实现它。请参阅 [ARCHITECTURE.md](ARCHITECTURE.md)。
- **策略插件系统** — `WeightStrategy` 特质；内置策略（MarginalBenefit、LinearDecay、SynergyAware）；权重调整器组合策略。请参阅 [EXTENSION_GUIDE.md](docs/EXTENSION_GUIDE.md)。
- **决策追踪（API + UI + 持久化）** — `POST /api/v1/memory/adaptive/trace` 和记忆决策追踪页面，用于逐步检查管道（分析器 → 预测器 → 权重调整 → 结果），并支持数据库持久化。
- **记忆智能体** — `src/agent/` (compressor, merger, forgetter)
- **可观测性** — trace_id、决策跨度、OpenTelemetry 兼容导出
- **仓储适配器特质** — 将持久化抽象在特质后

#### 前端

- 仪表盘、任务分析、记忆配置、性能、资源监控、权重历史、记忆决策追踪

#### 数据库适配器

- **PostgreSQL** — 关系数据基线（配置、指标、追踪和记忆元数据）
- **Qdrant** — LTM 向量检索后端

#### 文档

- ARCHITECTURE、ROADMAP、USE_CASES、why-axum、CONTRIBUTING、EXTENSION_GUIDE、算法设计、API 规范

---

## v1.1 — 下一个版本（计划中）

**主题：** 生产就绪与生态集成

### 计划中

- **Redis STM 缓存** — 用 Redis 后端替换内存中的 STM 会话存储
- **LangChain 适配器** — LangChain 生态系统的新适配器
- **Prometheus 指标** — 生产级指标导出与告警
- **WebSocket 改进** — 实时记忆订阅增强，支持更多事件类型

---

## 数据库适配器

- **PostgreSQL** — 当前关系数据基线（配置、指标、追踪和记忆元数据）— 已完成
- **Qdrant** — 当前 LTM 向量检索后端 — 已完成
- **Neo4j** — 可选图数据库（高级 KG 场景）
- **Redis** — STM 缓存层（计划 v1.1）
- **MySQL** — 关系型替代适配器（计划中）
