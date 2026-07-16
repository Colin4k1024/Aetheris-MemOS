# Team Execution Plan: Memory Storage Reliability

## 结论

Memory Storage Reliability 企业高可靠整改团队已组建完成，核心执行小队为：

- `tech-lead`：统一 intake、优先级裁决、阻塞升级、最终 gate。
- `architect`：架构执行设计、迁移边界、outbox/reconciliation contract、design review 收口。
- `backend-engineer`：tenant_id schema/RLS、TenantId 强制传递、事务化、outbox/reconciliation 实现。
- `qa-engineer`：RED tests、tenant negative tests、transaction fault injection、outbox/reconciliation E2E、放行门禁。
- `devops-engineer`：deployment context、backup/restore drill、alert rules、release/rollback readiness。

当前阶段：`design-review`

目标阶段：`handoff-ready`

当前就绪状态：`not-ready / blocked for implementation and operational evidence`

已确认输入：不涉及企业 T1/T2/T3/T4 应用等级；默认支持 50 并发；PostgreSQL 采用自建方案；Qdrant 采用 cluster 方向；Neo4j 为生产必选；告警暂时不考虑；历史无法归属数据进入只读隔离；outbox eventual consistency 可接受；P0 支持 read-after-write fallback。

剩余阻塞原因：P0 代码、migration、runbook、restore/rebuild/rollback drill 证据尚未完成；企业审计日志保留周期和敏感字段脱敏规则仍待确认。

## 输入依据

- `docs/artifacts/2026-07-06-memory-storage-reliability/prd.md`
- `docs/artifacts/2026-07-06-memory-storage-reliability/delivery-plan.md`
- `docs/artifacts/2026-07-06-memory-storage-reliability/test-plan.md`
- `docs/architecture/memory-storage-reliability.md`
- `docs/adr/ADR-0001-memory-storage-tenant-isolation.md`
- `docs/adr/ADR-0002-memory-vector-outbox-reconciliation.md`
- `docs/adr/ADR-0003-memory-storage-operational-readiness.md`

- [P0-S2 Tenant Production Path Inventory](tenant-production-path-inventory.md)
- [P0-S3 RLS Context Strategy](rls-context-strategy.md)
- [P0-S4 Transaction Boundaries](transaction-boundaries.md)

## 团队分工

| 角色 | 主责 | 第一批任务 | 主要交付物 |
|------|------|------------|------------|
| tech-lead | 目标锁定、冲突裁决、handoff-ready gate | 50 并发基线、read-after-write fallback、剩余审计规则确认；Requirement Challenge 收口 | 分派结论、阻塞升级、handoff-ready 判定 |
| architect | 架构方案收口 | TenantId/RLS/outbox/reconciliation/operational gate 设计评审 | architecture execution design、状态机、迁移边界、失败模式 |
| backend-engineer | 存储可靠性实现 | tenant_id migration、RLS context、TenantId 改造、事务化、outbox/reconciliation | migration、repository/service/router 改造、自测报告 |
| qa-engineer | 测试与放行建议 | RED tests、负向租户测试、事务故障注入、outbox/reconciliation E2E、operational evidence 审核 | 测试执行证据、阻塞项、放行建议 |
| devops-engineer | 运行保障与发布准备 | deployment-context、release-plan、launch-acceptance、backup/restore drill；告警暂不纳入当前门禁 | 运维 runbook、rollback/forward-fix 策略 |
| security reviewer（gate） | 安全专项复核 | RLS、跨租户拒绝、审计脱敏 | Critical/High 安全审查结论 |
| data/reliability reviewer（可选 gate） | 数据迁移复核 | backfill/quarantine/锁表风险 | 数据归属报告与迁移风险意见 |

## Requirement Challenge 记录

### 质疑 1：是否已具备进入 P0 实施的业务等级输入？

- Evidence：用户已确认不涉及企业 T1/T2/T3/T4 应用等级，默认支持 50 并发，PostgreSQL 自建，Qdrant cluster，Neo4j 生产必选，告警暂不考虑。
- Reasoning：这些输入足以启动 P0 foundation 与 build stories；但自建 PostgreSQL、Qdrant cluster、Neo4j 必选会提高 deployment-context 与 release-plan 的设计要求。
- Implications：应用等级和告警 owner 不再阻塞 handoff-ready；容量验证、HA 拓扑草案、restore/rebuild/rollback drill 仍需完成。
- 结论：整改方向可以进入 P0 执行准备；剩余阻塞转为实现与运维证据。

### 质疑 2：P0 范围是否过大？

- Evidence：P0 横跨 schema、repository、事务、outbox、reconciliation、audit、backup、alert、release gate。
- Reasoning：若作为一个大包推进，会导致 QA/DevOps 末期集中阻塞。
- Implications：必须拆成 story-sized execution units。
- 结论：不缩小 P0 范围，但分 foundation stories 与 build stories 执行。

### 质疑 3：outbox eventual consistency 是否改变搜索语义？

- Evidence：ADR-0002 明确 LTM/Qdrant 采用 eventual consistency。
- Reasoning：用户可能默认写后立即可向量搜索；outbox 后会短暂延迟。
- Implications：接口需暴露 `index_status=pending` / correlation id，并提供按 ID 回源读取兜底。
- 结论：接受 outbox 模型；P0 支持 read-after-write fallback，即事实写入后允许按 ID 回源读取，向量搜索可保持 `index_status=pending` 的 eventual consistency。

### 质疑 4：Operational readiness 能否只靠文档声明托管服务？

- Evidence：用户已确认 PostgreSQL 采用自建方案、Qdrant 采用 cluster 方向、Neo4j 为生产必选；因此不能依赖托管服务声明代替运维设计。
- Reasoning：自建与生产必选组件需要明确部署拓扑、恢复路径、容量基线和 rollback / forward-fix；告警暂不考虑，但不可把缺少告警误写为已具备完整 operational readiness。
- Implications：DevOps 必须补 deployment-context、release-plan、launch-acceptance、PostgreSQL restore、Qdrant cluster/rebuild、Neo4j restore；告警相关任务降级为后续项，不作为当前 handoff-ready 阻塞。
- 结论：不能以文档声明替代恢复演练；告警暂不纳入本阶段门禁。

## 第一批 Foundation Stories

### P0-S1 tenant_id expand migration plan

- Owner：backend-engineer
- Support：architect、qa-engineer
- 内容：梳理 STM/LTM/KG/MM/history/relation/outbox/audit 表的 `tenant_id` 增量方案、索引、约束和 backfill 输入来源。
- 验收：migration proposal + backfill/quarantine plan 通过 architect review。

### P0-S2 TenantId production-path inventory

- Owner：backend-engineer
- Support：qa-engineer
- 内容：盘点 update/delete/history/time-travel/relation mutation 中缺失 TenantId 的方法签名和调用链。
- 验收：改造清单覆盖 `backend/src/db`、`backend/src/services`、memory routers，QA 可据此设计 negative tests。

### P0-S3 RLS context strategy spike

- Owner：backend-engineer + architect
- Support：security reviewer
- 内容：确定 transaction-local tenant context、连接池复用安全策略、缺失 context 默认拒绝模型。
- 验收：设计说明通过 review，包含误拒绝/污染回退方案。

### P0-S4 multi-step write transaction boundaries

- Owner：backend-engineer
- Support：qa-engineer
- 内容：为 STM add/delete、KG relation、LTM supersede/history/update 划定事务边界和故障注入点。
- 验收：每条路径明确事务入口、rollback 条件、测试注入方式。

### P0-S5 outbox state machine contract

- Owner：architect + backend-engineer
- Support：devops-engineer、qa-engineer
- 内容：定义 outbox schema、状态流转、幂等 key、retry/dead-letter、Qdrant payload 校验字段。
- 验收：ADR-0002 落成可实现 contract，DevOps/QA 能据此设计 metrics 与 tests。

### P0-S6 operational evidence intake

- Owner：devops-engineer + tech-lead
- Support：qa-engineer
- 内容：确认目标环境、备份能力、默认 50 并发容量验证入口、PostgreSQL 自建 HA 方案、Qdrant cluster 方案、Neo4j 生产必选恢复证据来源；告警暂不纳入当前门禁。
- 验收：形成 deployment context 输入清单；无法确认项升级 tech-lead。

### P0-S7 QA P0 negative/fault matrix refinement

- Owner：qa-engineer
- Support：backend-engineer、devops-engineer
- 内容：把 test-plan 矩阵转为可执行用例批次，先覆盖 tenant negative、RLS missing context、事务故障注入、outbox retry。
- 验收：每个 P0 story 有对应验证点、前置数据和阻塞判定。

## 第二批 Build Stories

1. `tenant_id` schema expand + dry-run。
2. historical backfill report + quarantine。
3. TenantId 强制传递代码改造。
4. RLS/database policy 实现与 negative tests。
5. STM/KG/LTM 事务化实现与 fault injection tests。
6. LTM fact + outbox transaction 改造。
7. outbox worker + idempotent Qdrant upsert/delete。
8. search 回源校验 + reconciliation dry-run / repair。
9. audit event coverage。
10. restore/rebuild/rollback drills、launch acceptance evidence。

## P1 延后项

P1 在 P0 安全边界、事实源和恢复闭环稳定后推进：

- 批量 LTM 部分成功契约。
- outbox/reconciliation dashboard。
- migration 性能与锁影响评估。
- 租户隔离 fuzz/property tests。

## 架构执行顺序

1. 统一 TenantId 契约与生产路径清单。
2. `tenant_id` expand migration。
3. backfill report + quarantine。
4. repository TenantId 强制传递。
5. STM/KG/LTM 关键事务化。
6. RLS context 与 policy。
7. LTM outbox schema + write path。
8. outbox worker + Qdrant idempotency。
9. search 回源校验 + reconciliation。
10. operational readiness evidence。

## Backend 首批改动范围

代表性文件：

- `backend/migrations/`：新增 expand migration，不修改历史 migration。
- `backend/src/tenant/context.rs`
- `backend/src/tenant/isolation.rs`
- `backend/src/db/mod.rs`
- `backend/src/db/stm.rs`
- `backend/src/db/ltm.rs`
- `backend/src/db/kg.rs`
- `backend/src/db/mm.rs`
- `backend/src/services/memory_storage.rs`
- `backend/src/services/memory_search.rs`
- `backend/src/services/memory_transfer.rs`
- `backend/src/services/qdrant.rs`
- 新增建议：`backend/src/db/outbox.rs`
- 新增建议：`backend/src/services/vector_outbox.rs`
- 新增建议：`backend/src/services/reconciliation.rs`
- `backend/src/routers/memory_storage.rs`
- `backend/src/routers/memory_search.rs`
- `backend/src/routers/knowledge_graph.rs`
- `backend/src/routers/multimodal.rs`
- `backend/tests/tenant_isolation.rs`
- 新增建议：`backend/tests/memory_storage_integrity.rs`
- 新增建议：`backend/tests/memory_reliability_e2e.rs`

## QA 第一批 RED Tests

1. memory-owned tables 必须有 `tenant_id` 或受控迁移桥接。
2. RLS 缺失 tenant context 时拒绝 read/write。
3. tenant A 不能 update tenant B LTM entry。
4. tenant A 不能 read tenant B LTM history。
5. tenant A 不能 create relation to tenant B KG entity。
6. STM add_message 第二步失败时必须回滚。
7. KG create_relation relation_count 更新失败时必须回滚。
8. LTM 写入必须先 DB fact + outbox，而不是 Qdrant-first。

## DevOps 第一批任务

1. 编写 `deployment-context.md`，标注当前 `docker-compose.yml` 仅 local/dev，不是生产 HA 证据，并记录 PostgreSQL 自建、Qdrant cluster、Neo4j 生产必选和 50 并发基线。
2. 编写 `release-plan.md`，采用 expand-contract 分阶段发布。
3. 编写 `launch-acceptance.md` 模板，当前结论设为 blocked，直到 P0 代码和恢复/回滚演练完成。
4. 制定 PostgreSQL 自建 HA 与 PITR/restore drill runbook。
5. 制定 Qdrant cluster 运行方案、snapshot restore 与 DB+outbox rebuild 双路径 runbook。
6. 制定 Neo4j 生产必选的 cluster 或等价高可用方案，并补 dump/restore drill。
7. 升级 CI gate，移除关键 release check 的 `continue-on-error`。
8. 增加生产配置 gate：禁止 `jwt.disabled=true`、禁止默认 secret、要求 Postgres backend 和 TLS/auth 决策。
9. 建立 release evidence 归档格式。
10. 告警规则与 on-call owner 暂不纳入当前门禁，后续进入正式生产发布时再评估。

## Handoff-ready 前置条件

进入 `handoff-ready` 前必须满足：

1. 企业应用等级不适用；本轮不挂 T1/T2/T3/T4 定级门禁。
2. 容量与性能基线为默认支持 50 并发。
3. PostgreSQL 采用自建方案，需补自建 HA / restore 设计。
4. Qdrant 采用 cluster 方向，同时保留 DB + outbox/reconciliation rebuild。
5. 告警接收 owner、pager routing、升级路径暂不作为本阶段门禁。
6. tenant_id migration/backfill/RLS 方案通过 design review。
7. outbox schema、状态机、worker、reconciliation 策略通过 design review。
8. 事务边界和故障注入方案通过 backend/QA review。
9. read-after-write fallback 纳入 P0；eventual consistency 语义被 architect 明确。
10. P0 stories 已拆到可在单个 PR 或小批 PR 内完成。
11. `test-plan.md` P0 矩阵映射到具体 story。
12. 正式发布前 artifact 路径确认：`deployment-context.md`、`release-plan.md`、`launch-acceptance.md`、发布后 `closeout-summary.md`。
13. 企业审计日志保留周期和敏感字段脱敏规则仍需确认。

## 需要 tech-lead 仲裁 / 确认的问题

| 企业应用等级 | 不适用 | 不挂 T1/T2/T3/T4 定级门禁，按项目内部可靠性整改推进 |
| 容量与性能基线 | 默认支持 50 并发 | backend/QA/DevOps 需在后续验证中覆盖 50 并发下的写入、搜索、outbox lag 和恢复影响 |
| PostgreSQL 生产方案 | 自建 | DevOps 需补自建 HA / PITR / restore 方案；不使用托管服务作为证据 |
| Qdrant 生产方案 | cluster | DevOps 需补 cluster 运行方案，同时保留 DB + outbox/reconciliation rebuild |
| Neo4j 是否生产必选 | 是 | 必须纳入 cluster 或等价高可用设计与 dump/restore drill |
| 告警 owner / pager routing | 暂不考虑 | 不作为当前 handoff-ready 门禁；后续正式生产发布前需重新评估 |
| 历史无法归属 tenant 数据 | 只读隔离 | 不猜测归属，不默认删除；RLS enforce 前必须有隔离报告 |
| outbox 写后搜索延迟 | 可接受 | API 暴露 pending/correlation id；向量搜索允许 eventual consistency |
| read-after-write fallback | 支持 | P0 支持按 ID 回源读取兜底 |

## 风险

- RLS 与 SQLx 连接池结合是高风险点，必须使用 transaction-local `SET LOCAL` / `set_config(..., true)`。
- backfill 不能猜测历史数据归属，无法归属数据必须 quarantine 或只读隔离。
- outbox 引入 eventual consistency，需要 API 契约、指标和用户可见状态说明。
- reconciliation repair 默认必须 dry-run，避免误删有效向量。
- local docker-compose、基础 healthcheck 和 SelfHealingService 模拟恢复不能作为生产 HA / 恢复证据。
- CI 当前部分关键检查 `continue-on-error: true`，不满足企业 release gate。

## 当前阻塞项

- 应用等级不适用，已关闭。
- 50 并发容量基线已确认，需补验证。
- PostgreSQL 自建、Qdrant cluster、Neo4j 生产必选已确认，需补 deployment/release 设计和演练证据。
- 告警 owner / pager routing 暂不作为本阶段门禁。
- 历史数据只读隔离策略已确认，需补隔离报告。
- P0 代码、migration、runbook、演练证据未完成。
- `deployment-context.md`、`release-plan.md`、`launch-acceptance.md` 尚未补齐。
- 企业审计日志保留周期和敏感字段脱敏规则仍待确认。

## 下一步

1. architect 主持 design review，锁定 TenantId/RLS/outbox/reconciliation contract，并纳入 50 并发与 read-after-write fallback 语义。
2. backend-engineer 启动 P0-S1~P0-S5 的设计和 RED tests 准备。
3. qa-engineer 将 RED tests 转成测试文件计划和阻塞 gate，补 50 并发验证入口。
4. devops-engineer 补 `deployment-context.md`、`release-plan.md`、`launch-acceptance.md` 草案和 PostgreSQL 自建 / Qdrant cluster / Neo4j 必选的 drill runbook。
5. tech-lead 继续确认企业审计日志保留周期和敏感字段脱敏规则。

## Handoff 摘要

- 背景：memory storage 企业高可靠整改已完成 PRD、Delivery Plan、Test Plan、Architecture、ADR 输入，需要组建团队进入 handoff-ready 准备。
- 输入依据：上述 artifact 与五个角色审查结果。
- 结论：核心团队已组建，第一批 P0 foundation stories 已确定。
- 风险：PostgreSQL 自建、Qdrant cluster、Neo4j 生产必选增加运维复杂度；50 并发基线、只读隔离、read-after-write fallback 需要实现和验证。
- 待确认项：企业审计日志保留周期和敏感字段脱敏规则。
- 下一跳角色：architect、backend-engineer、qa-engineer、devops-engineer。
- 当前阶段：design-review。
- 目标阶段：handoff-ready。
- 就绪状态：not-ready。
- readiness proof：PRD、Delivery Plan、Test Plan、Architecture、ADR-0001/0002/0003 已存在；五角色已完成执行建议；用户已确认应用等级不适用、50 并发、PostgreSQL 自建、Qdrant cluster、Neo4j 必选、告警暂不考虑、历史数据只读隔离、outbox 延迟可接受、read-after-write fallback 支持；尚缺 design review 结论、P0 实施和运维演练证据。
- accepted_by：tech-lead 初步接受；其他角色待正式 handoff 确认。
- 阻塞项：企业审计日志保留周期和敏感字段脱敏规则待确认；P0 实施与运维演练证据未完成。
