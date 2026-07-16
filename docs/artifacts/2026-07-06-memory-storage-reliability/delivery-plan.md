# Delivery Plan: Memory Storage Reliability

## 版本目标

### 版本 / 里程碑

- 里程碑：Memory Storage Reliability P0/P1 Remediation
- 当前状态：review complete / remediation planned
- 目标阶段：从 MVP 可用性提升到企业内部高可靠准入前的设计与执行准备

### 范围说明

本计划承接 `prd.md` 和 `docs/architecture/memory-storage-reliability.md`，将企业高可靠审查中的 Critical / High 风险拆解为可执行工作项。

重点整改方向：

1. schema-level tenant isolation。
2. repository / router 全路径 TenantId 强制传递。
3. STM / KG / LTM 多步写事务化。
4. LTM + Qdrant durable outbox / reconciliation。
5. backup / restore / HA / release rollback runbook 与演练；告警暂不纳入当前门禁。
6. P0 测试矩阵、50 并发基线验证和 operational drills。

### 放行标准

P0 放行前必须满足：

- 所有 P0 工作项完成并有代码、migration 或文档证据。
- `test-plan.md` 中 P0 schema、unit、integration、E2E 和 operational drill 有执行结果。
- Critical 风险全部关闭；High 风险要么关闭，要么由 tech-lead 显式接受并记录。
- `deployment-context.md`、`release-plan.md`、`launch-acceptance.md` 在正式发布前补齐。
- 不再将 local/dev docker-compose、基础 healthcheck 或功能 E2E 误认为生产 HA 证据。

## 工作拆解

| 工作项 | 主责角色 | 依赖 | 计划阶段 | 完成条件 |
|--------|----------|------|----------|----------|
| P0-1 tenant_id schema migration 设计 | backend-engineer | ADR-0001 | Design / Execute | STM/LTM/KG/MM/history/relation/outbox 表 tenant_id 方案和 backfill plan 通过 review |
| P0-2 历史数据 backfill 与 quarantine 策略 | backend-engineer | P0-1 | Execute | 产出数据归属报告，无法归属数据有处理策略 |
| P0-3 PostgreSQL RLS / database policy 设计 | backend-engineer | P0-1 | Execute | 缺失 tenant context 默认拒绝的测试通过 |
| P0-4 router/service/repository TenantId 强制传递 | backend-engineer | P0-1 | Execute | update/delete/history/time-travel/relation 路径不再使用隐式 tenant |
| P0-5 STM 多步写事务化 | backend-engineer | P0-4 | Execute | add message、delete session 故障注入测试无半写 |
| P0-6 KG relation 写路径租户化与事务化 | backend-engineer | P0-4 | Execute | 跨租户 relation 被拒绝，relation_count 一致 |
| P0-7 LTM supersede/history/update 事务化 | backend-engineer | P0-4 | Execute | history/update/time-travel tenant negative tests 通过 |
| P0-8 LTM/Qdrant outbox schema 与 worker | backend-engineer | ADR-0002 | Execute | DB fact + outbox transaction；worker 幂等 upsert/delete |
| P0-9 Reconciliation dry-run / repair | backend-engineer | P0-8 | Execute | missing/orphan/tenant mismatch 可检测、可报告、可修复 |
| P0-10 持久化 audit event 范围落地 | backend-engineer | P0-4/P0-8 | Execute | 写入、删除、transfer、隔离拒绝、repair 事件可查询 |
| P0-11 PostgreSQL 自建 HA / backup / restore runbook | devops-engineer | 目标环境确认 | Release Prep | 自建拓扑、PITR 或备份恢复演练记录齐备 |
| P0-12 Qdrant cluster / snapshot / rebuild runbook | devops-engineer | P0-8/P0-9 | Release Prep | cluster 方案、snapshot restore 或 DB+outbox rebuild 演练通过 |
| P0-13 Neo4j backup / restore runbook | devops-engineer | Neo4j 生产必选 | Release Prep | dump/backup/restore 演练通过 |
| P0-14 50 并发基线验证 | qa-engineer / devops-engineer | P0-8/P0-9 | Review | 50 并发下写入、搜索、outbox lag 和恢复影响有验证结果 |
| P0-15 可靠性测试矩阵执行 | qa-engineer | P0-1~P0-14 | Review | `test-plan.md` P0 项全部有执行结果 |
| P0-16 发布回滚与上线准入文档 | devops-engineer / tech-lead | P0-15 | Release | deployment-context、release-plan、launch-acceptance 齐备 |
| P1-1 批量 LTM 部分成功契约 | backend-engineer | P0-8 | Execute | 返回成功/失败列表、可重试标识和 correlation id |
| P1-2 outbox / reconciliation dashboard | devops-engineer | P0-8/P0-9 | Review | dashboard 展示 backlog、dead-letter、drift、repair 状态 |
| P1-3 migration 性能与锁影响评估 | backend-engineer / qa-engineer | P0-1 | Review | 大数据量 dry-run 报告和锁风险说明完成 |
| P1-4 租户隔离 fuzz / property tests | qa-engineer | P0-4 | Review | 随机 tenant/resource 组合无泄漏 |

## 风险与缓解

| 风险 | 影响 | 缓解措施 | Owner |
|------|------|----------|-------|
| tenant_id backfill 无法覆盖历史数据 | 阻塞 NOT NULL 和 RLS enforce | 先运行归属报告；无法归属数据进入 quarantine；tech-lead 决策是否保留 | backend-engineer |
| RLS context 与连接池状态污染 | 误拒绝或跨租户泄漏 | 使用 transaction-local setting；测试连接复用场景 | backend-engineer |
| outbox 引入 eventual consistency | 新写入记忆短暂不可搜索 | API 文档说明；暴露 outbox lag；关键路径可提供 read-after-write fallback | architect |
| reconciliation repair 误删有效向量 | 数据检索能力下降 | repair 默认 dry-run；执行前输出 diff；高风险操作需要 admin approval | backend-engineer |
| 备份恢复依赖自建平台能力 | runbook 和演练需要额外环境准备 | 在 deployment context 记录 PostgreSQL 自建、Qdrant cluster、Neo4j 必选的拓扑、owner 和演练窗口 | devops-engineer |
| 告警暂不纳入当前门禁 | 故障发现闭环弱于完整生产级要求 | 在 launch acceptance 中显式记录为接受风险；后续正式生产发布前重新评估告警 owner 和路由 | tech-lead |
| 50 并发基线未验证 | 容量目标无法证明 | 在 QA / DevOps 验证中覆盖并发写入、搜索、outbox lag 和恢复影响 | qa-engineer |
| migration 锁表影响生产 | 发布风险升高 | expand-contract、分批 backfill、限速、可中断恢复 | backend-engineer |

## 节点检查

### 1. 方案评审

进入实现前检查：

- PRD、架构设计、ADR-0001/0002/0003 已完成 review。
- tenant_id migration / backfill / RLS 方案通过 design review。
- outbox 状态机、worker、reconciliation 策略通过 design review。
- 运维 runbook 范围、50 并发基线和自建/cluster/必选组件拓扑已确认。

### 2. 开发完成

进入 QA 前检查：

- 所有 P0 代码路径有单元或集成测试。
- migration dry-run 完成。
- legacy default tenant wrappers 不在生产路径使用。
- outbox / reconciliation metrics 可采集。
- audit event 关键路径可查询。

### 3. 测试完成

进入发布准备前检查：

- `cargo fmt --check` 通过。
- `cargo clippy --all-targets --all-features -- -D warnings` 通过。
- `cargo test --all-features` 通过。
- 环境型 E2E 与 reliability E2E 通过。
- tenant negative tests、事务故障注入、outbox retry、reconciliation drift 测试通过。
- PostgreSQL restore、Qdrant rebuild、Neo4j restore、rollback drill 有证据。
- 50 并发下写入、搜索、outbox lag 和恢复影响验证通过或残余风险被 tech-lead 接受。

### 4. 发布准备

正式发布前检查：

- `deployment-context.md` 完成。
- `release-plan.md` 完成。
- `launch-acceptance.md` 完成，结论为允许上线或有条件上线。
- 回滚 / forward-fix 路径经过演练或有明确执行命令。
- observation window、值守 owner 和升级路径明确。

## 当前阻塞项

- 应用等级不适用，已关闭；50 并发基线、PostgreSQL 自建、Qdrant cluster、Neo4j 生产必选、告警暂不考虑、历史数据只读隔离、outbox 延迟可接受、read-after-write fallback 支持已确认。
- 代码尚未实现 tenant_id schema、RLS、outbox、reconciliation 和 operational drills。
- 企业审计日志保留周期和敏感字段脱敏规则仍待确认。
- 因此当前不能进入企业内部生产级高可靠放行。

## Handoff

- 背景：企业高可靠审查发现 memory storage 存在 Critical / High 风险，需要进入整改。
- 输入依据：可靠性审查结果、`prd.md`、`docs/architecture/memory-storage-reliability.md`、ADR-0001/0002/0003。
- 结论：当前整改计划已拆解为 P0/P1 工作项。
- 风险：migration、RLS、outbox eventual consistency、运维依赖和恢复演练未完成。
- 待确认项：企业审计日志保留周期和敏感字段脱敏规则。
- 下一跳角色：architect / backend-engineer / devops-engineer / qa-engineer。
- 当前阶段：design-review。
- 目标阶段：handoff-ready。
- 就绪状态：not-ready。
- readiness proof：已完成审查结论与整改文档计划；尚未完成 design review、implementation readiness 和 release gate。
- accepted_by：tech-lead 已确认关键输入；其他角色待正式 handoff 确认。
- 阻塞项：P0 实施与运维证据未完成；企业审计日志保留周期和敏感字段脱敏规则待确认。
