# P1 实施计划：LTM↔Qdrant outbox+对账 与 治理 hooks 接入

- **日期**：2026-07-16
- **主责角色**：backend-engineer（本计划）/ tech-lead（收口与仲裁）/ architect（跨模块契约）
- **关联入口**：`delivery-plan.md` P1 地基阶段 → 两个 P1 子项的具体实施计划
- **硬约束**：离线无 PG/Qdrant，仅静态核验（工具 codegraph + 直接读源码）
- **依据**：ADR-0002（向量 outbox 与对账）、`../2026-07-06-memory-storage-reliability/` transaction-boundaries.md、`backend/migrations/20260706000100_memory_storage_tenant_foundation.sql`（表已建、src 零引用）

---

## 现状核验（两子项共享，已逐条对源码确认）

1. **生产路由链 = auth → rate_limit → handler**。`routers/mod.rs::root()`：`auth_layer`（55）在 `protected_api_router` 末尾 `.route_layer(auth_layer)`（406，最外层）；`memory_rate_limit`（57）仅挂在 `memory_routes` 末尾 `.route_layer(memory_rate_limit)`（264）。`main.rs:124` 用 `axum_routers::create_router()` → 内部 `routers::root().layer(trace).layer(cors)`，故 `root()` 是真实生产路由。`.route_layer` 语义：**后加 = 外层，执行由外向内**。
2. **`/kg`（319-336）与 `/mm`（337-348）直接 nest 进 `protected_api_router`，自身无 rate_limit 层**（仅被最外层 auth 覆盖）。这影响子项 (b) 的挂载点选择。
3. **outbox/对账/审计四张表已建但 src 零引用**：`memory_vector_outbox`、`memory_vector_reconciliation_runs`、`memory_vector_reconciliation_items`、`memory_audit_events`。
4. **LTM 写路径缺陷**（`memory_storage.rs::store_ltm_for_tenant` 124-277）：LLM 总结（163）→ embedding（181）→ **先 upsert Qdrant（207）→ 后写 DB（221）→ DB 失败才 delete Qdrant 补偿（241）**。两步之间进程崩溃 = 孤儿向量；补偿失败仅 `error!` 日志。
5. **`create_knowledge_entry_with_id`（ltm.rs:88-148）不回填迁移新增的物理 `tenant_id` 列**（只把 `tenant:source_id` 拼进 source_id 列）——对账按 tenant 比对会失效，必须一并修。
6. **Qdrant point id = `numeric_point_id(entry_id)`（DefaultHasher 哈希，qdrant.rs:552）→ 同 entry_id upsert 天然幂等**。但缺 `scroll_point_ids()` / `get_point_payload()`，对账扫 orphan/mismatch 需新增。
7. **治理 hooks 全链零调用**：`enterprise.rs`/`enterprise_impl.rs`（约 2300 行 RBAC/配额/审计框架）仅在 `dashboard.rs` 实例化用于展示；无任何 handler/middleware 调 `pre_store/pre_search/authorize/quota/record_audit`。
8. **审计只打日志**：`enterprise_impl.rs::create_enterprise_hook_set()` 的 `audit_callback` 仅 `tracing::info!`（536-538），不落库。
9. **配额是 no-op**：`tenant/quota.rs::QuotaManager::can_perform`（154）未设配额时恒 true，且**无任何路径 increment `used`**——即便接线也不会真正限流。

---

# 子项 (a)：LTM↔Qdrant outbox + 对账

## 结论先行

把 LTM 写从"**向量先行 + 失败补偿**"改为"**DB 单事务落地（fact + version + outbox 事件）→ 后台 worker 幂等消费 outbox 投递 Qdrant → 周期对账自愈**"。核心四条：

1. **Qdrant 移出写热路径**，写请求只保证 DB 事务原子（含 outbox 事件同事务），响应 `indexStatus: "pending"`。
2. **幂等靠三重保险**：outbox `UNIQUE(tenant_id, idempotency_key)` 去重 + Qdrant point id=hash(entry_id) upsert 幂等 + worker `FOR UPDATE SKIP LOCKED` 认领。
3. **崩溃不再产生部分写/孤儿**：崩溃点无论落在哪，要么 DB 事务整体回滚（无 outbox 即无投递），要么 outbox 已提交由 worker 重放；残留 divergence 由对账兜底。
4. LLM 总结 / embedding 是慢的外部调用，**保留在事务外**（先算好再开事务），事务只做 DB 写。

## 目标写路径（改造 `store_ltm_for_tenant`）

```
1. LLM summarize        （事务外，不变）
2. embedding            （事务外，不变）
3. BEGIN TX:
     a. create_knowledge_entry_tx(&mut tx, ...)   // 新增事务版；同时回填物理 tenant_id 列
     b. insert_version（若现有版本逻辑存在，纳入同事务）
     c. vector_outbox.insert_event_tx(&mut tx, UpsertEvent{
          tenant_id, entry_id, operation="upsert",
          payload_json = {vector, metadata(含 tenantId/title/summary/entities/relations/key_facts), content_hash},
          payload_hash, idempotency_key })
     d.（可选，若同期做子项 b）audit_events.insert_tx(&mut tx, "memory.write", ...)
   COMMIT
4. 返回 { entry_id, indexStatus: "pending" }
5. 后台 worker 异步消费 outbox → Qdrant upsert
6. 周期对账扫描修复 DB↔Qdrant 偏差
```

**幂等键设计**：

- upsert：`format!("upsert:{entry_id}:{payload_hash}")`（内容变更 → 新键 → 新事件；内容不变的重试 → 同键 → UNIQUE 拦截）
- delete：`format!("delete:{entry_id}")`
- `payload_hash` = sha256(规范化后的 vector+metadata)，复用 `information_guard::compute_sha256` 家族。

## 新增文件

| 文件 | 职责 |
|---|---|
| `db/vector_outbox.rs` | outbox 仓库：`insert_event_tx` / `claim_batch` / `mark_applied` / `mark_failed`（带 backoff）/ `mark_dead_letter` / `reclaim_stale` |
| `services/outbox_worker.rs` | worker 生命周期（认领→投递→标记），graceful shutdown |
| `db/vector_reconciliation.rs` | 对账仓库：写 `reconciliation_runs` / `reconciliation_items`，扫 DB 侧游标 |
| `services/vector_reconciliation.rs` | 对账扫描器：diff DB↔Qdrant，按 mode dry_run/repair 决策 |

需在 `services/qdrant.rs` **新增** `scroll_point_ids()`（分页游标扫全量 point id）与 `get_point_payload(id)`（取 payload 比对 tenant/content_hash）。

## Worker 生命周期与启动点

- **启动点**：`main.rs` daemon 区（`information_guard::init_*` 90-91 之后同一批），`if !crate::db::is_sqlite()` 才启（SQLite 见下）。
- **认领事务**（并发安全）：

```sql
SELECT ... FROM memory_vector_outbox
 WHERE status IN ('pending','failed') AND next_retry_at <= now()
 ORDER BY created_at LIMIT $batch
 FOR UPDATE SKIP LOCKED;
-- 同事务 UPDATE status='processing', locked_at=now(), locked_by=$worker_id
```

- **投递在认领事务之外**：认领事务只锁定+标 processing 立即提交（避免长事务持锁跨网络调用），随后对每条调 Qdrant，再用**结果事务**标记：
  - 成功 → `status='applied'`
  - 失败 → `status='failed'`, `attempt_count+1`, `next_retry_at = now() + base*2^attempt_count`（指数退避），`last_error=...`
  - `attempt_count >= MAX` → `status='dead_letter'`（进死信，告警，不再自动重试）
- **崩溃恢复**：`reclaim_stale` 周期把 `status='processing' AND locked_at < now()-stale_threshold` 的行打回 `pending`（worker 崩在投递中途 → 下轮重放；因 upsert 幂等，重复投递无害）。
- **shutdown**：监听现有 shutdown 信号，停止认领、排空在途。

## 对账扫描策略

- **触发**：周期 daemon（低频，如每 N 分钟/小时）+ 可手动触发；每次一条 `reconciliation_runs`（mode=dry_run|repair, status, summary_json）。
- **四类 drift → item.action**（对齐迁移 CHECK）：

| drift_type | 判定 | dry_run | repair |
|---|---|---|---|
| `missing` | DB 有 entry、Qdrant 无 point | report | 补投 upsert outbox 事件 |
| `orphan` | Qdrant 有 point、DB 无 entry | report | delete Qdrant point（需 `scroll_point_ids()`）|
| `tenant_mismatch` | Qdrant payload.tenantId ≠ DB tenant_id | report | rewrite_payload（`set_tenant_payload_for_entries`，qdrant.rs:395 已有）|
| `content_hash_mismatch` | payload.content_hash ≠ DB content_hash | report | 补投 upsert（重写向量+payload）|

- **repair 一律走 outbox**（不直接旁路写 Qdrant），保证所有 Qdrant 变更单一通道、可审计、可重放。

## Read-after-write 一致性

- 响应显式带 `indexStatus: "pending"`；前端/调用方据此提示"索引中"。
- 向量检索存在**最终一致窗口**（worker 消费前查不到刚写的向量）；若产品要求强读己所写，检索层可对最新 N 条**回源 PostgreSQL**（有 content 全文），或接受最终一致并在文档写明窗口。建议先接受最终一致 + 文档说明，回源作为后续增强。

## SQLite 差异

SQLite 是单进程 dev 模式、无并发 worker 意义：`is_sqlite()` 分支**保留现有同步双写**（或同步"写 DB→立即投 Qdrant"），不启 outbox worker。生产（PG）走异步 outbox。此差异在代码注释与文档标注。

## 测试策略（RED-first，离线可跑）

- **纯函数离线单测**：幂等键生成、指数退避计算、drift 分类、payload_hash 规范化——不依赖 PG/Qdrant。
- **集成测试 `#[ignore]` / cfg 门控**：需真 PG+Qdrant（Testcontainers，CI gate），本机离线跳过。
- **故障注入（RED）**：模拟"DB 事务已提交、worker 未消费即崩" → 重启后 worker 应重放至 applied；模拟"投递中途崩" → `reclaim_stale` + 幂等重投无重复。注：离线故障注入的具体手法（进程 kill vs 注入错误）为**待定风险项**，需在有 PG 环境时定方案。

---

# 子项 (b)：企业治理 hooks 接入请求链路

## 结论先行

新增 `hoops/governance.rs::governance_middleware`，插入后链路变为 **auth → rate_limit → governance → handler**；审计从"仅日志"改为"经 mpsc 异步落 `memory_audit_events`"。核心四条：

1. **挂载顺序**：governance 挂在 rate_limit **内侧**（先限流、再做较重的 RBAC/配额，避免为将被限流的请求做无用功）。
2. **审计落库**：`audit_callback` 改为 `mpsc::send` → 后台 writer 批量 INSERT，**不阻塞请求热路径**。
3. **配额要真生效**：补 `used` 的 increment（当前恒不增），否则配额永远拦不住。
4. **单例共享状态**：`EnterpriseHookSet` 走全局 `OnceLock`，保证 QuotaManager/RbacService 跨请求状态连续。

## 接入设计

### 1. 启动装配（main.rs）

在 daemon 区调 `init_enterprise_hooks(create_enterprise_hook_set())`（`enterprise.rs::ENTERPRISE_HOOKS` OnceLock，718/723 已有 init/get）。改造 `create_enterprise_hook_set()` 的 `audit_callback`：从 `tracing::info!` 改为向 `services/audit_writer.rs` 的 mpsc sender 投递 `AuditEvent`。

### 2. 新中间件 `hoops/governance.rs::governance_middleware`

```
- 从 extensions 取 RequestTenantContext + JwtClaims（上游 auth 已注入）
- correlation_id = 生成或透传（写回 extensions + 响应头，串联审计）
- op = classify(method, path)   // 见下表
- ctx = HookContext { tenant_id, user_id, operation: op, resource, params }
- decision = match op {
      MemoryWrite  => hooks.pre_store(&ctx),
      MemorySearch => hooks.pre_search(&ctx),
      Update       => hooks.pre_update(&ctx),
      Delete       => hooks.pre_delete(&ctx),
      _            => Allow,
  }
- match decision {
      Deny(reason) => { hooks.record_audit(deny...); return 403 Forbidden },  // AppError::Forbidden→403
      Allow        => { run handler; 成功后 hooks.record_audit(ok) + 配额 increment },
  }
```

### 3. 挂载点与顺序（关键，已核验路由结构）

- **`memory_routes`**：在现有 `.route_layer(memory_rate_limit)`（264）**之前**加 `.route_layer(governance_layer)`。因"后加=外层"：先加 governance（内）、后加 rate_limit（外）→ 执行 rate_limit→governance→handler；叠加最外层 auth ⇒ **auth→rate_limit→governance→handler** ✓。
- **`/kg`、`/mm`**：这两个 nest **无 rate_limit 层**（现状核验 #2），直接给各自 router 加 `.route_layer(governance_layer)` 即可（执行 auth→governance→handler）。
- 不动 agent/tenants/security 等非记忆操作路由，避免过度拦截。

### 4. `classify(method, path)` → Operation（纯函数，可离线单测）

| method + path | Operation | hook |
|---|---|---|
| POST `/v1/memory/storage/*`（写）| MemoryWrite | pre_store |
| POST/GET `/v1/memory/search/*` | MemorySearch | pre_search |
| POST `/kg/entities`、`/kg/relations` | MemoryWrite | pre_store |
| POST `/mm/store` | MemoryWrite | pre_store |
| GET `/kg/*`、`/mm/*`（读）| MemorySearch | pre_search |
| PUT/PATCH `*` | Update | pre_update |
| DELETE `*` | Delete | pre_delete |

## 审计持久化设计

| 文件 | 职责 |
|---|---|
| `db/audit.rs` | 审计仓库：`insert_event`（写 `memory_audit_events`：event_id, tenant_id, actor_id, event_type, resource_type, resource_id, correlation_id, metadata_json, created_at）+ `insert_tx`（供子项 a 同事务写）|
| `services/audit_writer.rs` | 后台 writer：持 mpsc receiver，批量落库；写失败降级为 `error!` 日志（不反压请求）|

- `audit_callback` 只投递不落库 → 请求热路径零 DB 往返。
- 持久性权衡：mpsc 有界缓冲，溢出时丢弃并计数告警（审计非强一致场景可接受；若要求强一致改为子项 a 式同事务写 audit，二选一由 tech-lead 定）。

## 配额真正生效

- 现状 `QuotaManager` 从不 increment `used`（现状核验 #9）。需在 governance 成功后 increment：`add_api_call` + storage bytes + memory_entries（`QuotaUsage` 88 已有 `add_api_call`/`reset_if_needed` 86400s）。
- **配额基线未定义**：`ResourceQuota` default（storage 1000mb / api_calls 100000 天 / memory_entries 100000）是否合适、按租户/套餐分级——**需产品决策**，列为风险项。

## 共享状态与陷阱

- **必须单例**：`EnterpriseHookSet`/`QuotaManager`/`RbacService` 走全局 OnceLock；若每请求 new 一个，配额/RBAC 状态清零 = 永远不限。
- **disabled 模式**：`jwt.disabled` 时 auth 注入匿名 `RequestTenantContext::new("anonymous")`（jwt.rs:60-68）。需定策略：匿名租户配额如何算 / 或 disabled 模式跳过 governance（建议 dev 跳过、生产强开）。
- **现有 handler 改动面 ≈ 0**：拦截全在 middleware，handler 不改签名；仅依赖 extensions 已有的 tenant context + 新增 correlation_id。

## 测试策略

- `classify()`、`HookDecision` 分支：纯函数离线单测。
- 中间件：用 mock hooks 做 tower 层集成测试（Allow→放行、Deny→403）。
- audit_writer：mpsc→仓库路径，单测用 mock 仓库；真落库走 `#[ignore]` PG 集成。

---

# 协调、顺序与风险

## 推荐实施顺序（两子项有共享件）

1. **(b) 审计基建**：`db/audit.rs` + `services/audit_writer.rs`（被 (a)(b) 共用）
2. **(a) 写事务重构**：`store_ltm_for_tenant` 改单事务 + 新增 `create_knowledge_entry_tx` + 回填物理 `tenant_id` 列
3. **(a) worker + 对账**：`outbox_worker` / `vector_reconciliation` + Qdrant 新方法
4. **(b) 中间件接线 + 配额 increment**（集成风险最高，放最后）

理由：审计基建共享先行；写事务是地基；worker 建于 outbox 之上；中间件涉及全链顺序，最后接。

## 关键风险

| 风险 | 影响 | 缓解 | Owner |
|---|---|---|---|
| Qdrant 缺 `scroll_point_ids()`/`get_point_payload()` | orphan/mismatch 对账做不了 | 先补这两个方法（含分页游标）| backend |
| 配额基线未定义 | 接线后限流值无依据 | tech-lead/product 定默认+分级 | tech-lead |
| 离线故障注入手法待定 | (a) RED 崩溃测试无法本机跑 | 有 PG 环境时定方案（kill vs 错误注入）+ Testcontainers CI | qa |
| SQLite 与 PG 行为差异 | dev 同步、生产异步，语义不一致 | 代码注释 + 文档显式标注；测试分别覆盖 | backend |
| 审计强/弱一致二选一未决 | mpsc 异步可能丢审计 | tech-lead 定：弱一致 mpsc vs 强一致同事务 | tech-lead |
| disabled 模式匿名配额策略未决 | 匿名请求配额行为不明 | 定 dev 跳过/生产强开 | backend |
| 迁移物理 `tenant_id` 列历史数据 NULL | 对账 tenant 比对漏历史行 | 出数据回填脚本（迁移已建列，需 backfill）| architect |

---

**说明**：以上两子项计划均结论先行、务实可执行，落位文件与改动点已对源码逐条核验（离线静态核验，未编译）。可作为 P1 地基阶段 outbox 与治理 hooks 两项工作的实施依据。
