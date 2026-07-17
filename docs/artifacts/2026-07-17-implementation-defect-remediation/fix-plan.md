# 实现层缺陷审查与修复计划

- **日期**：2026-07-17
- **分支快照**：`feat/p1-governance-middleware`（含 PR-1～PR-6：tenant_scope、审计 writer、四层 RLS、治理 middleware）
- **主责角色**：tech-lead（收口）/ backend-engineer（实现）/ architect（跨模块契约）/ qa（放行）
- **关联入口**：
  - 企业生产化总计划：[`../2026-07-16-enterprise-productionization/delivery-plan.md`](../2026-07-16-enterprise-productionization/delivery-plan.md)
  - P1 outbox + 治理：[`../2026-07-16-enterprise-productionization/p1-outbox-and-governance-plan.md`](../2026-07-16-enterprise-productionization/p1-outbox-and-governance-plan.md)
  - P1 RLS：[`../2026-07-16-enterprise-productionization/p1-rls-isolation-plan.md`](../2026-07-16-enterprise-productionization/p1-rls-isolation-plan.md)
  - 服务化 remediation 日志：[`../2026-07-16-backend-serviceization-remediation/execute-log.md`](../2026-07-16-backend-serviceization-remediation/execute-log.md)
- **关联 ADR**：
  - [ADR-0001](../../adr/ADR-0001-memory-storage-tenant-isolation.md) 租户隔离
  - [ADR-0002](../../adr/ADR-0002-memory-vector-outbox-reconciliation.md) 向量 outbox 与对账
  - [ADR-0003](../../adr/ADR-0003-memory-storage-operational-readiness.md) 运维就绪
  - [ADR-0004](../../adr/ADR-0004-mcp-sandbox-execution-model.md) MCP 沙箱双平面
  - [ADR-0005](../../adr/ADR-0005-ha-infrastructure-selection.md) HA 基建
  - [ADR-0007](../../adr/ADR-0007-multi-protocol-transport-auth.md) 多协议传输
  - [ADR-0008](../../adr/ADR-0008-adaptive-learning-and-eval.md) 自适应学习与 eval

---

## 1. 结论先行

1. 当前实现定位是 **「记忆平台 + 租户地基脚手架」**，不是完整企业运行时。
2. P1 已交付：四层记忆表 RLS 迁移 + `begin_tenant_tx`、审计异步 writer、治理 middleware 挂链。
3. P1 **仍未闭合的准入阻塞项**：
   - LTM 仍「先 Qdrant 后 DB」同步双写（outbox 表零引用）
   - MCP `call_tool` 无验签 / 无 capability / 沙箱 mock
   - 治理策略空转（配额不计数、RBAC 不参与 pre-hook、fail-open）
   - 默认部署角色可能使 RLS 成为 NO-OP
4. 本计划在既有 delivery-plan 之上，按 **实现层缺陷** 给出可执行修复顺序、验收标准与代码落点；不重复写已有 ADR 正文，只引用并标注差距。

---

## 2. 现状矩阵（宣称 vs 实现）

| 领域 | 已落地 | 仍是缺陷 / 半成品 |
|------|--------|-------------------|
| 租户 RLS | LTM/STM/KG/MM 四层 schema RLS + `tenant_scope` | 默认 DB 角色 superuser → RLS NO-OP；旁路表无 RLS |
| 治理 hooks | `governance_middleware` + `audit_writer` 已挂 | 配额不计数、RBAC 不参与决策、fail-open、覆盖缺口 |
| LTM↔Qdrant | 租户 filter / payload；写路径补偿删除 | **仍同步双写**；outbox / reconciliation 表零引用 |
| MCP | list 验签、tenant 透传、Plane A 纯逻辑 | call_tool 不验签、无 capability、Wasm 沙箱 mock |
| 自适应调度 | 启发式固定系数 | 无学习/拟合；predictor 写死常量 |
| 多协议 | HTTP 为主 | gRPC/WS 类型壳；A2A 假数据壳 |
| 文档 | CLAUDE 部分诚实标注 | `IMPLEMENTATION_STATUS.md` 仍写 “COMPLETED v1.0” |

---

## 3. 缺陷清单（按严重级）

### 3.1 P0 — 安全与一致性（企业红线）

| ID | 缺陷 | 证据位置 | 影响 |
|----|------|----------|------|
| D-01 | LTM 写路径先 Qdrant 后 DB；崩溃可出孤儿向量 | `backend/src/services/memory_storage.rs` `store_ltm_for_tenant` | 跨存储不一致，无法对账自愈 |
| D-02 | MCP `call_tool` 不验签、不走 `capability::authorize` | `backend/src/routers/mcp.rs`；`backend/src/mcp/capability.rs` 仅单测 | 租户内越权写/删；契约完整性未强制 |
| D-03 | Wasm 沙箱 mock 且请求链零调用 | `backend/src/mcp/sandbox.rs` `execute_wasm` → `Ok(input)` | 对外「沙箱隔离」名实不符 |
| D-04 | 默认部署角色可能 BYPASSRLS / superuser | RLS 迁移注释；stock `memory` 角色 | schema 隔离在 dev 默认 NO-OP |
| D-05 | 治理已挂链但策略空转 | `hoops/governance.rs` + `enterprise_impl.rs` + `tenant/quota.rs` | 配额永不生效；RBAC 不挡请求 |
| D-06 | 限流 key 取客户端可控 `x-forwarded-for` | `backend/src/hoops/rate_limit.rs` | 可轮换绕过限流 |

### 3.2 P1 — 可靠性与治理半成品

| ID | 缺陷 | 说明 |
|----|------|------|
| D-07 | Outbox / 对账表已建、src 零引用 | 缺 `db/vector_outbox.rs`、`outbox_worker`、reconciliation service |
| D-08 | 配额 `used` 从不 increment；未设配额恒 true | 即使 middleware 调用 pre_store 也拦不住 |
| D-09 | RBAC 内存 HashMap；`pre_store`/`pre_search` 只查配额 | 角色权限对写/搜无约束 |
| D-10 | RbacService 双单例 | `routers/tenant.rs` 与 `create_enterprise_hook_set()` 各 new 一份 |
| D-11 | 治理 fail-open | hooks 未初始化 / 无 tenant / 路径未 classify → 放行 |
| D-12 | classify 覆盖缺口 | `forget`、`/storage/sessions`、adaptive 等不经治理；MCP 独立路由无 governance |
| D-13 | 审计双轨 | 持久化 `db/audit` + 内存 `AuditHookImpl` v2；模型不统一 |
| D-14 | 非记忆表无 RLS | decision_trace / weights / performance / configs / agent / evidence 等 |
| D-15 | 完整性扫描偏默认租户 | information_guard 多租户覆盖不足 |

### 3.3 P2 — 协议与集成壳

| ID | 缺陷 | 说明 |
|----|------|------|
| D-16 | gRPC / WebSocket 类型壳 | `protocol/grpc.rs`、`protocol/websocket.rs` 无 server 接线 |
| D-17 | A2A feature 默认关 + handler 壳 | 假/简化响应；无真 task store / 真流式 |
| D-18 | Letta provider stub | `providers/letta.rs` 一律 not implemented |
| D-19 | Neo4j 索引未真初始化 | `init_neo4j_indexes` 仅 warn |

### 3.4 P3 — 自适应名实不符

| ID | 缺陷 | 说明 |
|----|------|------|
| D-20 | Predictor 写死系数 | `services/predictor.rs` 固定 baseline |
| D-21 | Scheduler 启发式选择 | 无遥测拟合、无在线更新 |
| D-22 | Planner sandbox 为 mock 输出 | dry-run 演示，非真实隔离 |

### 3.5 架构债 / 文档债

| ID | 缺陷 | 说明 |
|----|------|------|
| D-23 | Evidence Graph 链路不完整 | 有表/hash 字段，缺端到端防篡改与运维闭环 |
| D-24 | 双企业 Hook 体系 | enterprise / enterprise_impl vs enterprise_hooks_v2 |
| D-25 | 文档失真 | `IMPLEMENTATION_STATUS.md` 宣称 COMPLETED；部分旧 docs 过度承诺 |
| D-26 | 双时序 KG 不完整 | relation 表尚无 temporal 列 |
| D-27 | 前端/SDK 写后读契约 | 未来 outbox 化会破坏「写后立即向量可搜」假设 |

---

## 4. 修复计划（分波次）

### Wave 0 — 立刻（阻塞企业准入）

**目标**：闭合最危险的一致性与执行授权缺口。

| 序 | 工作项 | 关闭缺陷 | 主要落点 | 验收标准 |
|----|--------|----------|----------|----------|
| W0.1 | LTM 写改 **DB 单事务 + outbox 事件**，Qdrant 移出热路径 | D-01, D-07 | `memory_storage.rs`；新增 `db/vector_outbox.rs`、`services/outbox_worker.rs`；对齐 ADR-0002 | 崩溃后无孤儿向量；重启 worker 可重放至 applied；响应可带 `indexStatus: pending` |
| W0.2 | MCP `call_tool`：**验签 + `capability::authorize` + 结构化审计** | D-02 | `routers/mcp.rs`、`mcp/capability.rs`、`mcp/signing.rs`、audit_writer | 未签/能力不足 → 明确拒绝；有审计事件；单测 + 集成测 |
| W0.3 | 部署默认改为 **非 superuser / 非 BYPASSRLS** 应用角色 | D-04 | docker-compose、migration 文档、`deployment-context`、渗透测 | 受限角色下跨租户 0 行；superuser 仅运维 |
| W0.4 | 配额真生效 + RBAC 并入 pre-hook（或独立 middleware） | D-05, D-08, D-09, D-10 | `tenant/quota.rs`、`enterprise_impl.rs`、`governance.rs`、统一 RbacService 单例 | 设限额后超限 403；角色无 Write 拒 store；API 赋角色与 hook 同一状态 |
| W0.5 | 限流改受信客户端标识 | D-06 | `hoops/rate_limit.rs`、`main.rs` ConnectInfo | 伪造 XFF 不能无限轮换；文档说明代理信任链 |

**建议并行度**：W0.1 ∥ W0.2；W0.3 与 devops 并行；W0.4 依赖 hooks 已挂（已完成）；W0.5 独立。

**参考实现细节（W0.1）**：见 [`p1-outbox-and-governance-plan.md`](../2026-07-16-enterprise-productionization/p1-outbox-and-governance-plan.md) 子项 (a)。

目标写路径：

```text
1. LLM summarize / embedding     （事务外）
2. BEGIN TX:
     create_knowledge_entry_tx + 物理 tenant_id
     vector_outbox.insert_event_tx (upsert, idempotency_key)
     optional: audit insert_tx
   COMMIT
3. 返回 { entry_id, indexStatus: "pending" }
4. worker 认领 outbox → 幂等 upsert Qdrant → mark applied
5. 周期 reconciliation：missing / orphan / tenant_mismatch / content_hash_mismatch
```

**参考实现细节（W0.2）**：见 [ADR-0004](../../adr/ADR-0004-mcp-sandbox-execution-model.md) Plane A。

```text
call_tool:
  authn (已有) → tenant context (已有)
  → verify tool/call signature (强制)
  → capability::authorize(granted, tool_name)
  → 原生第一方工具执行（不套 WASM）
  → record_audit (success/denied/failure)
```

---

### Wave 1 — P1 收口

**目标**：治理可观测、策略可配置、一致性可自愈。

| 序 | 工作项 | 关闭缺陷 | 验收标准 |
|----|--------|----------|----------|
| W1.1 | outbox worker + reconciliation dry_run/repair | D-07 | 故障注入：DB 已提交 worker 未消费 → 重放；投递中途崩 → reclaim_stale + 幂等 |
| W1.2 | 治理覆盖 forget / MCP / 关键写路径 | D-11, D-12 | classify 表扩展；MCP 挂 governance 或等价 pre-check；误配可 fail-closed（配置开关） |
| W1.3 | 审计单真相源 | D-13 | 成功/拒绝均落 `memory_audit_events`；废弃或明确 v2 内存 audit 仅 dev |
| W1.4 | 旁路表 tenant 策略（按风险排序） | D-14 | decision_trace / feedback 至少应用层强制 + 可选 RLS；inventory 更新 |
| W1.5 | information_guard 多租户扫描 | D-15 | 滚动覆盖全部活跃 tenant |
| W1.6 | 文档诚实化 | D-25 | 修正 `IMPLEMENTATION_STATUS.md`；README/CLAUDE 与代码一致；agent-card 去未实现能力 |
| W1.7 | DB 备份/PITR/告警/恢复演练 | ADR-0003 | 有 runbook + 一次成功恢复演练记录 |

**P1 放行门禁（汇总）**：

- [ ] 跨租户隔离：受限角色渗透测通过（四层记忆表）
- [ ] 崩溃无部分写：outbox 重放 + 对账可修 orphan/missing
- [ ] MCP：未授权 call 被拒；有审计
- [ ] 配额/RBAC：默认策略可演示「拦得住」
- [ ] 对外文档无「声明 > 实现」
- [ ] `cargo test` / 关键集成测（含 `#[ignore]` PG 测在 CI gate）全绿

---

### Wave 2 — 多协议与沙箱 Plane B

**目标**：协议面产生真实记忆操作；不可信工具有真隔离。

| 序 | 工作项 | 关闭缺陷 | 验收标准 |
|----|--------|----------|----------|
| W2.1 | A2A handler 接真实记忆服务 + task store + 真流式 | D-17 | e2e 产生真实 STM/LTM 操作；feature pin 依赖可离线 build |
| W2.2 | gRPC tonic server + 鉴权 **或** 删除死代码 | D-16 | 若实现：契约测；若不实现：模块删除/移出 lib 导出，文档标注 |
| W2.3 | WebSocket 真 server + 鉴权 **或** 删除死代码 | D-16 | 同上 |
| W2.4 | MCP Plane B：wasmtime 真执行 + fuel/epoch/StoreLimits | D-03 | 不可信工具在沙箱内；能力拒绝可测；无 ambient authority |
| W2.5 | Letta provider：实现或从默认路径移除 | D-18 | 无假健康/假可用声明 |
| W2.6 | Neo4j 索引与失败语义诚实 | D-19 | 初始化成功/失败日志与 readiness 一致 |

依赖：Wave 0/1 核心稳定后再开协议，避免在未隔离地基上放大攻击面。

---

### Wave 3 — 自适应核心（或诚实降级）

**目标**：要么 eval 证明优于静态配置，要么对外降级文案。

| 序 | 工作项 | 关闭缺陷 | 验收标准 |
|----|--------|----------|----------|
| W3.1 | 遥测 → 特征管线（复用 OTel） | D-20, D-21 | 性能样本可查询、按 tenant 隔离 |
| W3.2 | predictor 从实测拟合 | D-20 | 系数非写死；有置信度定义 |
| W3.3 | scheduler 使用真预测 + 有界在线更新 | D-21 | 替换/隔离假 mutator 路径 |
| W3.4 | eval harness | ADR-0008 | 离线基准：自适应 **显著优于** 静态最优，否则产品文案降级为 heuristic |
| W3.5 | Evidence Graph 端到端闭环（可选并行） | D-23 | hash 链可验证 + 工作流 evidence API 文档与测试一致 |
| W3.6 | 双时序 KG relation 列补齐 | D-26 | history/at-time 对 relation 语义完整 |

**降级出口**：若 W3.4 不达标，禁止继续宣传「自进化/学习型自适应」；保留「启发式记忆配置选择」表述。

---

## 5. 横切工作（所有 Wave）

| 主题 | 动作 |
|------|------|
| 单测策略 | 纯函数离线（幂等键、backoff、classify、authorize）；PG/Qdrant 集成测 `#[ignore]` 或 Testcontainers CI gate |
| 前端/SDK | W0.1 后：文档与类型增加 `indexStatus`；勿假设写后立即可向量检索（D-27） |
| Hook 体系 | 收敛 `enterprise` vs `enterprise_hooks_v2`（D-24）：一条请求链只认一套 |
| 配置 | governance fail-open/fail-closed、默认配额、默认角色、MCP key bundle 均配置化并有安全默认值 |
| 可观测 | outbox lag、dead-letter 数、quota deny 率、MCP authz deny 率进 metrics/OTel |

---

## 6. 建议排期（1–2 后端粗估）

| 波次 | 粗估 | 依赖 |
|------|------|------|
| Wave 0 | 2–4 周 | 真 PG + Qdrant 环境（outbox/RLS 验收） |
| Wave 1 | 3–5 周 | Wave 0 核心合并后 |
| Wave 2 | 4–8 周 | Wave 1 放行后 |
| Wave 3 | 2–4 月（含数据积累） | 遥测可并行从 Wave 1 起埋点 |

与 [`delivery-plan.md`](../2026-07-16-enterprise-productionization/delivery-plan.md) 的 P1→P2→P3 对齐：**不要抢跑协议/自适应**。

---

## 7. 推荐开工顺序（最短路径）

若只能串行推进，推荐：

1. **W0.1 Outbox 改 LTM 写**（一致性）
2. **W0.2 MCP call_tool 验签 + capability**（执行安全）
3. **W0.4 配额 increment + RBAC 进 pre-hook + 单例统一**（治理真生效）
4. **W0.3 部署角色 hardening + 渗透测固化**
5. **W1.1 对账 worker**
6. 文档诚实化（W1.6）与限流 XFF（W0.5）穿插

---

## 8. 已具备、无需推倒重来的资产

- 四层记忆表 RLS 迁移与 `begin_tenant_tx` GUC 接线
- 搜索侧 `search_for_tenant` / Qdrant `tenantId` filter
- JWT 弱密钥 fail-fast、登录限流
- `governance_middleware` + `audit_writer` 骨架
- Plane A `capability::authorize` 纯逻辑与单测
- `distributed/` 已收敛为进程内协调（非假集群）

修复原则：**接线与补真，不重复造框架**。

---

## 9. 文档与代码同步清单

修复过程中同步更新：

| 文件 | 动作 |
|------|------|
| `IMPLEMENTATION_STATUS.md` | ✅ 已改为分能力状态表，去掉 COMPLETED 笼统声明 (2026-07-17) |
| `Claude.md` / `AGENTS.md` | ✅ AGENTS.md 已更新（新增 governance/audit/outbox/tenant_scope 模块）；无 Claude.md |
| `docs/API_USAGE_GUIDE*.md` | ✅ 已同步 (2026-07-17)：LTM 响应增加 `indexStatus` 字段 + 说明；18.1 治理 403 错误；18.2 MCP 错误码 |
| `docs/artifacts/.../deployment-context.md` | ✅ 已同步 (2026-07-17)：新增非 BYPASSRLS 应用角色说明 + 备份恢复演练清单；2026-07-06 版本更新阻塞项状态 |
| 本文件 | 每完成一波次勾选 §4 验收，并记 execute-log |

---

## 10. 变更日志

| 日期 | 变更 |
|------|------|
| 2026-07-17 | 初版：基于 `feat/p1-governance-middleware` 实现层审查，输出 Wave 0–3 修复计划 |
| 2026-07-17 | 文档同步完成：IMPLEMENTATION_STATUS 诚实化、AGENTS 更新、API_USAGE_GUIDE 增加 outbox/治理/MCP 错误码、deployment-context 增加 RLS 角色和备份演练 |

---

_本文为 living document。实现完成后请在同目录追加 `execute-log.md` 记录提交、测试证据与放行结论。_
