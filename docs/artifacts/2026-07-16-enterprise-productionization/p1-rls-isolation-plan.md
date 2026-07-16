# P1 实施计划 — Schema 级租户隔离（RLS + tenant_id NOT NULL + 接线）

- **日期**：2026-07-16
- **主责角色**：backend-engineer / DBA（本计划）；tech-lead 收口
- **关联入口**：`delivery-plan.md` P1 地基首项（第 42 行）；`docs/adr/ADR-0001-memory-storage-tenant-isolation.md`；`docs/artifacts/2026-07-06-memory-storage-reliability/`（rls-context-strategy / tenant-production-path-inventory / transaction-boundaries）
- **硬约束**：**离线无 PG**——本计划只做设计 + 静态核对，不跑迁移；db 层继续用运行时 `sqlx::query` / `query_as`（非编译期宏），保证离线 `cargo check` 通过。
- **GUC 命名**：统一采用 **`aetheris.tenant_id`**（事务局部），与 rls-context-strategy.md 对齐，避免与将来的 `app.*` 命名空间冲突（原始 prompt 写的是 `app.tenant_id`，见 §8 待确认项）。

---

## 0. 结论先行

1. **当前"隔离"是应用层前缀（`t:{tenant}:` 拼进 `source_id` / `entity_id` / `user_id` + `starts_with` 校验），DB 层零强制**。任何漏查询点、任何绕过仓库直连 SQL 都能跨租户读写。RLS 的价值就是把隔离从"应用自觉"降到"数据库兜底"。
2. **最大阻塞不在 SQL，而在接线载体**：代码用**全局静态连接池** `OnceLock<DatabasePool>` + 直接 `pool()` 取 `&'static PgPool`，**没有任何 per-request 事务/连接抽象**。RLS 依赖事务局部 GUC（`set_config('aetheris.tenant_id', $1, true)`），而**读路径当前根本没有事务载体**。因此 **R1「建 `tenant_scope` 执行器 + 仓库签名收敛为 `(executor, TenantId)` + 读路径也开短事务」是一切的前置**，必须先做，否则 RLS 无处安放。（详见 §3、§8 下游质疑记录）
3. **迁移必须 expand→backfill→enforce→(contract) 分期**，禁止一把梭：M0 已落地（加 nullable `tenant_id` + 复合索引）；M1 回填（从前缀 `split_part` 提取租户，不可归属者落 `'__unattributed__'` 哨兵）；M2 三段式 NOT NULL（`NOT VALID` → `VALIDATE`（`-- no-transaction`）→ `SET NOT NULL`）；M3 启用 RLS + policy；M4 收缩（去前缀列/逻辑）**推迟到过渡期后**。
4. **前缀不能立即删**：M3 之后前缀作为**兼容回退**保留一个过渡期。写路径先"双写"（既写物理 `tenant_id` 列、又保留前缀），读路径先"RLS 兜底 + 应用层 `starts_with` 并存"，观察零违规后再在 M4 收缩。
5. **SQLite 无 RLS**：RLS 只进 `migrations/`（PG），**永不进 `migrations_sqlite/`**。SQLite 优雅降级为应用层 `WHERE tenant_id = $1`。注意 `migrations_sqlite/` 目前只有 2 个文件、缺基表与 foundation，是既有缺陷（见 §5 风险 R-6）。
6. **离线只能静态验证**：扩展现有 `memory_storage_schema_reliability.rs`（读迁移 SQL 文本做断言）+ 新增 `tests/rls_tenant_isolation_pg.rs`（Testcontainers 起真 PG，`#[ignore]` 默认跳过，CI gate 显式跑）。这是唯一能真正证明"跨租户被拒"的手段。

---

## 1. 现状核对（带行号证据）

| 维度 | 现状 | 证据（file:line） |
|---|---|---|
| 连接池 | 全局静态 `OnceLock<DatabasePool>`，`Postgres|Sqlite` 枚举；仓库直接 `pool()` 取 `&'static PgPool` | `backend/src/db/mod.rs:49-54`；各仓库 `use crate::db::pool` |
| 无 per-request 事务抽象 | 写方法各自 `pool.begin()`；读方法直接 `fetch_*(pool)`，**无事务** | `stm.rs:230`、`ltm.rs:619`、`kg.rs:268` 有 tx；读方法全无 |
| STM 写不写物理列 | INSERT `context_sessions` 只写 `source_id = "{prefix}:{user_id}"`，**tenant_id 列为空** | `stm.rs:89`（拼前缀）、`stm.rs:91-110`（INSERT 无 tenant_id） |
| LTM 写不写物理列 | INSERT `knowledge_entries` 只写 `source_id = "{prefix}:{source_id}"` | `ltm.rs:114`、`ltm.rs:116-141` |
| KG 写不写物理列 | INSERT `entities` 只写 `entity_id = "{prefix}:{entity_id}"` | `kg.rs:116`、`kg.rs:118-141` |
| KG 关系**无租户参数** | `create_relation` 签名根本不收 `tenant_id`，INSERT `relations` 无租户列 | `kg.rs:245-252`（签名）、`kg.rs:273-293`（INSERT） |
| MM 用 JSON 兜租户 | 靠 `content_metadata::jsonb ->> 'tenant_id'` 过滤 + `scope_prefixed_id`，**无物理列** | `mm.rs:24-64`、`mm.rs:144-166`、`mm.rs:192/233/318` |
| 隔离靠应用层 `starts_with` | 读到行后 `source_id/entity_id.starts_with(prefix)` 才返回 | `stm.rs:143-158`、`ltm.rs:250-261`、`kg.rs:230-239` |
| 存在无租户全局扫描 | `get_active_user_ids` 全表 DISTINCT，无任何租户过滤 | `stm.rs:582-595`；`mm.rs:455-465` `count` 同样无过滤 |
| M0 已落地 | 12 张核心表加 nullable `tenant_id` + 复合索引；建 outbox/对账/审计/只读隔离表 | `migrations/20260706000100_memory_storage_tenant_foundation.sql` |
| outbox 已是强租户 | `memory_vector_outbox.tenant_id TEXT NOT NULL` + `UNIQUE(tenant_id, idempotency_key)` | 同上 `:139-160` |

**M0 已加 `tenant_id` 的 12 张核心表**：`context_sessions`、`context_messages`、`session_messages`、`knowledge_entries`、`knowledge_relations`、`knowledge_entry_versions`、`entities`、`relations`、`reasoning_paths`、`entity_versions`、`multimodal_entries`、`modality_relations`。

**RLS 目标（请求路径）= 13 张**：上述 12 张 + `memory_vector_outbox`。
**用管理执行器（系统作用域、`tenant_id` 可空）不套请求路径 RLS 的 4 张**：`memory_tenant_readonly_isolation`、`memory_vector_reconciliation_runs`、`memory_vector_reconciliation_items`、`memory_audit_events`。

> 注：`context_messages`、`knowledge_relations`、`knowledge_entry_versions`、`reasoning_paths`、`entity_versions` 在 M0 已加列，但在 `db/{stm,ltm,kg,mm}.rs` 未见活跃写路径（可能经 neo4j/其他路径或暂未使用）。RLS 仍应覆盖它们（纵深防御）；R1 需审计是否有旁路写入，一并补物理列。

---

## 2. 迁移 SQL 分期（M0 已完成，M1–M4 待做）

### M0 — expand（已完成）
`migrations/20260706000100_memory_storage_tenant_foundation.sql`：全部 additive、nullable，不锁表。无需改动。

### M1 — backfill（管理脚本，非事务批处理）
目标：把历史行的物理 `tenant_id` 从既有前缀回填；无法归属者落哨兵 `'__unattributed__'`，并登记到 `memory_tenant_readonly_isolation`，使其在 RLS 下对**任何真实租户不可见**。

**从前缀提取租户的规则**（前缀形如 `t:{tenant}:...`）：
```sql
-- knowledge_entries: source_id = 't:{tenant}:{source}'
UPDATE knowledge_entries
SET tenant_id = split_part(source_id, ':', 2)
WHERE tenant_id IS NULL
  AND source_id LIKE 't:%:%';

-- entities: entity_id = 't:{tenant}:{ulid}'
UPDATE entities
SET tenant_id = split_part(entity_id, ':', 2)
WHERE tenant_id IS NULL
  AND entity_id LIKE 't:%:%';

-- context_sessions: user_id = 't:{tenant}:{uid}'（MVP 也可能 user_id == tenant）
UPDATE context_sessions
SET tenant_id = split_part(user_id, ':', 2)
WHERE tenant_id IS NULL
  AND user_id LIKE 't:%:%';

-- multimodal_entries: 从 content_metadata JSON 取
UPDATE multimodal_entries
SET tenant_id = COALESCE(NULLIF(content_metadata, ''), '{}')::jsonb ->> 'tenant_id'
WHERE tenant_id IS NULL
  AND (COALESCE(NULLIF(content_metadata, ''), '{}')::jsonb ->> 'tenant_id') IS NOT NULL;
```

**子表按父表映射**（`session_messages`←`context_sessions.session_id`、`relations`←源实体、`modality_relations`←源条目、`knowledge_entry_versions`/`entity_versions`/`reasoning_paths` 按各自外键）用 `UPDATE ... FROM` join 回填。

**哨兵兜底 + 登记只读隔离**（对每张表）：
```sql
INSERT INTO memory_tenant_readonly_isolation (isolation_id, table_name, resource_id, reason, evidence_json)
SELECT gen_random_uuid()::text, 'knowledge_entries', entry_id,
       'unattributable_on_backfill', jsonb_build_object('source_id', source_id)::text
FROM knowledge_entries
WHERE tenant_id IS NULL;

UPDATE knowledge_entries SET tenant_id = '__unattributed__' WHERE tenant_id IS NULL;
```
执行要点：分批（`LIMIT` + 主键游标）避免长事务锁；大表回填前 `SET statement_timeout`；回填后跑校验查询确认 `COUNT(*) WHERE tenant_id IS NULL = 0`。M1 以**管理脚本/一次性 job** 形式提供，不放进随应用启动自动跑的 `migrations/`（回填耗时不可控，且需人工确认样本）。

### M2 — enforce NOT NULL（三段式，零停机）
每张表拆 3 个迁移文件，避免 `SET NOT NULL` 全表扫描期间长时间持锁：

`migrations/20260707000100_tenant_not_null_add_check.sql`：
```sql
ALTER TABLE knowledge_entries
  ADD CONSTRAINT knowledge_entries_tenant_not_null
  CHECK (tenant_id IS NOT NULL) NOT VALID;   -- 只对新写入生效，不扫全表
```
`migrations/20260707000200_tenant_not_null_validate.sql`（**首行加 `-- no-transaction`**，`VALIDATE` 取 `SHARE UPDATE EXCLUSIVE`，不能在迁移事务里）：
```sql
-- no-transaction
ALTER TABLE knowledge_entries VALIDATE CONSTRAINT knowledge_entries_tenant_not_null;
```
`migrations/20260707000300_tenant_set_not_null_drop_check.sql`（PG 12+ 已 VALIDATE 的等价 CHECK 可让 `SET NOT NULL` 免全表扫描）：
```sql
ALTER TABLE knowledge_entries ALTER COLUMN tenant_id SET NOT NULL;
ALTER TABLE knowledge_entries DROP CONSTRAINT knowledge_entries_tenant_not_null;
```
**前置门禁**：M2 只能在 M1 回填完成、且**写路径已双写物理列（R1 完成）之后**执行，否则新写入仍为 NULL → `NOT VALID` CHECK 立即拒绝、线上写全挂。

### M3 — enable RLS + policy
`migrations/20260708000100_tenant_rls.sql`，对 13 张请求路径表逐张：
```sql
ALTER TABLE knowledge_entries ENABLE ROW LEVEL SECURITY;
ALTER TABLE knowledge_entries FORCE  ROW LEVEL SECURITY;  -- 表 owner 也受约束，防超级路径绕过
CREATE POLICY knowledge_entries_tenant_isolation ON knowledge_entries
  USING (
    current_setting('aetheris.tenant_id', true) IS NOT NULL
    AND tenant_id = current_setting('aetheris.tenant_id', true)
  )
  WITH CHECK (
    current_setting('aetheris.tenant_id', true) IS NOT NULL
    AND tenant_id = current_setting('aetheris.tenant_id', true)
  );
```
关键点：
- `current_setting(..., true)`（第二参 `true` = missing_ok）：未设 GUC 时返回 NULL → `IS NOT NULL` 判假 → **默认拒绝所有行**（fail-closed，防"忘了设租户"变成"看到全部"）。
- `USING` 管读/更新可见性，`WITH CHECK` 管写入合法性（禁止写入别的租户值）。
- `FORCE ROW LEVEL SECURITY`：连表 owner 也受限。应用连接用**非 owner、非 BYPASSRLS** 角色（见 §3 集成点 + §8 待确认项）。
- 管理执行器需要跨租户的后台任务（对账/回填/`list_qdrant_tenant_backfill_entries`）用**独立的、具备 `BYPASSRLS` 或 owner 身份**的连接，且**永不**经 GUC 设租户 → 见 §3 `begin_admin_tx`。
- 哨兵 `'__unattributed__'` 永不被任何真实请求的 GUC 命中 → 历史脏数据对租户不可见，仅管理执行器可见。

### M4 — contract（**推迟**）
过渡期（RLS 上线 + 观察零违规，建议 ≥2 个发布周期）后再做：去掉写路径的前缀拼接、去掉读路径的 `starts_with` 兜底、评估是否把 `source_id/entity_id` 的前缀剥离。M4 不在本轮范围，仅登记为 backlog，防止过早收缩导致回退无路。

---

## 3. 应用层租户 GUC 接线

### 3.1 核心：新增 `backend/src/db/tenant_scope.rs`（R1，最高优先）
提供**唯一**的"带租户上下文执行"入口，所有请求路径仓库方法改为经它拿执行器。关键：GUC 必须**事务局部**（`set_config(_, _, true)`），因为连接池会复用连接，会话级 `SET` 会把租户泄漏给下一个请求。

设计的四个 helper（签名示意，非最终代码）：
```rust
/// 开启一个已绑定租户 GUC 的事务；读写都用它。
pub async fn begin_tenant_tx(tenant: &TenantId)
    -> Result<sqlx::Transaction<'static, sqlx::Postgres>, AppError> {
    let mut tx = pool().begin().await?;
    // 事务局部；提交/回滚后自动失效，不污染池中连接
    sqlx::query("SELECT set_config('aetheris.tenant_id', $1, true)")
        .bind(tenant.as_str())
        .execute(&mut *tx).await?;
    Ok(tx)
}

/// 只读闭包：开短事务设 GUC，跑闭包，提交。给当前"无事务的读方法"用。
pub async fn with_tenant_read<T, F>(tenant: &TenantId, f: F) -> Result<T, AppError> { /* ... */ }

/// 读写闭包：同上，闭包内多步写，末尾统一 commit。
pub async fn with_tenant_tx<T, F>(tenant: &TenantId, f: F) -> Result<T, AppError> { /* ... */ }

/// 管理执行器：用 BYPASSRLS/owner 连接，绝不设 GUC。仅供对账/回填/跨租户后台任务。
pub async fn begin_admin_tx() -> Result<sqlx::Transaction<'static, sqlx::Postgres>, AppError> { /* ... */ }
```
SQLite 分支：`is_sqlite()` 为真时 `set_config` 是 no-op（或跳过），退化为应用层 `WHERE tenant_id = $1`（M4 前本就并存前缀过滤，读写仍安全）。

### 3.2 仓库签名收敛（R1）
现状读方法各行其是：有的收 `pool: &PgPool`（如 `stm::get_session`），有的直接 `pool()` 全局（如 `ltm::get_entry_history`、`kg::supersede_entity`）。**统一改为收 executor**（`&mut Transaction` 或泛型 `impl PgExecutor`）+ `&TenantId`，由调用方经 `tenant_scope` 提供。写方法把各自的 `pool.begin()` 换成 `begin_tenant_tx(tenant)`。

这一步是 RLS 能落地的物理前提——**没有它，GUC 无处设置**（尤其读方法当前无事务）。

### 3.3 生产写路径补物理 `tenant_id` 列（R1）
在 M2 NOT NULL 之前必须先双写。改动点（INSERT 补 `tenant_id` 列 + 绑定 `tenant.as_str()`）：
- `stm.rs:91` INSERT `context_sessions`（+ `session_messages` at `stm.rs:235`）
- `ltm.rs:116` INSERT `knowledge_entries`（+ `supersede_entry` 新版本 `ltm.rs:644`）
- `kg.rs:118` INSERT `entities`（+ `supersede_entity` `kg.rs:741`）；**`kg.rs:245` `create_relation` 先加 `tenant_id` 参数**再补列（当前无参，最高危）
- `mm.rs:144` INSERT `multimodal_entries`（+ `create_relation` `mm.rs:374`）
前缀拼接**保留**（双写），M4 再评估去除。

### 3.4 请求链路集成点
- **中间件已就绪**：`hoops/jwt.rs:65-67/90-91` 已把 `RequestTenantContext { tenant_id, user_id }` 注入请求扩展；handler 用 `RequestTenantContext` extractor 取 `TenantId`（`tenant/context.rs:109-147`）。无需新中间件。
- **路由/handler**：handler 从 extractor 拿 `tenant`，调用仓库时经 `tenant_scope`（`with_tenant_read` / `with_tenant_tx`）。
- **后台/系统路径**：`memory_transfer`、`information_guard` 扫描、`memory_ingestion` 反射守护、对账 worker、`list_qdrant_tenant_backfill_entries`、`get_active_user_ids` —— 这些跨租户或系统作用域任务用 `begin_admin_tx()`（BYPASSRLS），**显式**与请求路径分流，不得误用租户执行器。
- **连接角色**：应用池连接用非 owner、非 BYPASSRLS 角色；管理任务用独立角色/连接。是否拆两个池 vs 单池 + `SET ROLE`，见 §8 待确认项。

---

## 4. 查询接线清单（逐方法：去前缀 / 保留前缀 / 管理执行器）

图例：**[写]** 补物理列 + `begin_tenant_tx`；**[读]** 经 `with_tenant_read`，RLS 兜底；**去前缀** = M4 可删应用层前缀/`starts_with`；**保留** = 过渡期保留 `LIKE`/`starts_with` 与 RLS 并存；**[admin]** = 走 `begin_admin_tx`。

### `db/stm.rs`（STMRepository）
| 方法 | 行 | 类别 | 处置 |
|---|---|---|---|
| `create_session` | 59-117 | [写] | INSERT(`:91`) 补 `tenant_id` 列；保留 `source_id` 前缀（双写） |
| `get_session` | 120-161 | [读] | RLS 兜底；`starts_with`(`:143-158`) M4 去 |
| `add_message` | 167-283 | [写] | tx(`:230`)→`begin_tenant_tx`；INSERT `session_messages`(`:235`) 补列；app 校验(`:208-224`) M4 去 |
| `get_session_messages` | 289-370 | [读] | RLS 兜底；app 校验(`:333-347`) M4 去 |
| `get_recent_sessions` | 373-408 | [读] | 保留 `LIKE`(`:384/391`) + RLS |
| `list_sessions` | 411-579 | [读] | 保留 `LIKE`（四分支）+ RLS |
| `get_active_user_ids` | 582-595 | [admin] | 全局 DISTINCT 无租户过滤 → 后台/系统路径，走 admin 执行器 |
| `get_active_agent_ids` | 598-619 | [读] | 保留 `LIKE`(`:604`) + RLS |
| `delete_session` | 625-704 | [写] | tx(`:672`)→`begin_tenant_tx`；app 校验(`:654-668`) M4 去 |

### `db/ltm.rs`（LTMRepository）
| 方法 | 行 | 类别 | 处置 |
|---|---|---|---|
| `create_knowledge_entry(_with_id)` | 59-148 | [写] | INSERT(`:116`) 补 `tenant_id` 列；保留 `source_id` 前缀 |
| `update_entry` | 151-182 | [写] | **无租户过滤（`WHERE entry_id` only），跨租户写风险**；加 `(executor, TenantId)` + 补列（当前疑似死路径，接线时必修） |
| `soft_delete_entry` | 185-216 | [写] | 经 `begin_tenant_tx`；已靠 `get_entry_by_id` 校验租户 |
| `get_entry_by_id` | 219-264 | [读] | RLS 兜底；`starts_with`(`:250-261`) M4 去 |
| `get_entries_by_source` | 267-331 | [读] | 保留 `LIKE`(`:276`) + RLS |
| `list_entries` | 334-395 | [读] | 保留 `LIKE`(`:346`) + RLS |
| `count` | 398-413 | [读] | 保留 `LIKE`(`:400`) + RLS（或 admin 做全局统计） |
| `list_qdrant_tenant_backfill_entries` | 416-439 | [admin] | 全局 `source_id LIKE 't:%'` 修复任务 → admin 执行器 |
| `get_entry_at_time` | 444-491 | [读] | RLS；`starts_with`(`:478-488`) M4 去 |
| `search_entries_at_time` | 494-544 | [读] | 保留 `LIKE`(`:503`) + RLS |
| `get_entry_history` | 547-594 | [读] | 当前 `pool()` 全局 + app 过滤(`:579-591`)；改经 `with_tenant_read`，RLS 兜底 |
| `supersede_entry` | 597-683 | [写] | tx(`:619`)→`begin_tenant_tx`；新版本 INSERT(`:644`) 补列 |

### `db/kg.rs`（KGRepository）
| 方法 | 行 | 类别 | 处置 |
|---|---|---|---|
| `create_entity` | 69-148 | [写] | INSERT(`:118`) 补 `tenant_id` 列；保留 `entity_id` 前缀 |
| `get_entity_by_name` | 151-199 | [读] | 保留 `LIKE`(`:158`) + RLS |
| `get_entity_by_id` | 202-242 | [读] | RLS；`starts_with`(`:230-239`) M4 去 |
| `create_relation` | 245-320 | [写] | **签名无 `tenant_id`（`:245-252`）——先加参数**；tx(`:268`)→`begin_tenant_tx`；INSERT `relations`(`:273`) 补列。最高危项 |
| `get_related_entities` | 323-380 | [读] | 经 `get_entity_by_id` → RLS |
| `search_knowledge_by_entity` | 383-390 | [读] | 转默认租户；审查调用方是否应传真实租户 |
| `search_knowledge_by_entity_for_tenant` | 393-433 | [读] | 保留 `LIKE`(`:400`) + RLS |
| `search_entries_by_entity` | 436-443 | [读] | 转默认租户；同上审查 |
| `search_entries_by_entity_for_tenant` | 446-493 | [读] | 保留 `LIKE`(`:452`) + RLS |
| `list_entities` | 496-594 | [读] | 保留 `LIKE`(`:506`) + RLS |
| `get_entity_at_time` | 599-644 | [读] | `pool()` 全局 → `with_tenant_read`；`starts_with`(`:631-641`) M4 去 |
| `get_entity_history` | 647-691 | [读] | `pool()` 全局 + app 过滤(`:676-688`) → `with_tenant_read` + RLS |
| `supersede_entity` | 694-781 | [写] | tx(`:716`)→`begin_tenant_tx`；新版本 INSERT(`:741`) 补列 |

### `db/mm.rs`（MMRepository，当前靠 JSON `content_metadata->>'tenant_id'`）
| 方法 | 行 | 类别 | 处置 |
|---|---|---|---|
| `create_entry` | 126-170 | [写] | INSERT(`:144`) 补物理 `tenant_id` 列；保留 JSON + `scope_prefixed_id` 双写 |
| `get_entry_by_id` | 173-205 | [读] | JSON 过滤(`:192`) → RLS 后可简化 |
| `update_entry` | 208-254 | [写] | 补物理列写 + `begin_tenant_tx`；JSON 过滤(`:233`) 过渡期保留 |
| `get_entries_by_session` | 257-295 | [读] | 依赖 `scope_prefixed_id`(`:263`)，无显式租户列过滤；加 RLS + 审查 |
| `get_entries_by_modality` | 298-339 | [读] | JSON 过滤(`:318`) + RLS |
| `create_relation` | 342-398 | [写] | 补 `modality_relations.tenant_id`(`:374`) + `begin_tenant_tx`；已靠 `get_entry_by_id` 校验 |
| `get_related_entries` | 401-452 | [读] | 经 `get_entry_by_id` → RLS |
| `count` | 455-465 | [admin/读] | **无租户过滤，全局 count** → 请求路径改带租户；系统统计走 admin |
| `list_entries` | 468-568 | [读] | JSON 过滤 + RLS |

---

## 5. 兼容 / 迁移风险与回滚

| # | 风险 | 影响 | 缓解 / 回滚 |
|---|---|---|---|
| R-1 | 漏改某写路径未双写物理列 → M2 后新写入 `tenant_id` NULL 被 CHECK 拒 | 线上写失败 | R1 完成后先跑"写覆盖静态断言"（每个 INSERT 含 `tenant_id`）；M2 前灰度观察 `COUNT(NULL)`；回滚：`DROP CONSTRAINT ...tenant_not_null`（M2 三文件可独立回退） |
| R-2 | 忘设 GUC 的请求路径命中 RLS fail-closed → 返回空 | 功能"查不到数据" | policy 设计即 fail-closed（宁可空不可越权）；R1 让所有请求路径唯一经 `tenant_scope`，静态检查禁止请求 handler 直接 `pool()`；渗透 + 冒烟测试覆盖每类读 |
| R-3 | 会话级 `SET` 泄漏租户（连接池复用） | **跨租户泄漏（红线）** | **只用事务局部 `set_config(_,_,true)`**；code review 禁止任何会话级 `SET aetheris.tenant_id`；Testcontainers 用例专门验证"复用连接不串租户" |
| R-4 | 回填误判租户（前缀畸形 / MVP `user_id==tenant`） | 数据归属错误 | `split_part` 前先 `LIKE 't:%:%'` 兜底；不匹配者一律落 `'__unattributed__'` 而非猜；回填后人工抽样核对；哨兵行 RLS 下对租户不可见，可事后修正 |
| R-5 | 管理任务误用租户执行器 / 应用连接有 BYPASSRLS | 隔离失效或后台任务被 RLS 挡住 | `begin_admin_tx` 与 `begin_tenant_tx` 分离且命名显式；应用角色确认无 BYPASSRLS（§8 待确认）；对账/回填只走 admin |
| R-6 | `migrations_sqlite/` 只有 2 文件、缺基表与 foundation（既有缺陷） | SQLite 路径迁移不完整 | 本轮不扩大范围，但**登记为 backlog**；RLS 不进 sqlite，sqlite 靠应用层 `WHERE tenant_id`；文档标注 sqlite 为 dev-only，多租户强隔离仅 PG 保证 |
| R-7 | `VALIDATE CONSTRAINT` / 大表回填持锁 | 迁移期抖动 | `VALIDATE` 用 `-- no-transaction` + `SHARE UPDATE EXCLUSIVE`（不阻塞读写）；回填分批 + `statement_timeout`；避开高峰窗口 |
| R-8 | 双写期前缀与物理列不一致 | 读写口径漂移 | 过渡期读以 RLS（物理列）为准、前缀为兜底；加一致性对账查询（`split_part(source_id) <> tenant_id` 计数）纳入监控；归零后才 M4 |

**回滚总原则**：M2/M3 每个迁移文件可独立 `DOWN`（`DROP POLICY` / `DISABLE ROW LEVEL SECURITY` / `DROP CONSTRAINT`）。**永不回滚到"纯前缀无物理列"**——物理列一旦回填即为事实源，回退只退强制（RLS/NOT NULL），不退数据。

---

## 6. 离线验证方案（Testcontainers CI gate）

离线无 PG，分两层：

### 6.1 静态断言（无需 DB，扩展现有 `backend/tests/memory_storage_schema_reliability.rs`）
该测试已读迁移 SQL 文本做断言。新增用例：
- M2 三文件存在且含 `NOT VALID` / `VALIDATE CONSTRAINT` / `SET NOT NULL`；`VALIDATE` 文件首行为 `-- no-transaction`。
- M3 文件对 13 张目标表都有 `ENABLE ROW LEVEL SECURITY` + `FORCE ROW LEVEL SECURITY` + policy，且 policy 用 `current_setting('aetheris.tenant_id', true)` 且含 `IS NOT NULL`（fail-closed）与 `WITH CHECK`。
- 写路径静态断言：扫描 `db/{stm,ltm,kg,mm}.rs` 各 INSERT 文本，断言含 `tenant_id` 列（防 R-1 漏改）。
- 反例断言：仓库源码中不得出现会话级 `SET aetheris.tenant_id`（防 R-3），且 `set_config` 第三参恒为 `true`。

### 6.2 集成测试（真 PG，CI gate；`backend/tests/rls_tenant_isolation_pg.rs`，`#[ignore]` 默认跳过）
`Cargo.toml` `[dev-dependencies]` 增（当前无）：
```toml
testcontainers = "0.23"
testcontainers-modules = { version = "0.11", features = ["postgres"] }
```
用例（起真 PG 容器 → 跑全部 `migrations/` → 建两个租户角色/连接）：
1. **正路**：设 `aetheris.tenant_id='A'` 写入 → 读回可见。
2. **拒读**：租户 A 写、设 GUC='B' 读 → 0 行（RLS `USING` 生效）。
3. **拒写**：GUC='A' 但 INSERT `tenant_id='B'` → 被 `WITH CHECK` 拒。
4. **fail-closed**：不设 GUC 查询 → 0 行（而非全部）。
5. **连接复用不串租户**（防 R-3）：同一池连接先 A 事务、后 B 事务，验证 A 的 GUC 不泄漏到 B。
6. **NOT NULL**：INSERT 不带 `tenant_id` → 被拒（M2 生效）。
7. **admin 执行器**：BYPASSRLS 连接可跨租户读（对账路径），且不影响请求路径。
8. **哨兵不可见**：`'__unattributed__'` 行对任何真实租户 GUC 均 0 行。

CI 显式 `cargo test --test rls_tenant_isolation_pg -- --ignored` 作为 P1 放行 gate（qa+devops 侧对应 delivery-plan 风险第 72 行）。

---

## 7. 执行顺序（R1–R5）与门禁

| 步 | 内容 | 前置门禁 | 产出验证 |
|---|---|---|---|
| **R1** | 建 `db/tenant_scope.rs`（4 helper）；仓库签名收敛为 `(executor, TenantId)`；写路径双写物理 `tenant_id` 列；`kg::create_relation` 补租户参数；请求/后台路径分流到 tenant/admin 执行器 | ADR-0001 已有；本计划评审通过 | 离线 `cargo check` 0 error；6.1 写覆盖静态断言全绿；无请求 handler 直连 `pool()` |
| **R2** | M1 回填管理脚本（`split_part` + 哨兵 + 只读隔离登记 + 分批） | R1 双写已上线（新写入不再产 NULL） | 真 PG（Testcontainers 或 staging）跑完 `COUNT(tenant_id IS NULL)=0` |
| **R3** | M2 三段式 NOT NULL（3 迁移文件） | R2 回填完成 | 6.1 M2 静态断言 + 6.2 用例 6 |
| **R4** | M3 启用 RLS + policy（13 表） + 应用/管理连接角色落位 | R3 完成；连接角色确认无 BYPASSRLS | 6.2 用例 1–5/7/8 全绿；渗透测试跨租户被拒 |
| **R5** | 观察期后 M4 收缩（去前缀/`starts_with`）+ 一致性对账归零 | RLS 上线 ≥2 发布周期、零违规 | 对账 `split_part<>tenant_id` 计数为 0；回归测试 |

R1–R4 为 P1 放行必需；R5 为过渡后 backlog。**未过 R1 不得动 M2/M3**（GUC 无载体 = RLS 空转 + NOT NULL 打挂线上）。

---

## 8. Handoff

- **当前阶段**：design-review → handoff-ready（本计划待 tech-lead + architect 评审）
- **目标阶段**：execute（R1）
- **就绪状态**：ready-for-review
- **背景**：delivery-plan P1 首项，把租户隔离从应用层前缀升到 DB 层 RLS 强制。
- **输入依据**：ADR-0001、rls-context-strategy.md、tenant-production-path-inventory.md、transaction-boundaries.md、M0 迁移、`db/{mod,stm,ltm,kg,mm}.rs` 与 `hoops/jwt.rs`、`tenant/context.rs` 源码核对（§1 行号）。
- **结论**：迁移分期与 RLS policy 可直接落；**但 R1 接线（tenant_scope 执行器 + 签名收敛 + 双写物理列）是硬前置**，必须先于任何 enforce 迁移。
- **风险**：见 §5（R-1 漏双写、R-3 GUC 泄漏、R-5 管理连接越权为最高危）。
- **就绪证明（readiness proof）**：静态核对完成（§1 全部行号已验证 @ 2026-07-16 源码）；离线约束下 §6.1 可立即执行、§6.2 依赖 CI 联网起容器。
- **accepted_by**：待 tech-lead / architect 接受。
- **阻塞项**：无硬阻塞；R2+ 依赖一次可连 PG 的环境（staging 或 CI 容器）跑回填与集成测试。

### 待确认项（to-confirm）
1. **GUC 名**：`aetheris.tenant_id`（本计划采用，对齐 rls-context-strategy）vs prompt 原文 `app.tenant_id`。建议前者，请 architect 拍板。
2. **连接角色模型**：应用池用非 owner/非 BYPASSRLS 角色 + 独立管理连接/池 —— 是拆两个池，还是单池 `SET ROLE`？影响 `begin_admin_tx` 实现与部署配置（需 devops）。
3. **哨兵租户策略**：`'__unattributed__'` 历史脏数据的后续处理（人工归属 / 保留只读 / 定期清理）由谁 own。
4. **执行器抽象形态**：仓库方法收 `&mut Transaction` 还是泛型 `impl PgExecutor`（后者读方法更省一次 begin，但与"读也要设 GUC"冲突——设 GUC 必须有事务/连接绑定，故倾向统一 `&mut Transaction`）。

### 下游质疑记录（对上游 rls-context-strategy.md）
- **质疑内容**：rls-context-strategy.md 假设"各仓库可复用统一 helper 设置租户上下文"，但**未指出代码用全局静态池 + 直接 `pool()`、读路径完全无事务载体**——helper 没有可插入的宿主。
- **质疑目标**：上游 rls-context-strategy.md 的接线可行性前提。
- **结论**：要求补充（在本计划内已自行消化，不升级 tech-lead）——把"建 tenant_scope 执行器 + 仓库签名收敛 + 读路径也开短事务"提升为 R1 强制前置。
- **处理说明**：本计划 §3 已给出 `tenant_scope.rs` 四 helper 与签名收敛方案，作为 RLS 落地的物理前提写入执行顺序 R1；上游文档建议同步补一句"依赖 per-request 事务执行器"。
