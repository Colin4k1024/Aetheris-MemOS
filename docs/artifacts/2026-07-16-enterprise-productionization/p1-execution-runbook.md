# P1 执行 Runbook — 依赖有序 + 逐 PR

- **日期**：2026-07-16 ｜ **主责**：tech-lead（排序）/ backend-engineer（执行）
- **用途**：把 P1 各计划/ADR 合并成**依赖有序、逐 PR 可执行**的落码指南。**在有真实基建的环境按序开工。**
- **依据**：`delivery-plan.md`、`p1-rls-isolation-plan.md`、`p1-outbox-and-governance-plan.md`、ADR-0001/0002/0003/0004/0006。
- **已完成的离线基石**：`backend/src/mcp/capability.rs`（Plane A 授权纯逻辑 + 12 单测，已合并）——PR-7 直接复用。

---

## 0. 结论先行

1. **第一块必是 `tenant_scope` 执行器**（PR-1）：RLS 计划已证——代码用全局静态池 + 直接 `pool()`、读路径零事务，RLS 依赖的事务局部 GUC 无处安放。不先做这个，M2 NOT NULL 迁移会直接打挂线上写。
2. **审计基建先于 outbox 与治理**（PR-2）：两者共用 `memory_audit_events` 写入。
3. **迁移必须 expand→backfill→enforce→contract 分期**，禁止一把梭；每个 enforce 迁移前先完成对应的应用层接线。
4. **凡改 `sqlx::query!` 宏的 SQL 字符串 → 需连真库 `cargo sqlx prepare` 重生成 `.sqlx`**；db 层多数是运行时 `sqlx::query`（不受影响），但迁移/新查询要留意。

---

## 1. 基建前置清单（开工前就绪）

| 依赖 | 用途 | 阻塞哪些 PR |
|---|---|---|
| PostgreSQL 14+（可写、可迁移） | RLS / advisory lock / 迁移 / 审计 | PR-1..7, 9 |
| Qdrant（gRPC 6334） | outbox 投递 / 对账 | PR-5 |
| Testcontainers（CI） | RLS/outbox 行为测试 gate（本地离线跳过） | PR-3,5 的验证 |
| 网络（github） | pin a2a 依赖（`--features a2a`） | P2 A2A |
| wasm 工具链（wasmtime target） | MCP Plane B 真沙箱 | PR-8 |
| （可选）etcd/Consul | enterprise 集群协调演进位（ADR-0006 起步用 PG，不必须） | P2 enterprise |

---

## 2. 依赖拓扑（P1）

```
PR-1 tenant_scope 执行器 ─┬─> PR-3 RLS 迁移+接线
                          ├─> PR-4 outbox 写路径(同事务)
                          └─> PR-6 治理中间件
PR-2 审计基建 ─┬─> PR-4 / PR-5 (outbox 审计)
               ├─> PR-6 治理审计
               └─> PR-7 MCP Plane A 审计
PR-4 ─> PR-5 outbox worker + 对账 (+ Qdrant scroll/get 方法)
PR-6 ─> PR-9 enterprise 许可分级(门控)
PR-7 (复用已合并 capability.rs) ; PR-8 Plane B 独立(需 wasm 链)
```

---

## 3. 逐 PR 拆解

| PR | 范围 | 涉及文件 | 需真 PG/Qdrant | 需 sqlx prepare | 验证 / 放行 |
|---|---|---|---|---|---|
| **PR-1** tenant_scope 执行器 | 建 per-request 事务/连接载体 + `SET LOCAL aetheris.tenant_id`；仓库签名收敛为 `(executor, TenantId)`；写路径双写物理 `tenant_id` 列 | 新 `db/tenant_scope.rs`；`db/{stm,ltm,kg,mm}.rs` 签名；中间件接入点 | 是（行为） | 否（运行时 query） | 单测 helper；PG 集成：GUC 在事务内可见、跨请求隔离 |
| **PR-2** 审计基建 | `insert_event`/`insert_tx` + 后台 mpsc writer（不阻塞请求） | 新 `db/audit.rs`、`services/audit_writer.rs`；`main.rs` 启动 | 是 | 视实现 | 单测 mpsc→mock 仓库；PG 集成写 `memory_audit_events` |
| **PR-3** RLS 迁移+接线 | M1 回填→M2 NOT NULL(NOT VALID→VALIDATE→SET)→M3 启用 RLS policy(`current_setting`)；读路径改依赖 RLS 兜底 | `migrations/*`；`db/*.rs` 查询点 | 是 | **是** | Testcontainers：跨租户被 DB 拒（渗透） |
| **PR-4** outbox 写路径 | `store_ltm` 改单事务(fact+version+outbox 事件同 tx)；`create_knowledge_entry_tx` 回填物理 tenant_id；响应 `indexStatus:pending` | `services/memory_storage.rs`、`db/ltm.rs`、新 `db/vector_outbox.rs` | 是 | 视实现 | 崩溃点故障注入：无部分写/无孤儿 |
| **PR-5** outbox worker + 对账 | worker(认领 FOR UPDATE SKIP LOCKED→投递→标记/退避/死信/reclaim_stale)；对账扫 4 类 drift | 新 `services/outbox_worker.rs`、`db/vector_reconciliation.rs`；`services/qdrant.rs` 加 `scroll_point_ids`/`get_point_payload`；`main.rs` | 是（PG+Qdrant） | 否 | 幂等重投无重复；对账修复 missing/orphan/mismatch |
| **PR-6** 治理中间件 | auth→rate_limit→**governance**→handler；classify()+pre_store/search/authorize/quota/audit；配额真 increment `used` | 新 `hoops/governance.rs`；`routers/mod.rs` 挂载；`hoops/enterprise*` 单例 | 是（审计/配额） | 否 | classify 纯函数单测；Deny→403 集成 |
| **PR-7** MCP Plane A | call_tool 强制验签(复用 signing.rs)+capability 授权(**复用 capability.rs**)+审计；灰度→enforce 两阶段 | `routers/mcp.rs`；(capability.rs 已合并) | 是（审计） | 否 | 越权/验签失败负向测试；审计字段非空 |
| **PR-8** MCP Plane B | `execute_wasm` mock→wasmtime 真实例化+capability host fn(deny-by-default)+fuel/epoch/StoreLimits | `mcp/sandbox.rs`、`sandbox_proxy.rs`；`#[cfg(feature="wasm-tools")]` | 否 | 否 | 沙箱逃逸/资源耗尽/capability 拒绝测试 |
| **PR-9** enterprise 许可分级 | `tenant_licenses` 表 + LicenseTier 门控接治理 hooks + 配额上限 | `migrations/*`、`hoops/enterprise.rs`、`services/enterprise.rs` | 是 | 视实现 | tier 门控 + 越限拒绝负向测试 |

> P2（后续）：enterprise 集群协调（PG advisory lock 选主 + 一致性哈希分片 + 心跳/re-balance 守护，ADR-0006）、多协议接真（gRPC/WS/A2A + 跨协议鉴权，ADR-0007）。P3：自适应核心（ADR-0008）。

---

## 4. 首批 3 个 PR 开工说明

### PR-1 `tenant_scope` 执行器（keystone）
- 目标：给每个请求一个"带租户 GUC 的事务/连接"载体，让 RLS 有处安放。
- 步骤：① 新 `db/tenant_scope.rs`：`async fn with_tenant_tx<F>(pool, tenant_id, f)`——`BEGIN` → `SELECT set_config('aetheris.tenant_id', $1, true)` → 执行 `f(&mut tx)` → `COMMIT`；② 读路径也开短事务（哪怕只读）；③ 仓库函数签名从 `pool()` 收敛为收 `executor`；④ 写路径同时写物理 `tenant_id` 列（为 PR-3 回填/对账铺路）。
- 验证：离线 `cargo check` + helper 单测；PG 环境验 GUC 事务内可见。

### PR-2 审计基建
- `db/audit.rs`（`insert_event`/`insert_tx`）+ `services/audit_writer.rs`（mpsc→批量落库，溢出丢弃+计数告警，不反压）；`main.rs` 启动 writer。
- 验证：mpsc→mock 仓库单测；PG 集成写 `memory_audit_events`。

### PR-3 RLS 迁移 + 接线
- 严格分期：M1 回填（`split_part` 从前缀提租户，不可归属→`'__unattributed__'`）→ M2 三段 NOT NULL → M3 `ENABLE ROW LEVEL SECURITY` + policy(`current_setting('aetheris.tenant_id', true)`)。
- 前缀过渡期保留（双读兜底），观察零违规后 M4 收缩。
- SQLite 无 RLS → 降级应用层 `WHERE tenant_id=$1`。
- 验证：**必须** Testcontainers 真 PG 渗透测试（A 租户连接查不到 B 数据）。

---

## 5. 放行门禁（每 PR）
- 开发完成：合并 + `SQLX_OFFLINE=true cargo check --offline` 0 error + clippy + 单测。
- 测试完成：PG/Qdrant 相关 PR 过 Testcontainers CI gate；负向/故障注入测试有证据。
- 阶段收口：P1 全绿 = 跨租户 DB 层强制拒 + 无部分写/孤儿 + 备份恢复演练(ADR-0003) + MCP 越权拒 + 审计非空。
