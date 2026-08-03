# Backend Technical Design — Aetheris-MemOS 架构修复 P0–P3

- **日期**：2026-08-03
- **主责角色**：backend-engineer
- **关联 PRD**：`docs/artifacts/2026-08-03-architecture-remediation-full/prd.md`
- **关联 Delivery Plan**：`docs/artifacts/2026-08-03-architecture-remediation-full/delivery-plan.md`
- **关联 ADR**：ADR-0001 ~ ADR-0008
- **应用等级**：T2（RPO ≤ 15min, RTO ≤ 30min）

---

## 结论先行

1. **P0 收口已正确执行**：`routers/mod.rs` 企业路由摘除 + `agent_card.rs` streaming 声明诚实化，均为纯清理操作。
2. **P1 地基基础设施已就位但未接线**：`tenant_scope.rs`（GUC 执行器）、`vector_outbox.rs`（outbox 仓库）、`audit_writer.rs`（异步审计）、`governance.rs`（治理中间件）均已实现核心逻辑，但大部分 repository 尚未从 `pool()` 切换到 `begin_tenant_tx()`。
3. **P2 存在"跳步"风险**：git status 显示 15 个文件中**至少 9 个属于 P2 范围**（gRPC interceptor、WebSocket upgrade handler、A2A handler 接真服务、统一 Authenticator 核心）已提前实现。P1 地基未完成就做 P2 会导致鉴权和租户隔离的接线不完整。
4. **P3 特征管线已起步**：`feature_pipeline.rs` 和 `training_samples` migration 已创建，但 predictor/scheduler 的写死常量替换需等 P1 稳定后才安全。
5. **关键约束**：2 名全职后端 + 6-12 个月，必须严格 P0→P1→P2→P3 串行。

---

## 上游质疑记录

### 质疑 1：接口契约的落地成本是否被低估？

| 字段 | 内容 |
|------|------|
| **质疑内容** | delivery-plan 将 PR-1（tenant_scope Step 1）估为 1-1.5 周、PR-1b（逐 repository 接入）估为 1.5-2 周。但实际需要改造的 repository 数量至少 6 个（`stm.rs`、`ltm.rs`、`kg.rs`、`mm.rs`、`memory_search.rs`、`memory_fusion.rs`），每个 repository 的写入路径（INSERT/UPDATE/DELETE/历史记录）平均 5-8 个方法，且需要同步迁移测试。PR-1b 的 1.5-2 周可能偏乐观。 |
| **质疑目标** | delivery-plan 中 PR-1/PR-1b 的工期估算 |
| **结论** | **接受风险但需监控**——PR-1 纯逻辑部分（`begin_tenant_tx` + 单测）确实 1-1.5 周可完成。PR-1b 的 1.5-2 周假设"每个 repository 改动模式相同"，但 `kg.rs`（Neo4j 双后端）和 `mm.rs`（多模态）复杂度较高，可能需要额外 0.5-1 周。建议在 PR-1b 开始前做一次精确的 repository 方法盘点。 |
| **处理说明** | 已在下方数据模型和接口契约中标注每个 repository 的预期改动面 |

### 质疑 2：数据模型是否过度或不足？

| 字段 | 内容 |
|------|------|
| **质疑内容** | P3 的 `training_samples` 和 `model_versions` 表已通过 migration 创建，但 `config_archetypes` 的 8 个 seed 值（stm-only、stm-ltm、full-stack 等）是静态硬编码的。ADR-0008 要求"候选配置空间"支持按租户 SLA 调整权重，但当前设计没有动态扩展机制。 |
| **质疑目标** | `config_archetypes` 表的扩展性设计 |
| **结论** | **接受原方案**——8 个 archetype 覆盖了常见的记忆层组合，权重可在 JSON 中按租户调整。新增 archetype 只需 INSERT，无需 schema 变更。在 P3 验证阶段（eval harness），固定配置空间是**有意为之**的——ADR-0008 §3.2 明确要求"配置原型集"而非全空间穷举。 |
| **处理说明** | 无需修改；后续如有动态需求，`config_archetypes` 表已预留 `is_active` 字段 |

### 质疑 3：异常路径是否被充分覆盖？

| 字段 | 内容 |
|------|------|
| **质疑内容** | outbox worker 的异常路径设计依赖 `MAX_ATTEMPTS = 8` 和 `STALE_LOCK_SECS = 120`，但未定义 dead-letter 后的告警阈值和人工介入流程。ADR-0002 要求"超过阈值进入 dead-letter 并告警"，但当前实现只有 `warn!` 日志。 |
| **质疑目标** | outbox worker 的 dead-letter 处理和告警闭环 |
| **结论** | **要求补充**——需要在 P1 PR-5 阶段增加：(1) dead-letter 事件的 Prometheus 指标（`outbox_dead_letter_total`）；(2) Grafana 告警规则（dead-letter > 0 持续 5min）；(3) 运维 runbook 中的处理流程。 |
| **处理说明** | 已在下方异常路径章节标注，列入 PR-5 的放行标准 |

---

## 1. Git Status 代码改动分析

### 1.1 改动清单与阶段归属

| 文件 | 改动摘要 | 正确阶段 | 是否"跳步" |
|------|----------|----------|------------|
| `routers/mod.rs` | 摘除 `/enterprise` 路由块（11 行删除 + 注释） | **P0** | ✅ 正确 |
| `a2a/agent_card.rs` | `streaming: true → false`（诚实化声明） | **P0** | ✅ 正确 |
| `hoops/jwt.rs` | 新增 `authenticate()` 核心 + `auth_middleware` 委托 | **P2**（ADR-0007） | ⚠️ 提前实现 |
| `web/jwt.rs` | 标注 deprecated + 迁移指引注释 | **P2**（ADR-0007） | ⚠️ 提前实现 |
| `protocol/grpc.rs` | 新增 `grpc_auth_interceptor`（调用 `authenticate`） | **P2**（ADR-0007） | ⚠️ 提前实现 |
| `protocol/websocket.rs` | 新增 `WsHandlerState`、`ws_upgrade_handler`、租户绑定 | **P2**（ADR-0007） | ⚠️ 提前实现 |
| `a2a/handler.rs` | 5 个 handler 从假数据改为调真 memory 服务 | **P2**（ADR-0007） | ⚠️ 提前实现 |
| `a2a/router.rs` | 路由适配（tenant context 注入） | **P2**（ADR-0007） | ⚠️ 提前实现 |
| `a2a/streaming.rs` | 新增 `Extension<RequestTenantContext>` 参数 | **P2**（ADR-0007） | ⚠️ 提前实现 |
| `services/analyzer.rs` | `calculate_confidence_score` 诚实化（去恒≈1.0） | **P3**（ADR-0008） | ⚠️ 提前实现 |
| `services/monitor.rs` | `response_time_ms` 从 850 改为查 DB | **P3**（ADR-0008） | ⚠️ 提前实现 |
| `db/performance.rs` | 新增 `get_latest_response_time()` | **P3**（ADR-0008） | ⚠️ 提前实现 |
| `Cargo.toml` | axum 启用 `ws` feature | **P2**（ADR-0007） | ⚠️ 提前实现 |
| `services/feature_pipeline.rs` | P3 特征管线（新文件） | **P3**（ADR-0008） | ⚠️ 提前实现 |
| `services/mod.rs` | 注册 `feature_pipeline` 模块 | **P3**（ADR-0008） | ⚠️ 提前实现 |

### 1.2 "跳步"风险评估

**P2 提前实现（9 个文件）——中风险**

- **优点**：统一 Authenticator 核心（`authenticate()`）的设计是正确的，提前实现不影响 P1 逻辑。
- **风险**：(1) P1 RLS 未 enforce 前，A2A handler 接真服务可能绕过租户隔离（`MemorySearchService::search_ltm_for_tenant` 若未走 `begin_tenant_tx`，RLS 不生效）；(2) gRPC interceptor 已实现但无 tonic server 接线，形成死代码。
- **建议**：保留代码但**标记为 `#[cfg(feature = "p2")]` 或 `#[allow(dead_code)]`**，在 P1 RLS enforce 后再接线。当前已标注的 `#![allow(dead_code)]` 在 grpc.rs 和 websocket.rs 上是合理的。

**P3 提前实现（5 个文件）——低风险**

- **优点**：analyzer 和 monitor 的诚实化改动是**纯改进**，不改变 API 形状，降低 P3 集成风险。
- **风险**：`feature_pipeline.rs` 无消费方（`training_samples` 表已创建但无写入路径），形成死代码。
- **建议**：保留 analyzer/monitor 诚实化改动；`feature_pipeline.rs` 和 migration 可保留但需在 P3 正式启动时补齐写入路径和集成测试。

### 1.3 结论

当前 git status 的改动整体方向正确，但存在"P1 未完成就实现 P2/P3"的串行纪律违反。建议：

1. **P0 收口**（2 个文件）立即提交
2. **P3 诚实化**（analyzer + monitor + performance，3 个文件）作为独立 PR 提交——这是无风险改进
3. **P2 相关改动**（9 个文件）暂不提交，等 P1 完成后作为 P2 PR-1 提交
4. **P3 特征管线**（3 个文件）暂不提交，等 P3 正式启动

---

## 2. P0 收口（1-2 天）

### 2.1 接口契约变更

| 变更 | API | 详情 |
|------|-----|------|
| 路由摘除 | `POST /enterprise/cluster/node` | 返回 404 |
| 路由摘除 | `GET /enterprise/cluster/nodes` | 返回 404 |
| 路由摘除 | `GET /enterprise/cluster/active` | 返回 404 |
| 路由摘除 | `GET /enterprise/cluster/leader` | 返回 404 |
| 路由摘除 | `POST /enterprise/cluster/become-leader` | 返回 404 |
| 路由摘除 | `GET /enterprise/cluster/is-leader` | 返回 404 |
| 路由摘除 | `POST /enterprise/shards` | 返回 404 |
| 路由摘除 | `GET /enterprise/shards` | 返回 404 |
| 路由摘除 | `GET /enterprise/shards/{key}` | 返回 404 |
| 声明修正 | agent-card `capabilities.streaming` | `true → false` |

### 2.2 保留项

- `routers/enterprise.rs` 文件保留（P2 做真复用）
- `services/enterprise.rs` 文件保留
- `mod enterprise;` 声明保留

### 2.3 验收标准

- `cargo check` 0 error, 0 warning（除已知 `dead_code`）
- `grep -r "streaming.*true" a2a/agent_card.rs` 无结果
- `curl localhost:8008/enterprise/cluster/nodes` 返回 404

---

## 3. P1 地基 — 技术方案细化

### 3.1 PR-1: tenant_scope 执行器 Step 1（纯逻辑 + 单测）

#### 核心业务逻辑

```rust
// 已实现: backend/src/db/tenant_scope.rs
// begin_tenant_tx(pool, tenant_id) -> Transaction
//   1. pool.begin() 开启事务
//   2. SELECT set_config('aetheris.tenant_id', $1, true) 设置 GUC
//   3. 返回 Transaction（调用方在 tx 上执行查询）
```

**关键算法**：`set_config($1, $2, true)` 的 `is_local = true` 确保 GUC 仅在当前事务内有效，事务结束自动清除，不会泄漏到连接池中的其他请求。

**状态机**：

```
[空闲连接] → begin_tenant_tx → [事务中 + GUC 设置]
    → 执行查询（RLS 生效）
    → commit() → [空闲连接，GUC 已清除]
    → drop(tx) → [空闲连接，事务回滚，GUC 已清除]
```

#### 数据模型

无新增表。依赖已有 RLS policy（`aetheris.tenant_id` GUC）：

```sql
-- 已存在于 migrations/20260716000100_rls_ltm.sql 等
CREATE POLICY {table}_tenant_isolation ON {table}
    USING (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    )
    WITH CHECK (
        current_setting('aetheris.tenant_id', true) IS NOT NULL
        AND tenant_id = current_setting('aetheris.tenant_id', true)
    );
```

#### 测试策略

| 测试 | 类型 | 覆盖场景 |
|------|------|----------|
| `tenant_guc_key_is_namespaced` | 单元 | GUC key 必须含 `.`（已通过） |
| `begin_tenant_tx_sets_guc` | 集成 | 事务内 `current_setting('aetheris.tenant_id')` 返回正确值 |
| `tenant_tx_isolation` | 集成 | 两个并发事务的 GUC 互不干扰 |
| `tenant_tx_rollback_clears_guc` | 集成 | drop(tx) 后 GUC 清除 |
| `rls_rejects_missing_guc` | 集成（负向） | 无 GUC 时查询返回 0 行 |
| `rls_rejects_wrong_tenant` | 集成（负向） | 设置 tenant A 的 GUC 查 tenant B 的数据返回 0 行 |

---

### 3.2 PR-1b: tenant_scope 执行器 Step 2（逐 repository 接入）

#### 改动面盘点

| Repository | 当前路径 | 需改动方法数 | 复杂度 | 备注 |
|------------|----------|-------------|--------|------|
| `db/stm.rs` | `pool()` 直连 | ~6 | 低 | 纯 CRUD |
| `db/ltm.rs` | `pool()` 直连 | ~8 | 中 | 含 history、time-travel |
| `db/kg.rs` | `pool()` + Neo4j | ~6 | 高 | 双后端（PG + Neo4j），Neo4j 无 RLS |
| `db/mm.rs` | `pool()` 直连 | ~5 | 中 | 多模态 |
| `db/vector_outbox.rs` | 已用 `Transaction` | 0 | — | **已就位**，`insert_event_tx` 接受 `&mut Transaction` |
| `db/audit.rs` | 双入口 | 2 | 低 | `insert_tx` 已接受 Transaction；`insert_event` 走全局 pool（审计允许弱一致） |

#### 接口契约变更

每个 repository 方法签名从：

```rust
// Before: 全局 pool，无租户上下文
pub async fn search(&self, query: &str, limit: i32) -> Result<Vec<Entry>, AppError>
```

改为：

```rust
// After: 接受 tenant-scoped executor
pub async fn search(
    &self,
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: &str,
    query: &str,
    limit: i32,
) -> Result<Vec<Entry>, AppError>
```

Service 层调用方改为：

```rust
let mut tx = begin_tenant_tx(pool, &tenant_ctx.tenant_id).await?;
let results = repo.search(&mut *tx, &tenant_ctx.tenant_id, query, limit).await?;
tx.commit().await?;
```

#### 核心业务逻辑：Neo4j 双后端处理

`db/kg.rs` 是特殊 case：PostgreSQL 部分走 RLS + `begin_tenant_tx`；Neo4j 部分**无 RLS**，需要应用层强制 tenant 过滤。ADR-0001 已明确"不替换 PostgreSQL"但未定义 Neo4j 隔离策略。

**方案**：Neo4j 查询一律在 WHERE 子句中附加 `tenant_id` 过滤（与 PostgreSQL 侧的 RLS 兜底形成双重防线）。不依赖 Neo4j 的 APOC 或自定义 security plugin。

```cypher
-- Neo4j 侧：应用层 tenant 过滤
MATCH (n:Entity {tenant_id: $tenant_id})-[r]->(m)
WHERE n.name CONTAINS $query
RETURN n, r, m LIMIT $limit
```

#### 异常路径

| 场景 | 处理 | 降级 |
|------|------|------|
| `set_config` 失败 | 返回 `AppError::Internal` | 请求失败，不影响其他连接 |
| 事务 commit 失败 | 自动回滚，GUC 清除 | 返回 `AppError::Internal`，outbox 事件不写入 |
| Neo4j 连接不可用 | 返回 `AppError::Internal` | KG 查询降级为 PG-only 搜索（已有 fallback 路径） |
| 连接池耗尽 | `pool.begin()` 超时 | 返回 `AppError::Internal`，需监控连接池使用率 |

#### 测试策略

| 测试 | 类型 | 覆盖场景 |
|------|------|----------|
| 每个 repository 方法的正向测试 | 集成 | 使用 `begin_tenant_tx` 查询返回正确数据 |
| 跨租户负向测试（每个 table） | 集成（负向） | tenant A 的数据对 tenant B 不可见 |
| 缺失 GUC 负向测试 | 集成（负向） | 直接用 `pool()` 查询（不设 GUC）返回 0 行 |
| 并发租户隔离 | 集成 | 两个并发请求各自只看到自己的数据 |
| Neo4j tenant 过滤 | 集成（负向） | 跳过 tenant_id 的 Neo4j 查询不返回跨租户数据 |

---

### 3.3 PR-3: RLS Migration（expand → backfill → enforce）+ 查询接线

#### Migration 策略

已有 migration 文件（`20260716000100_rls_ltm.sql` 等）已完成 **backfill + RLS policy 创建**。剩余工作：

| 步骤 | 内容 | 风险 |
|------|------|------|
| **verify** | 确认所有 memory-owned tables 的 `tenant_id` 已 backfill 完成 | 低 |
| **NOT NULL** | 对已 backfill 的表加 `ALTER COLUMN tenant_id SET NOT NULL` | 中——需确认无 NULL 残留 |
| **接线** | 所有生产查询路径切换到 `begin_tenant_tx` | 高——遗漏路径会导致 RLS 拒绝合法查询 |
| **enforce** | 确认 `FORCE ROW LEVEL SECURITY` 已生效 | 低——已在 migration 中 |

#### 数据模型：memory-owned tables RLS 矩阵

| Table | tenant_id 列 | RLS Policy | FORCE RLS | Migration |
|-------|-------------|------------|-----------|-----------|
| `knowledge_entries` | ✅ | ✅ | ✅ | `20260716000100` |
| `knowledge_relations` | ✅ | ✅ | ✅ | `20260716000100` |
| `knowledge_entry_versions` | ✅ | ✅ | ✅ | `20260716000100` |
| `context_sessions` | ✅ | ✅ | ✅ | `20260716000200` |
| `context_messages` | ✅ | ✅ | ✅ | `20260716000200` |
| `session_messages` | ✅ | ✅ | ✅ | `20260716000200` |
| `knowledge_entities` | ✅ | ✅ | ✅ | `20260716000300` |
| `multimodal_entries` | ✅ | ✅ | ✅ | `20260716000400` |
| `memory_vector_outbox` | ✅ | ✅ | ✅ | `20260706000100` |
| `memory_audit_events` | ✅ | — | — | `20260706000100`（审计允许跨租户管理查询） |
| `training_samples` | ✅ | ✅ | ✅ | `20260803000100` |
| `model_versions` | — | — | — | 非租户隔离（全局模型注册表） |
| `config_archetypes` | — | — | — | 非租户隔离（全局配置原型） |

#### 接口契约变更

**无 API 路径/请求/响应结构变更**。变更是内部实现：所有 memory 查询从 `sqlx::query(pool)` 切换到 `sqlx::query(&mut *tx)`，其中 `tx` 来自 `begin_tenant_tx`。

#### NOT NULL Migration 模板

```sql
-- P1 PR-3: Enforce NOT NULL on tenant_id for {table}.
-- PRECONDITION: backfill 完成，无 NULL 残留。
-- STRATEGY: 先验证 NULL 计数，再 ALTER。

DO $$
DECLARE
    null_count BIGINT;
BEGIN
    SELECT COUNT(*) INTO null_count FROM {table} WHERE tenant_id IS NULL;
    IF null_count > 0 THEN
        RAISE EXCEPTION 'Cannot enforce NOT NULL: % rows with NULL tenant_id', null_count;
    END IF;
    ALTER TABLE {table} ALTER COLUMN tenant_id SET NOT NULL;
END $$;
```

#### 异常路径

| 场景 | 处理 | 回滚 |
|------|------|------|
| NOT NULL 验证发现残留 NULL | **阻塞 migration**，需人工排查并 backfill | 不执行 ALTER |
| RLS 导致合法查询返回 0 行 | 检查 `begin_tenant_tx` 是否被调用 | 回退到应用层 TenantId 过滤（中间态） |
| 生产查询路径遗漏 | 立即修复 + 回归测试 | RLS 是兜底，不会泄露数据，只会拒绝 |

---

### 3.4 PR-5: Outbox Worker + 对账

#### 核心业务逻辑

**已有实现**（`services/outbox_worker.rs`）：

```
[请求写入 LTM] → 事务内 INSERT outbox event → commit
    ↓ (异步)
[outbox worker] → SELECT ... FOR UPDATE SKIP LOCKED → 取 batch
    → 对每个 event:
        - upsert: 调用 qdrant.upsert_point(payload)
        - delete: 调用 qdrant.delete_point(entry_id)
    → 成功: UPDATE status = 'applied'
    → 失败: UPDATE status = 'failed', attempt_count++, next_retry_at = now + backoff
    → 超过 MAX_ATTEMPTS(8): status = 'dead_letter'
[reclaim_stale] → 每 15 轮回收超时锁(120s)
```

**状态机**：

```
pending → (claim) → processing → (success) → applied
                                → (fail, attempts < 8) → failed → (retry) → pending
                                → (fail, attempts >= 8) → dead_letter
stale_processing → (reclaim after 120s) → pending
```

#### 数据模型

已有表（`memory_vector_outbox`）：

```sql
CREATE TABLE memory_vector_outbox (
    event_id        TEXT PRIMARY KEY,
    tenant_id       TEXT NOT NULL,
    entry_id        TEXT NOT NULL,
    operation       TEXT NOT NULL CHECK (operation IN ('upsert', 'delete')),
    payload_json    TEXT NOT NULL,
    payload_hash    TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending','processing','applied','failed','dead_letter')),
    attempt_count   INT NOT NULL DEFAULT 0,
    next_retry_at   TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    locked_by       TEXT,
    locked_at       TIMESTAMPTZ,
    last_error      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (tenant_id, idempotency_key)
);
```

**需新增索引**（对账查询性能）：

```sql
-- 对账: 比较 PG active entries vs Qdrant points
CREATE INDEX IF NOT EXISTS idx_outbox_status_tenant
    ON memory_vector_outbox (status, tenant_id);

-- 死信告警查询
CREATE INDEX IF NOT EXISTS idx_outbox_dead_letter
    ON memory_vector_outbox (created_at) WHERE status = 'dead_letter';
```

#### 接口契约变更

**无 API 变更**。outbox worker 是后台进程，不暴露 HTTP 端点。

新增 Prometheus 指标：

| 指标名 | 类型 | 含义 |
|--------|------|------|
| `outbox_pending_total` | Gauge | 待处理事件数 |
| `outbox_dead_letter_total` | Counter | 死信事件累计数 |
| `outbox_processing_duration_seconds` | Histogram | 单批处理耗时 |
| `outbox_qdrant_upsert_success_total` | Counter | Qdrant upsert 成功数 |
| `outbox_qdrant_upsert_failure_total` | Counter | Qdrant upsert 失败数 |

#### 异常路径

| 场景 | 处理 | 降级 |
|------|------|------|
| Qdrant 不可达 | `attempt_count++`, backoff 重试 | PG 事实写入不受影响，搜索延迟增加 |
| Qdrant 返回非幂等错误 | 记录 `last_error`，标记 `dead_letter` | 需人工介入 |
| Worker 进程崩溃 | `reclaim_stale` 回收锁（120s 后） | 其他 worker 实例接管 |
| Outbox backlog 增长 | 告警（backlog > 1000 持续 5min） | 检查 Qdrant 连接或增加 worker 并发 |
| Dead letter 堆积 | 告警（dead_letter > 0 持续 5min） | 人工检查后可手动重置 status 重试 |

#### 对账 Job

```rust
// 定期（每日/每周）比较 PG active entries vs Qdrant points
pub async fn reconcile(tenant_id: &str) -> ReconciliationReport {
    // 1. 从 PG 取 active LTM entries (通过 begin_tenant_tx)
    // 2. 从 Qdrant 取 tenant filter 的 points
    // 3. 比较:
    //    - missing_in_qdrant: PG 有但 Qdrant 无 → 触发 outbox upsert
    //    - orphan_in_qdrant: Qdrant 有但 PG 无 → 触发 outbox delete
    //    - hash_mismatch: content_hash 不一致 → 触发 outbox upsert
    // 4. 生成 repair report
}
```

---

### 3.5 PR-2: 审计基建（并行）

#### 核心业务逻辑

**已有实现**（`services/audit_writer.rs` + `db/audit.rs`）：

```
[请求处理] → governance hook 记录审计事件
    → record_audit(event) → mpsc::try_send（非阻塞）
    ↓ (后台)
[audit_writer_worker] → mpsc::recv batch (最多 256 条, 50ms 窗口)
    → INSERT INTO memory_audit_events (批量)
    → 失败: error! 日志 + DROPPED 计数
```

**一致性模型**：弱一致 / 尽力而为。关键审计（如 RBAC 拒绝）使用 `insert_tx` 写入同一事务保证原子性。

#### 数据模型

已有表（`memory_audit_events`）：

```sql
CREATE TABLE memory_audit_events (
    event_id        TEXT PRIMARY KEY,
    tenant_id       TEXT,
    actor_id        TEXT,
    event_type      TEXT NOT NULL,
    resource_type   TEXT NOT NULL,
    resource_id     TEXT,
    correlation_id  TEXT,
    metadata_json   TEXT NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

#### 接口契约变更

无 API 变更。审计通过 governance middleware 自动附加到 memory 操作。

---

### 3.6 PR-6: 治理 Hooks 接入请求中间件（并行）

#### 核心业务逻辑

**已有实现**（`hoops/governance.rs`）：

```
[HTTP 请求] → auth_middleware（注入 RequestTenantContext）
    → governance_middleware:
        1. classify(method, path) → Operation（Search/Store/Update/Delete/None）
        2. 构建 HookContext{tenant_id, user_id, operation, resource}
        3. EnterpriseHookSet.pre_{store|search|update|delete}(ctx)
        4. Deny → 403 Forbidden + 审计
        5. Allow → 继续
```

#### 接口契约

| 场景 | HTTP 状态码 | 响应体 |
|------|------------|--------|
| Quota 超限 | `403 Forbidden` | `{"error": "Quota exceeded: {current}/{limit}"}` |
| RBAC 权限不足 | `403 Forbidden` | `{"error": "Insufficient role permissions"}` |
| 企业 hooks 未初始化 | 继续（fail-open） | — |

#### 测试策略

| 测试 | 类型 | 覆盖场景 |
|------|------|----------|
| quota_exceeded_returns_403 | 集成（负向） | 超过配额的 store 操作返回 403 |
| rbac_denied_returns_403 | 集成（负向） | 无写权限的用户尝试 store 返回 403 |
| unclassified_path_passes_through | 单元 | 非 memory 路径不受治理中间件影响 |
| governance_fail_open | 集成 | hooks 未初始化时请求正常通过 |
| audit_recorded_on_deny | 集成 | 拒绝操作产生审计记录 |

---

### 3.7 PR-7: MCP Plane A（call_tool 验签 + capability 授权）

#### 核心业务逻辑

**当前问题**：`list_tools` 验签但 `call_tool` 不验签（ADR-0004 §2）。

**改造方案**：

```rust
// routers/mcp.rs::call_tool 改造
pub async fn call_tool(...) -> Result<Json<Value>, AppError> {
    let tenant_ctx = extensions.get::<RequestTenantContext>().unwrap();

    // 1. 验签：复用 signing.rs 的 verify_component
    let signatures = load_tool_signatures();
    verify_component(&tool_name, &signatures)
        .map_err(|_| AppError::Forbidden(format!("Tool '{}' signature verification failed", tool_name)))?;

    // 2. Capability 授权
    let capability = tool_capability(&tool_name)
        .ok_or_else(|| AppError::Forbidden(format!("Unknown tool '{}'", tool_name)))?;
    check_capability(&tenant_ctx, capability)?;

    // 3. 审计
    record_audit(AuditEvent::new("mcp.call_tool", "mcp_tool")
        .tenant(&tenant_ctx.tenant_id)
        .actor(&tenant_ctx.user_id)
        .resource_id(&tool_name)
        .with_metadata(&json!({"capability": capability, "result": "allow"})));

    // 4. 执行（复用现有逻辑）
    match tool_name.as_str() {
        "memory_write" => handle_memory_write(...),
        "memory_search" => handle_memory_search(...),
        // ...
    }
}
```

#### 工具 Capability 映射

| 工具名 | Capability | Operation |
|--------|-----------|-----------|
| `memory_write` | `memory:write` | Store |
| `memory_search` | `memory:read` | Search |
| `memory_recall` | `memory:read` | Search |
| `memory_forget` | `memory:delete` | Delete |
| `memory_list` | `memory:read` | Search |

#### 接口契约变更

| 场景 | 变更前 | 变更后 |
|------|--------|--------|
| 有效签名 + 有权限 | 200 OK | 200 OK（不变） |
| 无效签名 | 200 OK（未验签） | **403 Forbidden** |
| 有签名无权限 | 200 OK（未授权） | **403 Forbidden** |
| 未知工具名 | 404 Not Found | 403 Forbidden（安全考虑不暴露工具列表） |

#### 灰度策略

```
Phase 1 (第1周): 验签失败 → warn! 日志 + 继续执行（告警不拒绝）
Phase 2 (第2周): 验签失败 → 403 Forbidden（强制拒绝）
```

通过配置项 `mcp.enforce_signature` 控制，默认 `false`（Phase 1）。

---

### 3.8 P1 放行标准验收矩阵

| 放行标准 | 验证方法 | 通过条件 |
|----------|----------|----------|
| 跨租户负向测试通过 | 对每个 memory-owned table 执行跨租户查询 | 全部返回 0 行 |
| MCP call_tool 验签 + 越权拒绝 | 发送未签名/越权的 call_tool 请求 | 返回 403 |
| 备份/恢复演练 | PG PITR + Qdrant snapshot restore | 数据完整、应用正常 |
| 治理 hooks 接线验证 | 超配额 store 操作 | 返回 403 + 审计记录 |
| Outbox worker 正常运行 | 写入 LTM → 检查 Qdrant point | applied 状态、payload 正确 |
| Dead letter 告警 | 模拟 Qdrant 不可达 | dead_letter > 0 触发告警 |

---

## 4. P2 多协议 — 技术方案细化

### 4.1 统一 Authenticator 核心

#### 接口契约

```rust
/// 传输无关鉴权核心（ADR-0007）
/// 输入: raw JWT token string
/// 输出: (JwtClaims, RequestTenantContext) 或 AppError::Unauthorized
pub fn authenticate(token: &str) -> Result<(JwtClaims, RequestTenantContext), AppError>
```

**已实现**（`hoops/jwt.rs`），包含：HS256 解码、过期校验、租户上下文构造。

#### 四传输适配器

| 传输 | Token 来源 | 适配器 | 状态 |
|------|-----------|--------|------|
| REST/MCP | Cookie `jwt_token` 或 `Authorization: Bearer` | `auth_middleware`（委托 `authenticate`） | ✅ 已实现 |
| gRPC | `authorization: Bearer <jwt>` metadata | `grpc_auth_interceptor` | ⚠️ 已实现但未接线（无 tonic server） |
| WebSocket | HTTP 握手 headers | `ws_upgrade_handler`（手动提取） | ⚠️ 已实现但 `send_to_session` 仍为占位 |
| A2A | HTTP middleware（复用 REST） | axum `auth_middleware` | ⚠️ handler 已接真服务但 A2A feature 默认关闭 |

### 4.2 gRPC（tonic 真 server）

#### 接口契约

```protobuf
// proto/memory_service.proto
syntax = "proto3";
package aetheris.memory;

service MemoryService {
    rpc SearchLtm(SearchRequest) returns (SearchResponse);
    rpc StoreLtm(StoreRequest) returns (StoreResponse);
    rpc ListSessions(ListSessionsRequest) returns (ListSessionsResponse);
    rpc GetKgEntities(GetKgEntitiesRequest) returns (GetKgEntitiesResponse);
    rpc HealthCheck(Empty) returns (HealthResponse);
}

message SearchRequest {
    string query = 1;
    int32 limit = 2;
    optional double min_score = 3;
}

message SearchResponse {
    repeated SearchResult results = 1;
    int32 total = 2;
}

message SearchResult {
    string entry_id = 1;
    string content = 2;
    double score = 3;
    string tenant_id = 4;
}
```

#### 核心业务逻辑

gRPC handler **委托 REST 同款 memory 服务**，不另写业务逻辑：

```rust
impl MemoryService for MemoryServiceImpl {
    async fn search_ltm(&self, req: Request<SearchRequest>) -> Result<Response<SearchResponse>, Status> {
        let tenant_ctx = req.extensions().get::<RequestTenantContext>()
            .ok_or_else(|| Status::unauthenticated("Missing tenant context"))?;

        let results = MemorySearchService::search_ltm_for_tenant(
            &tenant_ctx.tenant_id, &inner.query, inner.limit, None, None
        ).await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(to_proto_response(results)))
    }
}
```

#### 部署

- 同进程独立 HTTP/2 端口（默认 `0.0.0.0:8009`）
- TLS 复用现有 rustls 配置
- 优雅停机：`tokio::select!` 监听 SIGTERM

### 4.3 WebSocket（axum 升级）

#### 接口契约

```
GET /ws/events
  Upgrade: websocket
  Authorization: Bearer <jwt>
  → 101 Switching Protocols (auth 成功)
  → 401 Unauthorized (auth 失败)
```

**消息格式**：

```json
// 客户端 → 服务端
{"type": "subscribe", "channels": ["memory.updates", "kg.changes"]}
{"type": "unsubscribe", "channels": ["memory.updates"]}

// 服务端 → 客户端
{"type": "event", "channel": "memory.updates", "data": {"entry_id": "...", "action": "created"}}
{"type": "heartbeat", "timestamp": 1691234567}
```

#### 核心业务逻辑

- 握手时调用 `jwt::authenticate()` 验证 token
- `WsConnection` 绑定 `RequestTenantContext`
- 连接最大 TTL：24 小时（到期要求重连，防 token 过期绕过）
- 心跳：每 30 秒 ping/pong
- 订阅过滤：按 tenant_id 过滤推送事件

#### 异常路径

| 场景 | 处理 |
|------|------|
| 握手 token 无效 | 拒绝升级，返回 close frame |
| 连接中 token 过期 | TTL 到期后主动关闭连接 |
| 背压（客户端消费慢） | 广播 channel 自动丢弃旧消息 |
| 跨租户推送 | 订阅列表按 tenant_id 过滤，绝不推送其他租户数据 |

### 4.4 A2A（a2a-rs 接真实）

#### 接口契约

保持现有 A2A 协议形状（`SendMessageRequest` / `SendMessageResponse`），但：

| Skill | 改造前 | 改造后 |
|-------|--------|--------|
| `memory_search` | `format!("Memory search completed for query: '{}'", text)` | 调用 `MemorySearchService::search_ltm_for_tenant` |
| `memory_store` | 假数据响应 | 调用 `MemoryStorageService::store_ltm` |
| `memory_fusion` | 假数据响应 | 调用 `MemoryFusionService::query_ltm` |
| `memory_status` | `stm_count:0, overall_healthy:true` 硬编码 | 查询真实 STM/LTM/KG/MM 计数 |
| `knowledge_graph` | 假数据响应 | 调用 `KGRepository` |

#### Agent 身份映射

```
A2A caller agent → user_id: "a2a:{agent_id}" → tenant_id: 从 JWT 解析
```

Agent-card 能力声明必须与实测一致：`streaming: false`（P0 已修正）。

### 4.5 P2 测试策略

| 测试 | 类型 | 覆盖场景 |
|------|------|----------|
| 四协议鉴权负向（缺 token） | 集成 | 每个协议缺 token 返回 unauthenticated |
| 四协议鉴权负向（过期 token） | 集成 | 过期 token 返回 unauthenticated |
| 四协议鉴权负向（无效签名） | 集成 | 伪造 token 返回 unauthenticated |
| 跨租户隔离（gRPC） | 集成 | tenant A 的 gRPC 请求看不到 tenant B 数据 |
| 跨租户隔离（WS） | 集成 | tenant A 的 WS 推送不包含 tenant B 事件 |
| A2A 端到端（真服务） | 集成 | A2A memory_search 返回真实搜索结果 |
| Agent-card 一致性 | 集成 | agent-card 声明与实测能力匹配 |
| 双 JWT 消除验证 | 集成 | `web::jwt` 不再被任何生产路径调用 |

---

## 5. P3 自适应 — 技术方案细化

### 5.1 P3a: 特征管线

#### 数据模型

**已创建 migration**（`20260803000100_p3_training_samples.sql`）：

```sql
training_samples (
    sample_id     TEXT PRIMARY KEY,
    tenant_id     TEXT NOT NULL,          -- RLS 隔离
    task_id       TEXT NOT NULL,
    config_id     TEXT NOT NULL,
    features_json TEXT NOT NULL DEFAULT '{}',  -- 13 维特征向量
    labels_json   TEXT NOT NULL DEFAULT '{}',  -- 5 维标签向量
    policy_tag    TEXT NOT NULL DEFAULT 'normal' CHECK (policy_tag IN ('normal', 'exploration')),
    split_tag     TEXT CHECK (split_tag IN ('train', 'val', 'test', NULL)),
    collected_at  TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
)

model_versions (
    version        TEXT PRIMARY KEY,
    model_type     TEXT NOT NULL CHECK (model_type IN ('linear', 'gbdt', 'ensemble')),
    artifact_uri   TEXT NOT NULL,
    metrics_json   TEXT NOT NULL DEFAULT '{}',
    feature_names  TEXT NOT NULL DEFAULT '[]',
    trained_at     TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status         TEXT NOT NULL DEFAULT 'shadow' CHECK (status IN ('shadow', 'canary', 'active', 'rolled_back')),
    promoted_at    TIMESTAMPTZ,
    rolled_back_at TIMESTAMPTZ,
    notes          TEXT
)

config_archetypes (
    archetype_id TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    description  TEXT,
    config_json  TEXT NOT NULL DEFAULT '{}',
    is_active    BOOLEAN NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
)
```

#### 接口契约

**新增内部 API**（非公开，仅供训练管线调用）：

```
POST /api/v1/internal/training/samples
  Body: { task_id, config_id, features, labels, policy_tag }
  → 201 Created { sample_id }

GET /api/v1/internal/training/samples?split_tag=train&limit=1000
  → 200 OK { samples: [...], total }

POST /api/v1/internal/models/register
  Body: { version, model_type, artifact_uri, metrics_json, feature_names }
  → 201 Created { version, status: "shadow" }

POST /api/v1/internal/models/{version}/promote
  → 200 OK { version, status: "active", previous_active: "..." }

POST /api/v1/internal/models/{version}/rollback
  → 200 OK { version, status: "rolled_back" }
```

#### 核心业务逻辑：特征提取

```rust
// 已实现: services/feature_pipeline.rs
pub fn extract_features(
    characteristics: &TaskCharacteristics,
    resource_status: &ResourceStatus,
    config: &MemoryConfig,
    response_time_ms: Option<u64>,
) -> FeatureVector
```

13 维特征：complexity、modality_count、temporal_scope、reasoning_depth、context_dependency、resource_cpu、resource_memory、response_time_ms、config_stm_weight、config_ltm_weight、config_kg_weight、config_mm_weight、config_multimodal。

#### 探索机制（ε-配置分配）

```rust
/// ε-greedy 配置分配：以概率 ε 随机选配置，1-ε 选当前最优配置。
/// exploration 样本标记 policy_tag = 'exploration'。
pub fn select_config(
    epsilon: f64,  // 建议 0.05-0.1
    archetypes: &[ConfigArchetype],
    predictor: &dyn Predictor,
    features: &FeatureVector,
) -> (ConfigArchetype, String /* policy_tag */) {
    if rand::random::<f64>() < epsilon {
        let idx = rand::random::<usize>() % archetypes.len();
        (archetypes[idx].clone(), "exploration".to_string())
    } else {
        // 选预测效用最高的配置
        let best = archetypes.iter()
            .max_by(|a, b| predictor.predict_utility(a, features)
                .partial_cmp(&predictor.predict_utility(b, features))
                .unwrap())
            .unwrap();
        (best.clone(), "normal".to_string())
    }
}
```

### 5.2 P3b: 离线批训练

#### 训练流程

```
1. 读取 training_samples（split_tag = 'train'）
2. 切分验证集（split_tag = 'val'）
3. 拟合模型:
   - 线性/GLM: sklearn LinearRegression / Rust linfa
   - GBDT: LightGBM / Rust xgboost-rs
4. 在 val 集评估: R², MAE, win-rate
5. 生成 model card (metrics_json)
6. 写入 model_versions (status = 'shadow')
```

#### 模型加载

```rust
// predictor.rs 改造
pub struct AdaptivePredictor {
    model: Option<Box<dyn Model>>,  // 从 model_versions 加载
    fallback: StaticPredictor,      // 无模型时的静态最优
}

impl AdaptivePredictor {
    pub fn predict(&self, features: &FeatureVector) -> Prediction {
        match &self.model {
            Some(m) => {
                let pred = m.predict(features);
                let ci = m.conformal_interval(features);  // 校准置信区间
                Prediction {
                    efficiency: pred[0],
                    coherence: pred[1],
                    latency: pred[2],
                    cost: pred[3],
                    confidence_interval: ci,
                    model_version: m.version().to_string(),
                }
            }
            None => self.fallback.predict(features),  // 诚实降级
        }
    }
}
```

### 5.3 P3c: Scheduler 候选选优 + Eval Harness

#### 候选选优

```rust
// scheduler.rs 改造
pub fn adaptive_memory_selection(
    &self,
    features: &FeatureVector,
    archetypes: &[ConfigArchetype],
    predictor: &AdaptivePredictor,
    constraints: &ResourceConstraints,
) -> MemoryConfig {
    // 对每个候选拍品预测效用
    let best = archetypes.iter()
        .filter(|a| satisfies_constraints(a, constraints))
        .max_by(|a, b| {
            let util_a = utility(predictor.predict(&augment_features(features, a)));
            let util_b = utility(predictor.predict(&augment_features(features, b)));
            util_a.partial_cmp(&util_b).unwrap()
        })
        .unwrap();

    archetype_to_config(best)
}

/// 效用函数: utility = w_q·quality - w_l·norm(latency) - w_c·cost
fn utility(pred: &Prediction) -> f64 {
    let w_q = 0.6;
    let w_l = 0.25;
    let w_c = 0.15;
    w_q * pred.efficiency - w_l * normalize(pred.latency) - w_c * pred.cost
}
```

#### Eval Harness

```rust
/// eval 结果报告
pub struct EvalReport {
    pub primary_metric: String,           // "quality_win_rate"
    pub adaptive_win_rate: f64,           // 自适应 > 静态最优的任务占比
    pub median_improvement: f64,          // 中位提升
    pub ci_95_lower: f64,                 // bootstrap 95% CI 下界
    pub ci_95_upper: f64,                 // bootstrap 95% CI 上界
    pub p_value: f64,                     // Wilcoxon signed-rank p-value
    pub effect_size: f64,                 // Cohen's d
    pub n_tasks: usize,                   // 基准任务数
    pub leakage_audit_passed: bool,       // 数据泄漏检查
    pub stratified_results: Vec<StratumResult>,  // 分层结果
    pub verdict: EvalVerdict,             // PASS / FAIL / INCONCLUSIVE
}
```

**放行判定**：`win_rate CI lower > 0.5` AND `median_improvement >= MDE` AND `leakage_audit_passed == true` AND `p_value < 0.05`。

**诚实降级**：任一条件不满足 → predictor 回退为静态最优 + 清理"自适应"文案。

### 5.4 异常路径

| 场景 | 处理 | 降级 |
|------|------|------|
| 训练样本不足（< 功效所需 n） | 不启动训练 | predictor 使用静态最优 |
| 模型上线后质量退化 | shadow/canary 检测 → 自动回滚 | 回退到上一 active 版本 |
| OTel 数据延迟/缺失 | 特征用默认值（0）标注 | 样本标记为 incomplete |
| 探索影响用户体验 | ε 设小（0.05）+ 按租户可关 | 探索样本不参与 SLA 归因 |
| 校准退化（预测偏离实测） | 监控校准曲线 → 告警 | 触发重训或降级 |

---

## 6. 风险与约束汇总

| 风险 | 影响阶段 | 严重度 | 缓解措施 |
|------|----------|--------|----------|
| PR-1b 工期偏乐观 | P1 | 中 | 开始前做 repository 方法精确盘点 |
| P2 跳步导致 RLS 未 enforce | P1/P2 | 高 | P2 代码保留但不接线，P1 完成后再激活 |
| Neo4j 无 RLS 机制 | P1 | 中 | 应用层强制 tenant 过滤 + 双重防线 |
| a2a-rs git 依赖离线不可拉 | P2 | 中 | 联网一次 pin rev + 提交 lock |
| P3 eval 无法证明增益 | P3 | 高 | 诚实降级路径（ADR-0008 §5） |
| 人力不足（2 后端） | 全程 | 高 | 严格串行 P0→P1→P2→P3；P3a 可并行 |
| Dead letter 无告警 | P1 | 中 | PR-5 增加 Prometheus 指标 + Grafana 告警 |
| 双 JWT 消除后遗留调用 | P2 | 低 | `web::jwt` 标注 deprecated + grep 验证 |

---

## 7. 待确认项

| 编号 | 事项 | 建议 | 需要谁确认 |
|------|------|------|------------|
| T1 | P2 代码是否暂存分支还是保留 main | 暂存分支，P1 完成后合入 | tech-lead |
| T2 | MCP Plane B 是否进 P1 | 不进（当前无不可信工具） | tech-lead |
| T3 | P3 是否纳入本轮 | P3-lite（配置推荐引擎）先行 | tech-lead + 业务方 |
| C1 | 应用等级 T2 定档确认 | RPO ≤ 15min, RTO ≤ 30min | tech-lead |
| C2 | 部署目标确认 | 私有云/K8s | tech-lead + devops |
| C5 | 集团统一 IAM/OIDC 是否可用 | 需评估 | architect + devops |

---

## 8. 下一步交接

**下一跳角色**：`qa-engineer`（P1 测试计划）、`devops-engineer`（HA 基建）

**交接内容**：
1. 本文档作为后端技术方案细化
2. P1 PR 依赖拓扑：PR-1 → PR-1b → PR-3 → PR-5（关键路径）；PR-2 || PR-6 || PR-7（并行）
3. 每个 PR 的接口契约、数据模型、异常路径和测试策略
4. Git status 跳步风险评估和处理建议
5. 待确认项 T1-T3、C1-C5

**就绪状态**：`handoff-ready`（待 tech-lead 确认 T1-T3 后可进入 `/team-execute`）
