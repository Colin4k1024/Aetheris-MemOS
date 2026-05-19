# Next Steps — Aetheris MemOS

> 基于代码库实态扫描 + 产品路线图 + PPT 战略对齐，整理当前每个模块的真实状态与优先级排序。
> 更新时间：2026-05-19

---

## 一、代码实态总览

### 1.1 已完整实现（生产就绪）

| 模块 | 关键文件 | 说明 |
|------|---------|------|
| Memory Kernel + 四层抽象 | `src/kernel/`, `src/layers/` | STM / LTM / KG / MM / Procedural 全部就绪 |
| 自适应调度系统 | `services/scheduler.rs`, `analyzer.rs`, `predictor.rs`, `weight_adjuster.rs` | 任务画像 → 策略路由完整可用 |
| 三位一体混合检索 | `services/memory_search.rs`, `memory_fusion.rs` | Vector + Keyword + Graph 三路融合 |
| 置信度评分 | `services/memory_search.rs` | Quality / Relevance / Recency / Access / Completeness 五维 |
| 上下文压缩 | `services/memory_storage.rs` | sliding_window / importance_prune / llm_summary / hierarchical |
| 多租户 + RBAC + 配额 | `src/tenant/`, `services/rbac.rs` | Member / Admin / SuperAdmin 权限体系 |
| 向量守护 + 自愈 + 证据图 | `services/vector_guard.rs`, `self_healing.rs`, `db/evidence_graph.rs` | 哈希链防篡改，跨模型向量崩溃防护 |
| 记忆摄取 / 反射守护进程 | `services/memory_ingestion.rs` | 后台主动摄取与记忆巩固 |
| 双时态知识图谱 | `db/kg.rs`, `services/` | 快照 / diff / 矛盾检测 |
| Planner Sandbox | `runtime/planner_sandbox.rs` (695 行) | Dry-run 执行沙箱 |
| Subagent Pool | `runtime/subagent_pool.rs` (248 行) | 子智能体池管理 |
| MCP 协议 | `protocol/mcp.rs` (450 行), `mcp/sandbox.rs`, `mcp/signing.rs` | 沙箱隔离 + 签名验证 |
| WebSocket | `protocol/websocket.rs` (429 行) | 实时记忆订阅 |
| Prometheus 导出 | `services/prometheus_exporter.rs` (332 行) | 指标采集就绪 |
| OpenAI / Anthropic 适配器 | `runtime/openai_adapter.rs` (238 行), `anthropic_adapter.rs` (195 行) | 主流模型适配 |
| LangChain 适配器 (Rust) | `runtime/langchain_adapter.rs` (420 行) | Rust LangChain 集成 |
| Python SDK | `sdks/python/adaptive_memory/client.py` (382 行) | 同步客户端基础可用 |
| Rust SDK | `sdks/rust/src/` | client / memory / kg / agent / mcp 模块 |

### 1.2 已存在但为 Stub / 骨架

| 模块 | 关键文件 | 实际状态 | 缺什么 |
|------|---------|---------|--------|
| Redis STM 适配器 | `db/adapters/redis_stm.rs` (496 行) | 实现完整，**未接入默认路径** | config 路由 + 启用开关 + 集成测试 |
| 分布式共识 | `distributed/consensus.rs` (57 行) | 注释明确："In production: would go through Raft" — 纯占位 | Raft 真实实现 |
| 分布式复制 | `distributed/replication.rs` (94 行) | 类型定义，无同步逻辑 | 实际 leader→replica 数据同步 |
| 分布式分片 | `distributed/sharding.rs` (89 行) | 一致性哈希骨架，无路由逻辑 | 一致性哈希分片路由 |
| gRPC 服务 | `protocol/grpc.rs` (100 行) | 只有 proto 数据类型，无 tonic server | tonic server/client wiring |
| Letta Provider | `providers/letta.rs` (74 行) | 所有方法返回 `NotImplemented` | Letta API 真实调用 |
| OTel 完整导出 | `otel/mod.rs` (127 行) | 类型 + context 就绪，无 OTLP exporter 初始化 | tracer provider init + OTLP export 配置 |

### 1.3 完全缺失（Phase 5 战略规划项）

| 项目 | 来源 | 说明 |
|------|------|------|
| LangGraph adapter (Python) | 路线图 Phase 5 | 当前最主流 Agent 编排框架，仅有 LangChain Rust 适配器，Python 生态空白 |
| AutoGen adapter (Python) | 路线图 Phase 5 | Microsoft AutoGen 集成，零进展 |
| MemOS Protocol 规范 | 路线图 Phase 5 | 跨智能体标准化记忆交换格式，未开始设计 |
| 分布式记忆联邦 | 路线图 Phase 5 | 多集群跨节点记忆协调，依赖 Raft consensus 先完成 |
| Aetheris Cloud 控制平面 | 产品生态矩阵 | 托管平台，当前无任何代码 |
| Python asyncio 客户端 | SDK 完善 | 现有 SDK 为同步，Agent 场景需要异步版本 |
| Python LangChain Memory 包装 | SDK 完善 | 将 MemOS 暴露为 LangChain `BaseMemory` 接口 |

---

## 二、优先级排序

### Tier 1 — 生态系统影响最大，可立刻启动

#### T1-A：LangGraph Python Adapter

**为什么优先：** LangGraph 是目前 Agent 编排生态中使用率最高的框架（StateGraph / 持久化 checkpoint）。接入 MemOS 后，任意 LangGraph Agent 可直接使用多层记忆内核，显著扩大用户覆盖面。

**参考实现：** `runtime/langchain_adapter.rs` (420 行) 提供了完整的 Adapter 设计模式，Python 版本复用同一接口契约。

**交付物：**
- `sdks/python/adaptive_memory/langchain/` — LangChain `BaseMemory` wrapper
- `sdks/python/adaptive_memory/langgraph/` — LangGraph checkpointer / memory node
- 集成示例 + 单元测试

---

#### T1-B：Redis STM 接入默认路径

**为什么优先：** 代码已实现（496行），只差配置路由和默认启用，是**投入最小、收益最大**的快赢项。

**工作内容：**
- `config/storage.rs` 新增 `stm_backend: InProcess | Redis` 配置项
- `main.rs` 启动时根据配置选择 STM 后端
- `config.toml.example` 补充 Redis 配置注释
- 集成测试覆盖 Redis 路径

---

#### T1-C：Python SDK 完善

**为什么优先：** 绝大多数 Agent 开发者使用 Python，完整的 SDK 是接入门槛的核心。

**工作内容：**
- asyncio 异步客户端（`client_async.py`）
- LangChain `BaseMemory` 实现（T1-A 合并交付）
- 完整 API 覆盖（STM / LTM / KG / MM / search / compress）
- 类型标注 + docstring
- pip 发布 (`pyproject.toml` 已存在，补充 CI publish 步骤)
- 使用示例文档

---

### Tier 2 — 补完现有半成品

#### T2-A：gRPC tonic Server Wiring

**现状：** `protocol/grpc.rs` 100 行只有数据类型，`proto/` 目录可能缺失。

**工作内容：**
- 补充 `proto/memory_kernel.proto` 定义
- 接入 `tonic` 生成 server stub
- 注册 gRPC 路由到 Axum（或独立端口）
- 覆盖 store / retrieve / search 三个核心 RPC

---

#### T2-B：OpenTelemetry OTLP 完整导出

**现状：** `otel/mod.rs` 类型就绪，无初始化路径；Prometheus 导出已完整。

**工作内容：**
- `main.rs` 补充 OTel tracer provider 初始化（OTLP / stdout）
- 关键路径（adaptive 决策、hybrid search、KG 遍历）添加 span
- `config.toml` 新增 `[otel]` 配置块
- 与 Prometheus 打通（exemplars 关联 trace_id）

---

#### T2-C：Letta Provider 真实实现

**现状：** 接口已定义，所有方法返回 `NotImplemented`。

**参考：** `providers/mem0.rs` (328 行) 提供了完整的第三方 provider 实现模式，Letta 复用同一结构。

**工作内容：**
- 接入 Letta REST API（store / retrieve / search / delete）
- `health_check` 真实检测
- 配置 `[providers.letta]` 块

---

### Tier 3 — 基础设施，工作量大

#### T3-A：Raft Consensus 真实实现

**阻塞链：** consensus → replication → sharding → 分布式联邦（Phase 5）

**建议方案：** 引入 `openraft` crate，接入 `ConsensusModule` 现有接口。

**工作内容：**
- 接入 `openraft`：leader 选举、日志复制、状态机
- 实现 `replication.rs` 中的实际 leader→replica 同步逻辑
- 实现 `sharding.rs` 中的一致性哈希路由
- 集群节点发现（基于 `distributed/node.rs`）
- 端到端集成测试（至少 3 节点 docker-compose 环境）

---

#### T3-B：AutoGen Adapter (Python)

**依赖：** 建议在 T1-A（LangGraph）完成后复用相同的 SDK 集成模式。

**工作内容：**
- `sdks/python/adaptive_memory/autogen/` — AutoGen `Memory` 接口实现
- 注册为 AutoGen tool / memory provider

---

#### T3-C：MemOS Protocol 规范（草案）

**意义：** Phase 5 核心，定义跨智能体标准化记忆交换格式，是"生态互操作"的基础。

**工作内容：**
- 起草 `docs/MEMOS_PROTOCOL.md`：memory entry schema、exchange API、version negotiation
- 参考 OpenAI 的 MCP 协议设计思路
- 在 Python SDK 中实现 protocol client

---

## 三、执行建议

```text
Sprint 1（当前）
├── T1-B  Redis STM 接入        — 2-3天，快赢
├── T1-C  Python SDK asyncio    — 3-4天
└── T2-B  OTel OTLP 初始化      — 2天

Sprint 2
├── T1-A  LangGraph adapter     — 5-7天，核心生态
└── T2-A  gRPC tonic wiring     — 3-4天

Sprint 3
├── T2-C  Letta provider        — 2天
├── T3-B  AutoGen adapter       — 3天（复用 T1-A 模式）
└── T3-C  MemOS Protocol 草案   — 3-4天

Sprint 4+
└── T3-A  Raft consensus        — 2-3周，解锁分布式故事
```

---

## 四、依赖关系图

```text
T1-B (Redis STM)
    └── 独立，可立即启动

T1-C (Python SDK)
    └── T1-A (LangGraph) 依赖此基础

T1-A (LangGraph)
    └── T3-B (AutoGen) 复用此模式

T2-A (gRPC)
    └── 独立，不阻塞其他项

T2-B (OTel)
    └── 独立，不阻塞其他项

T2-C (Letta)
    └── 独立，参考 mem0 模式

T3-A (Raft)
    └── 解锁：分布式联邦 → Aetheris Cloud 基础设施
        └── T3-C (MemOS Protocol) 可并行设计，不依赖 T3-A

Aetheris Cloud
    └── 依赖：T3-A + T3-C + 完整 SDK 生态
```

---

## 五、Open-Core 边界提醒

参见 [OPEN_CORE_BOUNDARY.md](OPEN_CORE_BOUNDARY.md)。以下规划项涉及 Enterprise 模块边界，实现前需确认归属：

| 项目 | 建议归属 |
|------|---------|
| gRPC 服务端 | Core（开放协议） |
| Redis STM 后端 | Core |
| LangGraph / AutoGen adapter | Core（生态接入） |
| Raft Consensus / 分布式联邦 | Enterprise |
| Aetheris Cloud 控制平面 | Enterprise / Commercial |
| MemOS Protocol 规范 | Core（标准化） |
