# Release Plan: Memory Storage Reliability

## 发布信息

| 字段 | 内容 |
|------|------|
| Release 名称 | Memory Storage Reliability P0/P1 Remediation |
| 当前状态 | Planned / not release-ready |
| 目标阶段 | design-review → handoff-ready → execute → review → release |
| 关联 PRD | `docs/artifacts/2026-07-06-memory-storage-reliability/prd.md` |
| 关联架构 | `docs/architecture/memory-storage-reliability.md` |
| 关联 ADR | ADR-0001 / ADR-0002 / ADR-0003 |
| 关联测试计划 | `docs/artifacts/2026-07-06-memory-storage-reliability/test-plan.md` |
| 关联部署上下文 | `docs/artifacts/2026-07-06-memory-storage-reliability/deployment-context.md` |
| 发布负责人 | 待 tech-lead 指定 |
| 执行人 | backend-engineer / devops-engineer |
| 验收人 | qa-engineer / tech-lead |

## 变更与风险

### 变更范围

- 新增 `tenant_id` schema、tenant-scoped indexes/constraints。
- 引入历史数据 backfill report 和无法归属数据只读隔离。
- 改造 repository / service / router 为 TenantId 强制传递。
- 引入 PostgreSQL RLS 或等价 database policy。
- STM / KG / LTM 关键写路径事务化。
- LTM + Qdrant 从同步双写切换为 PostgreSQL fact + durable outbox + async worker + reconciliation。
- Qdrant 采用 cluster 方向，并保留 DB + outbox/reconciliation rebuild。
- Neo4j 作为生产必选组件纳入 restore drill。
- 支持 read-after-write fallback：按 ID 回源 PostgreSQL 读取，向量搜索允许 `index_status=pending`。
- 50 并发作为本轮容量与性能基线。

### 主要风险

| 风险 | 影响 | 处理方式 |
|------|------|----------|
| tenant_id backfill 错误 | RLS enforce 后数据不可见或错租户 | 先 dry-run 和归属报告；无法归属数据只读隔离 |
| RLS context 污染连接池 | 误拒绝或跨租户泄漏 | 只使用 transaction-local `SET LOCAL` / `set_config(..., true)` |
| outbox eventual consistency | 新写入短暂不可向量搜索 | 返回 `index_status=pending`，支持 read-after-write fallback |
| Qdrant repair 误删 | 检索数据缺失 | reconciliation 默认 dry-run，repair 需显式确认 |
| PostgreSQL 自建 HA 方案不足 | 恢复能力不足 | release 前完成 restore drill 和 rollback/forward-fix 策略 |
| Neo4j 必选但未演练 | KG 生产路径不可恢复 | release 前完成 dump/restore 或等价 drill |
| 告警暂不考虑 | 故障发现闭环弱 | 在 launch acceptance 中记录为接受风险，后续生产发布前重新评估 |

## 执行步骤

### Phase 0: Preflight

- 确认 `team-execution-plan.md` 中的已确认决策仍有效。
- 完成 architecture design review。
- 完成 P0 RED tests 列表。
- 确认 50 并发验证方法。
- 确认审计日志保留周期和敏感字段脱敏规则；若未确认，release 状态保持 blocked。

### Phase 1: Expand

- 新增 nullable `tenant_id` columns。
- 新增 tenant-scoped 非唯一索引。
- 新增 outbox / audit / reconciliation 相关表。
- 新写路径 dual-write `tenant_id` 与 legacy prefix。
- 不启用 RLS enforce。

验证：

- migration dry-run 通过。
- schema verification 通过。
- 旧路径 smoke 通过。

### Phase 2: Backfill + Read-only Isolation

- 从 legacy prefix、metadata 或受信上下文推导 tenant。
- 输出 backfill report。
- 无法归属数据进入只读隔离。
- 不猜测归属，不默认删除。

验证：

- backfill 总量、成功量、只读隔离量可追溯。
- 隔离数据不可被正常租户读写。

### Phase 3: TenantId Production Path

- 改造 update/delete/history/time-travel/relation mutation 为 TenantId 强制传递。
- 移除生产路径对 default tenant wrapper 的依赖。
- 增加 tenant negative tests。

验证：

- tenant A 无法读写 tenant B 的 STM/LTM/KG/MM 数据。
- router/service/repository 路径均绑定 request tenant。

### Phase 4: Transaction Boundaries

- STM add/delete 事务化。
- KG relation + relation_count 事务化。
- LTM supersede/history/update 事务化。
- outbox event 与 fact 写入同事务。

验证：

- fault injection tests 证明失败时无半写。

### Phase 5: RLS Enforce

- 使用 transaction-local setting 设置 tenant context。
- 启用 RLS policy。
- 验证缺失 tenant context 默认拒绝。

验证：

- RLS missing context negative tests 通过。
- 连接池复用污染测试通过。

### Phase 6: Outbox + Reconciliation

- LTM 写入改为 DB fact + outbox。
- outbox worker 实现 pending / processing / applied / failed / dead-letter。
- Qdrant cluster payload 包含 tenantId / entryId / contentHash。
- search 结果回源 PostgreSQL 校验。
- reconciliation dry-run / repair 实现。

验证：

- worker paused / resumed 测试通过。
- duplicate event idempotency 通过。
- missing / orphan / tenant mismatch dry-run 和 repair 通过。
- read-after-write fallback 通过。

### Phase 7: Operational Drills

- PostgreSQL 自建 restore drill。
- Qdrant cluster / snapshot restore 或 DB+outbox rebuild drill。
- Neo4j dump/restore drill。
- migration rollback / forward-fix drill。
- 50 并发验证。

验证：

- drill evidence 按 test-plan 格式归档。
- 50 并发下写入、搜索、outbox lag、恢复影响有结果。

## 验证与监控

### 自动化验证命令

```bash
cd backend
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo sqlx prepare --check
AMS_E2E=1 cargo test --test memory_platform_e2e
AMS_E2E=1 cargo test --test memory_reliability_e2e
```

### 必须有证据的验证

- schema verification。
- tenant negative tests。
- transaction fault injection。
- outbox retry / dead-letter / idempotency。
- reconciliation dry-run / repair。
- PostgreSQL restore。
- Qdrant rebuild。
- Neo4j restore。
- rollback / forward-fix。
- 50 并发基线。

### 监控说明

告警暂不作为当前门禁。正式生产发布前建议重新评估：

- DB/Qdrant/Neo4j availability。
- Outbox backlog / dead-letter。
- Reconciliation drift。
- Tenant isolation violation。
- Backup age / restore drill status。

## 回滚方案

### 可回滚

- 应用镜像变更。
- feature flag。
- outbox worker 启停。
- 未 enforce 的 dual-write / dual-read 代码路径。

### 应 forward-fix

- 已写入生产数据的 tenant_id migration。
- 已执行 backfill 的数据归属结果。
- 已启用 outbox 后的 PostgreSQL fact。
- Qdrant drift，必须以 PostgreSQL 为事实源 repair/rebuild。
- RLS policy 问题优先修复 policy 或 transaction context，不 destructive rollback。

### 发布中止条件

- tenant negative tests 失败。
- RLS missing context 测试失败。
- PostgreSQL restore drill 失败。
- Qdrant rebuild / repair 失败。
- Neo4j restore drill 失败。
- migration dry-run 出现不可接受锁表或数据损坏风险。
- 50 并发基线下错误率或 outbox lag 超过 tech-lead 接受范围。

## 放行结论

当前 release 结论：**blocked**。

原因：P0 代码、migration、restore/rebuild/rollback drill、50 并发验证和审计规则尚未完成。

只有在 P0 验证通过并由 tech-lead / QA 给出 launch acceptance 后，才能从 blocked 调整为 conditional allow 或 allow。
