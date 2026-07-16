# P0 建前清理方案 — Aetheris-MemOS 企业级生产化

- **日期**：2026-07-16
- **主责角色**：backend-engineer（本方案：读+分析+设计）/ 主控（应用 + `cargo check` 验证）
- **关联入口**：`delivery-plan.md`（P0 建前清理里程碑）→ 定位「企业级可售卖」+「分布式不自建，HA 走托管基建」
- **性质**：**设计交付物**，非代码改动。全部改动由主控应用后离线编译验证。
- **验证口径**：`cd backend && SQLX_OFFLINE=true cargo check --offline`（默认 features）。D2a（a2a）feature 默认关，仅 `cargo check --features a2a` 可验（需联网拉 a2a 依赖，本机不可）。

---

## 结论先行

1. **可安全删除**：`distributed/` 下 4 个假集群文件（`consensus.rs` / `replication.rs` / `sharding.rs` / `node.rs`）——已 grep 全仓，**零外部调用方**、无 `#[cfg(test)]`、无 OpenAPI schema 注册。删除后仅需改 `distributed/mod.rs` 一处（去除对应 `pub mod` + 再导出 + `DistributedConfig`/`ConsensusMethod`）。`main.rs` / `axum_routers/distributed.rs` / `routers/distributed.rs` **无需改动**（它们只用到保留的 epoch/interrupt/signaling 原语）。
2. **能力声明对齐**：`a2a/agent_card.rs` 去掉 `streaming:true` 与 5 个未实现 skill 广告（3 处 Edit）；`CLAUDE.md` 6 处「声明>实现」表述改为诚实状态（含 3 个根本不存在的 distributed 端点）。
3. **死产物标注**：`strategy_mutator` 是**在跑的常驻守护**（`auto_mutate` 默认 `true`），产出零消费却打「self-optimizing」误导日志 → 在 `main.rs` 停用 + 加状态横幅（不删模块，保留为 P3 候选）；`protocol/grpc.rs`、`protocol/websocket.rs` 为纯类型壳、零消费 → 加 P2 状态文档 + `#![allow(dead_code)]`。
4. **超范围新发现（R4，强烈建议纳入本次 P0）**：`services/enterprise.rs` + `routers/enterprise.rs` 是**第二套假集群，且是已挂载的活路由**（`/enterprise/cluster/*`、`/enterprise/shards/*`，共 9 个端点），内存 HashMap 假 Raft 选举 + 假分片。比 `distributed/` stub 更违背「企业级可售卖」红线（对外暴露的假控制面）。需 lead 决策是否本轮删除（附完整 turnkey 方案）。
5. **需 lead 决策项**：R1（`lease_coordinator` 实测零消费，与「真在用」判断不符）、R2（agent_card 离线不可编译验证）、R3（mutator 停用 vs 删除）、R4（enterprise 活假集群是否本轮删）。

---

## 交付物 1：分布式假集群删除

### 1.1 删除清单（4 文件）

| 文件 | 内容性质 | 零调用方证据 |
|---|---|---|
| `backend/src/distributed/consensus.rs` | 假共识（`ConsensusModule`，注释「In production: would go through Raft consensus」）| `ConsensusModule` 全仓仅 `mod.rs:18` 再导出；grep 业务代码 0 命中 |
| `backend/src/distributed/replication.rs` | 假复制（`ReplicationManager`/`ReplicaState`/`ReplicationConfig`）| 三型全仓仅 `mod.rs:22` 再导出；grep 0 命中 |
| `backend/src/distributed/sharding.rs` | 假分片（`ShardManager`/`ShardKey`/`ShardPlacement`）| 三型全仓仅 `mod.rs:23` 再导出；grep 0 命中 |
| `backend/src/distributed/node.rs` | 假节点（`MemoryNode`/`NodeId`/`NodeInfo`/`NodeRole`）| `NodeId`/`NodeInfo` grep 全仓（排除 `distributed/`）**0 命中**；仅被 `mod.rs` 与已删同目录文件引用 |

**验证命令与结果**（已执行）：
```
grep -rn -E "ConsensusModule|ConsensusMethod|ReplicaState|ReplicationConfig|ReplicationManager|ShardKey|ShardManager|ShardPlacement|DistributedConfig|MemoryNode|NodeRole|NodeResources" --include='*.rs' src/ | grep -v "src/distributed/"
# 唯一命中来自 src/routers/enterprise.rs + src/services/enterprise.rs 的
# EnterpriseShardManager / ClusterNodeRole 等（子串误命中，属另一套模块，见 R4），
# distributed/ 自己的类型 0 外部命中。

grep -rn -E "\bNodeId\b|\bNodeInfo\b" --include='*.rs' src/ | grep -v "src/distributed/"
# >> ZERO hits

grep -ln "cfg(test)" src/distributed/{consensus,replication,sharding,node}.rs
# >> 4 文件均无 #[cfg(test)]
```

**编译安全旁证**（已执行）：
- 保留的 4 原语文件（`epoch_manager.rs` / `interrupt_propagator.rs` / `lease_coordinator.rs` / `signaling_bus.rs`）grep 对被删模块/类型 **0 引用** → 删除不会破坏保留原语。
- 无任何 `utoipa` `components(schemas(...))` 注册被删类型 → 删除不会悬挂 OpenAPI 引用。
- `ulid` 依赖被 `node.rs` 用到，但全仓另有约 30 处使用（如 `services/enterprise.rs:13`）→ 删 `node.rs` 不会孤立该依赖。

### 1.2 连带引用修法：仅 `distributed/mod.rs` 一处

`backend/src/distributed/mod.rs` 全文替换。

**old（当前 1–54 行全文）**：
```rust
//! Distributed Memory Node
//!
//! This module provides distributed memory node capabilities:
//! - Node abstraction
//! - Replication
//! - Sharding
//! - Consensus

pub mod consensus;
pub mod epoch_manager;
pub mod interrupt_propagator;
pub mod lease_coordinator;
pub mod node;
pub mod replication;
pub mod sharding;
pub mod signaling_bus;

pub use consensus::ConsensusModule;
pub use epoch_manager::{CancellationFunc, EpochContext, EpochManager, RegisteredContext};
pub use interrupt_propagator::InterruptPropagator;
pub use node::{MemoryNode, NodeId, NodeInfo, NodeRole};
pub use replication::{ReplicaState, ReplicationConfig, ReplicationManager};
pub use sharding::{ShardKey, ShardManager, ShardPlacement};

/// Distributed system configuration.
#[derive(Debug, Clone)]
pub struct DistributedConfig {
    pub node_id: NodeId,
    pub cluster_id: String,
    pub replication_factor: usize,
    pub shard_count: usize,
    pub consensus_method: ConsensusMethod,
}

impl Default for DistributedConfig {
    fn default() -> Self {
        Self {
            node_id: NodeId::new(),
            cluster_id: "default".to_string(),
            replication_factor: 3,
            shard_count: 16,
            consensus_method: ConsensusMethod::Raft,
        }
    }
}

/// Consensus method for distributed coordination.
#[derive(Debug, Clone, Copy)]
pub enum ConsensusMethod {
    Raft,
    Paxos,
    MultiPaxos,
}
```

**new（替换为）**：
```rust
//! In-process coordination primitives (single process — NOT a self-built cluster).
//!
//! HA / replication / sharding are delegated to managed infrastructure
//! (PostgreSQL / Qdrant / Neo4j), per the "distributed not self-built"
//! architecture decision (delivery-plan.md). This module only holds the
//! in-process primitives that are actually wired into the request path:
//! - `epoch_manager` / `interrupt_propagator`: cooperative cancellation across epochs
//! - `lease_coordinator`: in-process lease bookkeeping
//! - `signaling_bus`: per-workflow signal routing for the sub-agent pool
//!
//! NOTE: the module is intentionally still named `distributed` to bound P0 blast
//! radius. Renaming to e.g. `coordination` is deferred (would touch every
//! `crate::distributed::*` path site with no functional gain) — see p0-cleanup-plan.

pub mod epoch_manager;
pub mod interrupt_propagator;
pub mod lease_coordinator;
pub mod signaling_bus;

pub use epoch_manager::{CancellationFunc, EpochContext, EpochManager, RegisteredContext};
pub use interrupt_propagator::InterruptPropagator;
```

### 1.3 无需改动的连带点（已核实）

| 文件 | 现状 | 为何安全 |
|---|---|---|
| `backend/src/main.rs:101` | `crate::axum_routers::distributed::init_distributed();` | 只初始化 `EpochManager`/`InterruptPropagator`（保留原语），不触被删类型 |
| `backend/src/axum_routers/distributed.rs` | `use crate::distributed::{EpochManager, InterruptPropagator};`（第 8 行）| 仅用保留原语；`mod.rs` new 版仍再导出这两个 |
| `backend/src/routers/distributed.rs` | `use crate::distributed::signaling_bus::{SignalingBus, WorkflowSignal};`（第 13 行）| 走 `pub mod signaling_bus` 全路径，new 版保留该 `pub mod` |
| `backend/src/main.rs:15` | `mod distributed;` | 模块本身保留，无需改 |

### 1.4 应用后编译风险点（D1）

- **风险：低**。理论上删除后可能出现「保留的再导出 `CancellationFunc`/`EpochContext`/`RegisteredContext` 未被内部使用」的告警——但 `pub use` 属公共 API，不触发 `unused_imports` error，最多是既有 dead-code 告警级别，不阻断。
- **命名保留**：不改 `distributed` 名（改名 blast radius 大、无功能收益），仅在 mod.rs 顶部注释澄清「单进程、非自建集群」。符合任务「若改名 blast radius 大则只标注」。

### 1.5 需 lead 决策 R1：`lease_coordinator` 实测零消费

- lead 交付说明把 `lease_coordinator` 列为「真在用」；**实测 grep 全仓（排除自身文件）对 `LeaseCoordinator`/`lease_coordinator` 0 命中**，仅 `mod.rs:14` 的 `pub mod lease_coordinator;` 引用它，且 `mod.rs` 从不再导出其类型。
- 即：它编译进 crate 但**没有任何消费方**，与 epoch/interrupt/signaling（有活跃调用）不同。
- **本方案处置**：按 lead 指令**保留**（new 版 mod.rs 保留 `pub mod lease_coordinator;`），保留是编译安全的（仅 dead-code 告警）。**请 lead 确认**：保留现状 / 加「暂未接线」文档标注 / 一并删除。

---

## 交付物 2：能力声明对齐

### 2a. `backend/src/a2a/agent_card.rs`（3 处 Edit）

> **R2 前提**：a2a 为默认关闭 feature（`Cargo.toml` 依赖已注释），本文件默认**不参与编译**。以下 Edit 语义正确，但**离线无法编译验证**，仅 `cargo check --features a2a`（需联网）可验。P2 启用 A2A 时这些修改是正确起点。

**Edit 1 — `streaming: Some(true)` → `Some(false)`**（第 13–18 行）
old：
```rust
        capabilities: AgentCapabilities {
            streaming: Some(true),
            push_notifications: Some(false),
            extensions: None,
            extended_agent_card: None,
        },
```
new：
```rust
        capabilities: AgentCapabilities {
            // streaming not implemented yet (P2). Do not advertise until the
            // handler produces real incremental results.
            streaming: Some(false),
            push_notifications: Some(false),
            extensions: None,
            extended_agent_card: None,
        },
```

**Edit 2 — 5 个未实现 skill 广告清空**（第 19–85 行，整个 `skills: vec![...]`）
old：整段 `skills: vec![ AgentSkill {id:"memory_search"...}, ... "knowledge_graph"... ],`（19–85 行五个 `AgentSkill`）
new：
```rust
        // Skills intentionally empty until backed by real memory operations (P2).
        // The handler currently returns placeholder text (see a2a/handler.rs), so
        // advertising skills would over-claim capability. Re-populate when wired.
        skills: vec![],
```

**Edit 3 — 移除因 Edit 2 而未使用的 `AgentSkill` 导入**（第 1 行）
old：
```rust
use a2a::agent_card::{AgentCapabilities, AgentCard, AgentInterface, AgentProvider, AgentSkill};
```
new：
```rust
use a2a::agent_card::{AgentCapabilities, AgentCard, AgentInterface, AgentProvider};
```
> 注：`--features a2a` 下若不移除 `AgentSkill` 会触发 `unused_imports`（warning，deny 时报 error）。故与 Edit 2 配套。

### 2b. `CLAUDE.md`（6 处，纯文档，零编译风险）

**Edit 1 — Project Overview「Adaptive」措辞**（第 7 行）
old：
```
Adaptive Memory Management System for AI Agent & LLM workloads. Uses Rust (Axum) backend with React (Ant Design Pro) frontend.
```
new：
```
Memory Management System for AI Agent & LLM workloads, with an adaptive-scheduling roadmap (config selection is currently heuristic/static; learned adaptation is planned for P3). Uses Rust (Axum) backend with React (Ant Design Pro) frontend.
```

**Edit 2 — scheduler 描述**（第 41 行）
old：
```
  - `scheduler.rs` — Adaptive memory scheduler (selects optimal memory config)
```
new：
```
  - `scheduler.rs` — Memory scheduler (selects memory config via heuristic/static policy; learned adaptation planned for P3)
```

**Edit 3 — predictor 描述**（第 43 行）
old：
```
  - `predictor.rs` — Performance prediction model
```
new：
```
  - `predictor.rs` — Performance prediction (fixed-coefficient model; fit-from-telemetry planned for P3)
```

**Edit 4 — Distributed & Signaling Endpoints 表（3 个幽灵端点）**（第 111–117 行）
> 现表列出 `/nodes`、`/node/{id}`、`/replicate` **代码中根本不存在**；真实端点来自 `routers/distributed.rs` + `axum_routers/distributed.rs`。
old：
```
### Distributed & Signaling Endpoints

| Method | Endpoint                          | Description              |
| ------ | --------------------------------- | ------------------------ |
| GET    | `/api/v1/distributed/nodes`       | List cluster nodes       |
| GET    | `/api/v1/distributed/node/{id}`  | Get node status          |
| POST   | `/api/v1/distributed/replicate`  | Trigger replication      |
```
new：
```
### Coordination & Signaling Endpoints

> In-process coordination only (single node). No self-built cluster — HA is delegated to managed infra.

| Method | Endpoint                                      | Description                        |
| ------ | --------------------------------------------- | ---------------------------------- |
| GET    | `/api/v1/distributed/epoch/status`            | Epoch / active-context status      |
| GET    | `/api/v1/distributed/pool/status`             | Sub-agent pool status              |
| POST   | `/api/v1/distributed/pool/allocate`           | Allocate sub-agent slots           |
| POST   | `/api/v1/distributed/pool/release`            | Release sub-agent slots            |
| GET    | `/api/v1/distributed/signals/{workflow_id}`   | Get signals for a workflow         |
| POST   | `/api/v1/distributed/signals/publish`         | Publish a workflow signal          |
```

**Edit 5 — Multi-Tenant Isolation（应用层 vs schema 级）**（第 164–166 行）
old：
```
### Multi-Tenant Isolation

All repository queries are scoped by `tenant_id`. Use `TenantContext` extractor to access tenant info in handlers. See `backend/src/tenant/isolation.rs`.
```
new：
```
### Multi-Tenant Isolation

Repository queries are scoped by `tenant_id` at the **application layer** today. Use the `TenantContext` extractor to access tenant info in handlers. See `backend/src/tenant/isolation.rs`. Schema-level enforcement (Postgres RLS + `tenant_id NOT NULL`) is planned for P1 — until then isolation depends on every query path passing the correct `tenant_id`.
```

**Edit 6 — MCP Sandbox（签名真、执行 mock）**（第 168–170 行）
old：
```
### MCP Sandbox

MCP tool calls run in isolated sandboxes with signing verification. See `backend/src/mcp/sandbox.rs` and `backend/src/mcp/signing.rs`.
```
new：
```
### MCP Sandbox

MCP tool calls are **signature-verified** on the `call_tool` path (real; see `backend/src/mcp/signing.rs`). Sandboxed **execution isolation** is currently a mock that validates the policy framework only (`backend/src/mcp/sandbox.rs` — wasmtime is imported but `WasmSandbox` does not yet enforce real isolation); true wasmtime isolation is planned for P1 (see ADR-0004).
```

---

## 交付物 3：死产物标注

### 3a. `strategy_mutator` — 停用常驻守护 + 状态横幅（R3）

**准确定性**（修正 lead「零消费产物」措辞）：它不是静态产物，而是**在跑的常驻 daemon**——`MutationConfig::default().auto_mutate == true`（`strategy_mutator.rs:47`），故 `main.rs:94` 条件成立、`init_mutation_daemon` 每 600s 跑一轮并打日志 `"StrategyMutator daemon started"` / `"Strategy mutation accepted: score X → Y"`。但其产出 `current_hyperparams()` / `BEST_PARAMS` **全仓零消费**（grep 除自身文件外仅命中 `main.rs:94-96` 的启动调用；scheduler/predictor 从不读它）。即：**空转 + 打「self-optimizing」误导日志**，与「企业级自适应」名实不符。

**Edit 1 — `backend/src/main.rs` 停用守护**（第 93–98 行）
old：
```rust
    // Issue #55: start adaptive strategy mutation daemon
    if crate::services::strategy_mutator::MutationConfig::default().auto_mutate {
        crate::services::strategy_mutator::StrategyMutator::init_mutation_daemon(
            crate::services::strategy_mutator::MutationConfig::default(),
        );
    }
```
new：
```rust
    // Issue #55: adaptive strategy mutation daemon — DISABLED (P0 cleanup).
    // The mutator runs a heuristic hill-climb but its output
    // (strategy_mutator::current_hyperparams / BEST_PARAMS) is consumed by
    // nothing — scheduler/predictor never read it — so running it only emits
    // misleading "self-optimizing" logs. Re-enable in P3 once the scheduler
    // actually consumes learned hyperparameters. Manual run_mutation_cycle()
    // remains available for experiments.
    // if crate::services::strategy_mutator::MutationConfig::default().auto_mutate {
    //     crate::services::strategy_mutator::StrategyMutator::init_mutation_daemon(
    //         crate::services::strategy_mutator::MutationConfig::default(),
    //     );
    // }
```

**Edit 2 — `backend/src/services/strategy_mutator.rs` 顶部状态横幅**（第 1–11 行文档注释）
old：
```rust
/// 策略变异服务 — Issue #55
///
/// 基于历史性能指标自动调整 WeightStrategy 超参数，
/// 实现自适应进化（greedy hill-climbing + random perturbation 混合）。
///
/// 工作机制：
/// 1. 从数据库读取近 N 窗口的性能指标（accuracy, coherence, response_time）
/// 2. 计算当前权重配置的综合评分
/// 3. 生成若干候选突变（小幅随机扰动 ± delta）
/// 4. 贪心选择：若突变后预测评分更高，则接受
/// 5. 将变异结果写入权重历史（决策轨迹）并返回新权重建议
```
new：
```rust
/// 策略变异服务 — Issue #55
///
/// ⚠️ P0 状态：自动守护已在 main.rs 停用（DISABLED）。
/// 本模块产出（`current_hyperparams` / `BEST_PARAMS`）当前**无任何消费方**——
/// scheduler / predictor 均不读取；启发式 `estimate_candidate_score` 也非实测拟合。
/// 保留为 P3「自适应核心」候选：届时需先让 scheduler 真正消费学习到的超参数，
/// 再重新启用守护。手动 `run_mutation_cycle()` 可用于离线实验。
///
/// 工作机制（设计意图，尚未接入决策链路）：
/// 1. 从数据库读取近 N 窗口的性能指标（accuracy, coherence, response_time）
/// 2. 计算当前权重配置的综合评分
/// 3. 生成若干候选突变（小幅随机扰动 ± delta）
/// 4. 贪心选择：若突变后预测评分更高，则接受
/// 5. 将变异结果写入权重历史（决策轨迹）并返回新权重建议
```

**编译风险（3a）**：低。停用后 `init_mutation_daemon` / `run_mutation_cycle` 等变为未调用 → 既有 dead-code 告警可能新增几条，不阻断（默认 warn）。若 CI `-D warnings`，主控可在 `impl StrategyMutator` 上加 `#[allow(dead_code)]`。**请 lead 决策 R3**：停用+标注（本方案推荐，保留 P3 复用）vs 直接删除模块。

### 3b. `backend/src/protocol/grpc.rs` — 纯类型壳标注

零消费证据：`grep -rn -E "Grpc[A-Za-z]+" --include='*.rs' src/ | grep -v protocol/grpc.rs` → **0 命中**。所有 `Grpc*` 类型无消费方；无 tonic server 实装。

**Edit — 顶部文档 + crate 级 allow**（第 1–3 行）
old：
```rust
//! gRPC Protocol Definition
//!
//! This module provides gRPC service definitions for the Memory Kernel.
```
new：
```rust
//! gRPC Protocol Definition — NOT IMPLEMENTED (pending P2).
//!
//! This module only holds hand-written Rust structs that mirror what a protobuf
//! codegen *would* produce. There is no tonic server/client wired anywhere — all
//! `Grpc*` types below have zero consumers (verified). Kept as a P2 starting point;
//! do not treat as a working gRPC surface.
#![allow(dead_code)]
```
> 注：`#![allow(dead_code)]` 为内部属性，须紧跟模块文档注释、位于任何 `use` 之前（当前 `use crate::kernel::types::*;` 在第 15 行，满足）。

**编译风险（3b）**：无。仅抑制告警。

### 3c. `backend/src/protocol/websocket.rs` — 纯类型壳标注

零消费证据：`grep -rn -E "WsMessageType|WsConnectionManager|WsMessage|WsEnvelope|WsConnection" --include='*.rs' src/ | grep -v protocol/websocket.rs` → **0 命中**（连接管理器仅在自身 `#[cfg(test)]` 内被构造，未注册进任何路由；`send_to_session` 为返回 `true` 的占位）。

**Edit — 顶部文档 + crate 级 allow**（第 1–3 行）
old：
```rust
//! WebSocket Protocol
//!
//! This module provides WebSocket message types and connection management for real-time memory operations.
```
new：
```rust
//! WebSocket Protocol — NOT IMPLEMENTED (pending P2).
//!
//! Message types + an in-memory connection manager exist, but nothing is wired to
//! an axum WebSocket route and `send_to_session` is a placeholder (returns true).
//! All `Ws*` types have zero non-test consumers (verified). Kept as a P2 starting
//! point; do not treat as a working real-time surface.
#![allow(dead_code)]
```

**编译风险（3c）**：无。仅抑制告警。

---

## R4（新发现 · 超原范围 · 强烈建议纳入 P0 · 需 lead 决策）：`enterprise.rs` 活假集群

### 现象

`grep` 交付物 1 时发现**第二套自建假集群**，且危害高于 `distributed/` stub——它是**已挂载的活 HTTP 路由**：

- `backend/src/services/enterprise.rs`：`ClusterManager`（内存 `HashMap` 假集群 + 假 Raft：`become_leader` 自增 `term`、心跳、「simplified election」注释）+ `EnterpriseShardManager`（内存 `HashMap` 假分片）。全部单进程内存态。文件头自称「Enterprise Cloud Platform - Distributed System Support / High availability election / Data sharding」。
- `backend/src/routers/enterprise.rs` + `backend/src/routers/mod.rs:227-239`：挂载 9 个活端点：
  - `POST /enterprise/cluster/node`、`GET /enterprise/cluster/nodes`、`GET /enterprise/cluster/active`、`GET /enterprise/cluster/leader`、`POST /enterprise/cluster/become-leader`、`GET /enterprise/cluster/is-leader`
  - `POST /enterprise/shards`、`GET /enterprise/shards`、`GET /enterprise/shards/{key}`

企业买家调用 `/enterprise/cluster/become-leader` 得到的是单进程内存玩具——正是「分布式不自建」决策要清除的、且是「声明>实现」红线中最严重的一类（对外暴露的假控制面）。

### 关键区分（勿误删）

- `hoops::enterprise` / `enterprise_hooks_v2` / `enterprise_impl`（治理 hooks：RBAC/配额/审计）是**另一命名空间**，被 `routers/dashboard.rs` 合法引用，是 P1 要接线的真能力 → **不在 R4 范围，禁止改动**。
- R4 仅指 `services::enterprise` + `routers::enterprise`（cluster/shard 假集群）。

### Turnkey 删除方案（若 lead 批准纳入 P0）

1. 删除 `backend/src/routers/enterprise.rs`。
2. 删除 `backend/src/services/enterprise.rs`。
3. `backend/src/routers/mod.rs`：删除 `mod enterprise;`（第 17 行）+ 删除第 227–239 行整个 `/enterprise` 路由块（`.nest("/enterprise", ...)` 或等价装配，主控按实际结构定位该 9 route 块）。
4. `backend/src/services/mod.rs:18`：删除 `pub mod enterprise;`。
5. 核实无其他引用：`grep -rn "routers::enterprise\|services::enterprise" src/` 应仅剩上述将删点。

**编译风险（R4）**：中。需同时删「路由装配块 + 两文件 + 两处 `mod`」四处，漏一处即 error。删除后 `ulid`/`chrono` 等依赖仍被其他模块使用，不孤立。

**决策建议**：纳入本轮 P0（与交付物 1 同批），因其对外声明失真程度高于 `distributed/` stub；若因风险偏好单开工单，也应在 P0 收口前**至少从路由树摘除**（保留文件但去 route），避免继续对外暴露假控制面。

---

## 应用顺序与验证

建议主控按序应用并各自 `cargo check`：

1. **交付物 1**（删 4 文件 + 改 `distributed/mod.rs`）→ `SQLX_OFFLINE=true cargo check --offline`
2. **交付物 3a/3b/3c**（main.rs 停用 mutator + 3 处文档/allow）→ 同上
3. **交付物 2b**（CLAUDE.md，纯文档，无需编译）
4. **交付物 2a**（agent_card.rs）→ 仅记录，离线不可验；联网启用 a2a 时 `cargo check --features a2a`
5. **R4**（若 lead 批准）→ 删 2 文件 + `routers/mod.rs` 路由块 + 2 处 `mod` → 同上

**统一验证口径**：
```bash
cd /Users/fanjia/Desktop/code/Aetheris-MemOS/backend
SQLX_OFFLINE=true cargo check --offline      # 期望 0 error
```

---

## 需 lead 决策项汇总

| 编号 | 事项 | 本方案推荐 |
|---|---|---|
| **R1** | `lease_coordinator` 实测零消费（与「真在用」判断不符）| 按指令保留 + 加「暂未接线」文档标注；是否删除请 lead 定 |
| **R2** | `agent_card.rs` 改动离线不可编译验证（a2a feature 默认关）| 记录为正确改动，联网启用 a2a 时验证 |
| **R3** | `strategy_mutator` 停用 vs 删除 | 停用 + 标注（保留 P3 复用），不删 |
| **R4** | `enterprise.rs` 活假集群是否本轮删 | 纳入本轮 P0；至少 P0 收口前从路由树摘除 |

---

## 涉及文件清单（绝对路径）

**删除**：
- `/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/distributed/consensus.rs`
- `/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/distributed/replication.rs`
- `/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/distributed/sharding.rs`
- `/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/distributed/node.rs`
- （R4 待批）`/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/routers/enterprise.rs`
- （R4 待批）`/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/services/enterprise.rs`

**修改**：
- `/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/distributed/mod.rs`（全文替换）
- `/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/main.rs`（停用 mutator 守护，第 93–98 行）
- `/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/services/strategy_mutator.rs`（顶部文档横幅）
- `/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/protocol/grpc.rs`（顶部文档 + `#![allow(dead_code)]`）
- `/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/protocol/websocket.rs`（顶部文档 + `#![allow(dead_code)]`）
- `/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/a2a/agent_card.rs`（3 处，feature-gated）
- `/Users/fanjia/Desktop/code/Aetheris-MemOS/CLAUDE.md`（6 处，纯文档）
- （R4 待批）`/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/routers/mod.rs`（删 `mod enterprise;` + 第 227–239 行路由块）
- （R4 待批）`/Users/fanjia/Desktop/code/Aetheris-MemOS/backend/src/services/mod.rs`（删第 18 行 `pub mod enterprise;`）
