# RLS Context Strategy: Memory Storage Reliability

## 结论

Memory storage 的 PostgreSQL RLS 必须使用 transaction-local tenant context，禁止使用 session-level `SET`。所有生产读写路径都应通过 tenant-scoped transaction 或 tenant-scoped executor 执行，确保连接池复用不会污染后续请求。

本文件支撑 P0-S3：RLS context strategy spike。

## 目标

- 缺失 tenant context 时默认拒绝 memory-owned table 访问。
- 同一 SQLx pool 连接复用时，不会把上一个请求的 tenant 泄漏到下一个请求。
- RLS context 与业务 SQL 在同一个 transaction 内生效。
- 后续 repository 改造可以复用统一 helper，避免每个方法手写 context 设置。

## 非目标

- 本阶段不立即启用 RLS enforce。
- 本阶段不删除 legacy prefix 字段。
- 本阶段不处理非 memory storage 表。
- 本阶段不定义完整 DB role / permission 模型，但 release 前必须补充生产 DB 用户不能 bypass RLS 的检查。

## 推荐实现模型

### Tenant-scoped transaction helper

建议新增一个基础 helper，位置可选：

- `backend/src/db/tenant_scope.rs`
- 或 `backend/src/tenant/db_context.rs`

目标接口形态：

```rust
pub async fn begin_tenant_transaction(
    pool: &sqlx::PgPool,
    tenant_id: &TenantId,
) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, AppError>;
```

内部行为：

1. `pool.begin().await` 开启 transaction。
2. 在 transaction 内执行 `SELECT set_config('aetheris.tenant_id', $1, true)`。
3. 返回 transaction，业务 SQL 必须使用该 transaction 执行。
4. transaction commit / rollback 后，tenant context 自动失效。

关键 SQL：

```sql
SELECT set_config('aetheris.tenant_id', $1, true);
```

第三个参数 `true` 表示 local to current transaction。

### RLS policy 模板

```sql
ALTER TABLE knowledge_entries ENABLE ROW LEVEL SECURITY;
ALTER TABLE knowledge_entries FORCE ROW LEVEL SECURITY;

CREATE POLICY knowledge_entries_tenant_isolation
ON knowledge_entries
USING (
    current_setting('aetheris.tenant_id', true) IS NOT NULL
    AND tenant_id = current_setting('aetheris.tenant_id', true)
)
WITH CHECK (
    current_setting('aetheris.tenant_id', true) IS NOT NULL
    AND tenant_id = current_setting('aetheris.tenant_id', true)
);
```

适用表：

- `context_sessions`
- `context_messages`
- `session_messages`
- `knowledge_entries`
- `knowledge_relations`
- `knowledge_entry_versions`
- `entities`
- `relations`
- `reasoning_paths`
- `entity_versions`
- `multimodal_entries`
- `modality_relations`
- `memory_vector_outbox`
- `memory_vector_reconciliation_runs`
- `memory_vector_reconciliation_items`
- `memory_audit_events`

`memory_tenant_readonly_isolation` 可以允许 admin 角色读取，但普通 request path 不应绕过 tenant gate。

## Repository 使用规则

### Production path

- create/update/delete/history/time-travel/relation mutation 必须通过 tenant-scoped transaction。
- read path 可以使用 tenant-scoped executor；若 RLS enforce 后仍直接使用 pool，会因缺失 context 被拒绝。
- 不能在 repository 内从 `source_id` 或 `entity_id` 推导 tenant 作为授权依据。

### Migration / backfill path

- backfill 可以使用独立 admin execution path。
- backfill 结果必须写入 report。
- 无法归属数据进入只读隔离。
- migration/backfill admin path 不应复用 request path 的 DB role。

### Legacy wrapper

- 无 tenant wrapper 只能保留给 test-only、migration-only 或明确 deprecated 兼容入口。
- production router/service 不允许调用无 tenant wrapper。

## 测试要求

### RED tests

1. `rls_rejects_memory_query_without_tenant_context`
2. `rls_rejects_memory_write_without_tenant_context`
3. `tenant_context_is_transaction_local`
4. `tenant_context_does_not_leak_across_pool_connection_reuse`
5. `tenant_a_transaction_cannot_read_tenant_b_rows`
6. `tenant_a_transaction_cannot_write_tenant_b_rows`

### 连接池污染测试

测试流程建议：

1. 获取 pool。
2. transaction A 设置 tenant A，查询 tenant A 数据，commit。
3. 新 transaction B 不设置 tenant context，尝试查询 memory-owned table。
4. 预期拒绝或 0 行，不允许继承 tenant A。
5. transaction C 设置 tenant B，确认只能访问 tenant B。

## 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| 使用 session-level `SET` | 连接池复用导致跨租户污染 | 只允许 `set_config(..., true)` 或 `SET LOCAL` |
| DB 用户 bypass RLS | policy 失效 | production DB 用户不得是 owner/superuser/bypassrls |
| repository 混用 pool 与 tx | 部分 SQL 缺失 context | lint/grep + review gate，生产方法签名接收 tenant executor |
| backfill admin path 权限过大 | 误操作历史数据 | 独立 runbook、dry-run、只读隔离、审计 |
| RLS enforce 过早 | 历史数据不可见 | enforce 前必须 backfill report + negative tests 通过 |

## Handoff

- 背景：P0-S3 需要确定 RLS context 与连接池安全策略。
- 输入依据：ADR-0001、team execution plan、tenant foundation migration。
- 结论：采用 transaction-local tenant context，禁止 session-level setting。
- 风险：DB role、repository executor 和 backfill admin path 需要后续实现严格区分。
- 待确认项：生产 DB role 是否能保证不 bypass RLS。
- 下一跳角色：backend-engineer / security reviewer / qa-engineer。
- 当前阶段：design-review。
- 目标阶段：handoff-ready。
- 就绪状态：ready-for-review。
- readiness proof：本文件定义 RLS context helper、policy 模板和测试要求。
- accepted_by：待 backend-engineer 确认。
- 阻塞项：代码尚未实现，RLS 尚未 enforce。
