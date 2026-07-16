# Memory Storage Reliability Architecture

## Why This Matters

记忆存储承载 agent 的长期上下文、短期会话、知识图谱、检索索引和多模态内容。企业内部场景不仅要求功能可用，还要求数据隔离、事务一致性、恢复能力、审计证据和发布回滚都能被验证。

当前仓库已经具备基础 memory substrate，但审查结论显示：它更接近本地开发 / PoC / MVP 可用状态，尚未达到企业高可靠存储的准入要求。本设计说明用于区分 current repository state 与 target model，避免把 planned 能力误写成 implemented。

## Current Repository State

### 已有基础

- PostgreSQL migrations 覆盖 STM、LTM、KG、MM、decision trace、bi-temporal tracking 等基础表。
- Qdrant collection 初始化、向量写入和 tenant payload filter 已存在。
- LTM 写入流程已意识到 Qdrant 与 DB 双写问题：Qdrant 写成功、DB 写失败时会尝试删除 Qdrant point。
- 请求侧多数 memory routes 使用 request-scoped tenant context。
- `backend/tests/tenant_isolation.rs` 与 `backend/tests/memory_platform_e2e.rs` 覆盖部分租户隔离和环境型 E2E。
- Prometheus、OpenTelemetry、Jaeger、Grafana datasource 等 observability 基础配置存在。
- `docker-compose.yml` 提供 PostgreSQL、Qdrant、Neo4j、Redis、backend、frontend 和观测组件的本地开发拓扑。

### 主要缺口

- STM / LTM / KG / MM 主表缺少统一显式 `tenant_id` 作为数据库级安全边界。
- 当前租户隔离主要依赖 `source_id`、`user_id`、`entity_id` 的 `t:{tenant}` 前缀和应用层约定。
- 部分 update、history、time-travel、relation mutation 方法没有强制 tenant 参数。
- STM add message、delete session、KG create relation 等多步写入缺少事务边界。
- LTM 与 Qdrant 是同步双写 + 局部补偿，没有 durable outbox、幂等重放、dead-letter 或 reconciliation。
- Qdrant tenant payload 缺失或错误时，缺少统一的回源校验和修复闭环。
- PostgreSQL、Qdrant、Neo4j 当前没有生产 HA、备份恢复、PITR、snapshot schedule 或 restore drill 证据。
- Prometheus / Grafana 基础存在，但缺少 alert rules、Alertmanager / Grafana alerting、SLO、pager routing 和恢复失败告警。
- `backend/src/distributed/` 中 replication / consensus 更接近接口或骨架，不应被视为生产级 HA 已实现。

## Target Model

### Tenant Isolation Model

目标是将租户隔离从“应用前缀约定”升级为三层防线：

1. **Schema layer**：memory-owned tables 显式包含 `tenant_id TEXT NOT NULL`。
2. **Database policy layer**：PostgreSQL RLS 或等价策略默认拒绝缺失租户上下文的访问。
3. **Application layer**：router / service / repository 方法显式传递 `TenantId`，不允许使用隐式 default tenant 进入生产路径。

目标查询模式：

- 单条读取：`WHERE tenant_id = $tenant_id AND id = $id`
- 业务唯一：`UNIQUE (tenant_id, business_key)`
- 列表查询：`WHERE tenant_id = $tenant_id AND status = ... ORDER BY created_at ...`
- history/time-travel：`WHERE tenant_id = $tenant_id AND memory_id = $memory_id AND valid_time ...`

### Transaction Model

所有多表或多步骤状态变更必须在明确事务边界内完成。

必须事务化的代表路径：

- STM session create / message append / context length update。
- STM delete session 与相关 messages 删除。
- KG relation insert 与两个 entity relation_count 更新。
- LTM supersede、history append、status update。
- outbox event 与 relational fact 写入。

事务失败时，不能留下半写 message、relation、history 或错误计数。

### Vector Index Consistency Model

LTM relational store 是事实源；Qdrant 是可重建的检索索引。

目标写入模型：

1. 在 PostgreSQL transaction 中写入 LTM entry / metadata / history。
2. 同一 transaction 写入 outbox event，包含 tenant_id、entry_id、operation、payload hash、idempotency key。
3. outbox worker 异步 upsert / delete Qdrant。
4. worker 使用幂等 key，重复执行不会产生重复或错误状态。
5. Qdrant 操作成功后标记 outbox event 为 applied。
6. 失败进入 retry，超过阈值进入 dead-letter 并触发告警。
7. reconciliation job 定期比较 PostgreSQL active entries 与 Qdrant payload，发现缺失、孤儿或 tenant mismatch 并生成 repair report。

搜索语义：

- Qdrant 返回结果必须包含 tenant payload。
- 服务层必须回源 PostgreSQL 按 tenant_id 校验 entry 仍存在且可见。
- outbox lag 期间，新写入 LTM 可以短暂不可被向量搜索命中，但事实写入已经持久化。

### Operational Readiness Model

企业高可靠准入必须包含：

- PostgreSQL HA / failover 方案或托管服务说明。
- PostgreSQL PITR / WAL archive / backup restore runbook。
- Qdrant cluster 或 snapshot + rebuild 策略。
- Neo4j backup / dump / restore 策略。
- RPO / RTO 定义和演练记录。
- 告警规则和接收 owner。
- 发布前 migration dry-run、rollback / forward-fix 策略。
- 发布后 observation window 和 closeout summary。

## Reliability Guarantees

目标状态下应提供以下保证：

- 不存在没有租户归属的 memory-owned 行。
- 不存在跨租户 update、delete、history、time-travel 或 relation mutation。
- 缺少 tenant context 的数据库访问默认失败。
- 多步写入要么全部成功，要么全部回滚。
- PostgreSQL 是 LTM 事实源，Qdrant 可通过 outbox / reconciliation 恢复。
- Qdrant orphan vector、missing vector、tenant payload mismatch 都能被扫描、告警和修复。
- 备份恢复和发布回滚不是口头流程，而是有命令、结果和 owner 的证据。

## Storage Responsibilities

| 组件 | 目标职责 | 可靠性边界 |
|------|----------|------------|
| PostgreSQL | STM/LTM/KG/MM 事实源、history、outbox、audit | 事务、RLS、PITR、backup restore |
| Qdrant | LTM 向量检索索引 | payload tenant filter、幂等 upsert、snapshot/rebuild |
| Neo4j | 可选图查询增强或外部 KG backend | backup/restore、cluster 或明确非生产依赖 |
| Redis | 缓存、队列或短期协调能力 | 不作为不可恢复事实源，必须可重建 |
| Outbox worker | Qdrant 同步、重试、dead-letter | 幂等、可观测、可暂停恢复 |
| Reconciliation job | DB/Qdrant 漂移检测与修复 | 定期报告、repair evidence、告警 |
| Audit store | 写入、删除、transfer、隔离拒绝事件 | 持久化、可查询、脱敏 |

## Migration Strategy

### Phase 1: Expand

- 为 memory-owned tables 增加 nullable `tenant_id`。
- 增加非唯一 tenant indexes，先不强制 NOT NULL。
- 新写路径 dual-write `tenant_id` 与既有 prefix 字段。
- 增加数据归属报告，识别无法 backfill 的历史行。

### Phase 2: Backfill

- 从 `t:{tenant}` prefix、request metadata 或已有 tenant registry 推导 tenant_id。
- 无法归属的数据进入 quarantine 或只读隔离区。
- 产出 backfill 结果：总行数、成功数、失败数、无法归属样例、处理 owner。

### Phase 3: Dual-read / Dual-write

- repository 优先使用 `tenant_id` 查询，保留 prefix 兼容 fallback。
- 所有 update/history/time-travel/relation API 必须显式传入 TenantId。
- 增加跨租户 negative tests。

### Phase 4: Enforce

- 将 `tenant_id` 改为 NOT NULL。
- 添加 tenant-scoped unique constraints。
- 启用 RLS policy 或等价数据库级策略。
- 禁止生产路径使用 default tenant wrapper。

### Phase 5: Cleanup

- 移除 prefix 作为安全边界的依赖，仅保留为可读业务标识或兼容字段。
- 删除或限制 legacy wrappers。
- 更新架构文档、API 文档和 release notes。

## Failure Modes

| Failure Mode | 目标处理方式 |
|--------------|--------------|
| Qdrant upsert 失败 | outbox 保持 pending / failed，可重试并告警 |
| DB commit 成功但 worker 崩溃 | worker 重启后从 durable outbox 继续处理 |
| outbox event 重复处理 | idempotency key 确保重复 upsert 安全 |
| Qdrant 中存在孤儿向量 | reconciliation 标记 orphan 并删除或隔离 |
| Qdrant 缺少 active entry 向量 | reconciliation 重新生成/重放索引 |
| Qdrant tenant payload 错误 | 搜索回源校验拒绝返回，并生成 repair event |
| RLS context 未设置 | 数据库访问默认拒绝，测试覆盖该场景 |
| migration backfill 部分失败 | 阻塞 enforce 阶段，无法归属数据进入 quarantine |
| PostgreSQL restore 后 Qdrant 漂移 | 使用 outbox/reconciliation 或 snapshot 恢复索引 |
| 发布后错误 migration | 走 forward-fix 或已验证 rollback 策略，触发观察窗口 |

## API Surface

目标 API 设计原则：

- request tenant 只来自认证上下文或受信任 server-side context，不从 body/query 直接信任。
- history、time-travel、update、delete、relation mutation 必须使用 request tenant。
- 批量写入返回部分成功语义：成功列表、失败列表、可重试标识、correlation id。
- Qdrant backfill/reconciliation/repair endpoint 必须是受保护 admin API，默认 dry-run。
- outbox 和 reconciliation 状态暴露只提供汇总指标与脱敏详情。

## Observability and Verification

### Metrics

- memory write latency / error rate by layer。
- tenant isolation violation count。
- outbox pending / failed / dead-letter count。
- outbox oldest pending age。
- reconciliation drift count。
- Qdrant orphan / missing / tenant mismatch count。
- backup age and restore drill status。
- DB/Qdrant/Neo4j availability。

### Alerts

- DB unavailable。
- Qdrant unavailable。
- Neo4j unavailable when production KG requires it。
- outbox backlog exceeds threshold。
- reconciliation drift exceeds threshold。
- tenant isolation violation detected。
- backup age exceeds RPO。
- restore drill failed or overdue。
- migration failed during release window。

### Audit Events

持久化审计至少覆盖：

- STM/LTM/KG/MM create、update、delete。
- STM to LTM transfer。
- KG relation mutation。
- Qdrant repair / reconciliation action。
- cross-tenant access rejected。
- admin backfill / repair dry-run and execution。
- restore drill and release rollback record。

## Documentation Boundaries

- 当前 `docker-compose.yml` 应视为 local/dev 拓扑，不代表生产 HA。
- 当前 tenant prefix isolation 是兼容机制，不是长期安全边界。
- 当前 Qdrant rollback compensation 是局部保护，不代表 durable consistency。
- 当前 Prometheus/Grafana 配置是采集基础，不代表告警闭环。
- 当前 distributed 模块是能力骨架，不代表生产 consensus/replication 已实现。

## Gap Summary

已具备：

- 基础 memory persistence。
- 部分 request-scoped tenant context。
- Qdrant tenant payload filter。
- 局部双写补偿。
- migration、E2E 和 observability 基础。

未完成：

- schema-level tenant isolation。
- RLS / database policy。
- 全路径 TenantId 强制传递。
- 多步写事务化。
- durable outbox 和 reconciliation。
- 生产 HA、备份恢复、灾备演练。
- 告警闭环和发布回滚证据。
- 持久化 audit coverage。

## Recommended Next Steps

1. 以 `docs/artifacts/2026-07-06-memory-storage-reliability/prd.md` 锁定企业高可靠整改范围。
2. 按 ADR-0001、ADR-0002、ADR-0003 锁定长期架构决策。
3. 先实施 tenant_id migration + backfill report，再启用 repository TenantId 强制传递。
4. 补齐 STM/KG/LTM 关键写路径事务。
5. 引入 LTM/Qdrant outbox 和 reconciliation。
6. 建立 backup/restore/alert/release runbook 和 operational drills。
7. P0 测试和演练全部通过后，再进入企业生产 release gate。
