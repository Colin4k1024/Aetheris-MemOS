# Tenant Production Path Inventory: Memory Storage Reliability

## 结论

当前 memory storage 生产路径已经有 request-scoped tenant 与 prefix 隔离基础，但仍存在多处依赖 `t:{tenant}` prefix、body/query tenant 或无 tenant 参数的方法。P0 改造必须将生产路径统一收敛到显式 `TenantId` + `tenant_id` schema + transaction-local RLS context。

本盘点用于支撑 P0-S2：TenantId production-path inventory。

## 改造原则

- 生产路径不再依赖 `source_id LIKE 't:%'`、`user_id LIKE 't:%'`、`entity_id LIKE 't:%'` 作为安全边界。
- update、delete、history、time-travel、relation mutation 必须显式接收 `TenantId`。
- router 只能从 `RequestTenantContext` 获取 tenant，不信任 body/query 中的 tenant 字段。
- legacy prefix helper 仅保留给 backfill、兼容读和审计告警。
- 后台任务必须按 tenant 显式枚举，不允许全局 user/session 扫描后再过滤。

## 高风险路径清单

| 区域 | 路径 | 当前问题 | P0 目标 |
|------|------|----------|---------|
| STM | `backend/src/db/stm.rs::create_session` | 写入 `user_id` prefix，未写物理 `tenant_id` | 写入 `tenant_id`，保留 prefix 仅兼容 |
| STM | `STMRepository::get_session` | 先按 `session_id` 查，再 prefix 校验 | SQL 直接使用 `tenant_id = $tenant_id AND session_id = $session_id` |
| STM | `STMRepository::add_message` | session 校验、message insert、context update 分离 | 同一 tenant-scoped transaction，写 `session_messages.tenant_id` |
| STM | `STMRepository::get_session_messages` | 先查 session，再按 session_id 查 messages | messages 查询绑定 `tenant_id`，避免 session_id 全局碰撞 |
| STM | `STMRepository::get_recent_sessions` | 使用 `(user_id = $1 OR user_id LIKE $2)` | 改为 `tenant_id = $tenant_id` + user/agent 条件 |
| STM | `STMRepository::list_sessions` | 依赖 `user_id LIKE tenant_prefix` | 改为 tenant_id 查询 |
| STM | `STMRepository::get_active_user_ids` | 全局枚举 active users | 改为 `get_active_user_ids_for_tenant(tenant_id)`，后台任务显式枚举 tenant |
| STM | `STMRepository::delete_session` | 删除 messages 与 session 分离，按 session_id 删除 | 同一 transaction，绑定 tenant_id |
| LTM | `LTMRepository::create_knowledge_entry_with_id` | 写 prefix source_id，未写物理 tenant_id | 写 `tenant_id`，source_id prefix 仅兼容 |
| LTM | `LTMRepository::update_entry` | 无 TenantId，仅按 entry_id update | 改为 `update_entry_for_tenant(tenant_id, entry_id, patch)` |
| LTM | `LTMRepository::soft_delete_entry` | 先 get 校验，再 update 仅按 entry_id | update SQL 绑定 `tenant_id AND entry_id` |
| LTM | `LTMRepository::get_entry_by_id` | 先按 entry_id 查，再 prefix 校验 | SQL 直接绑定 tenant_id |
| LTM | `LTMRepository::get_entry_history` | 无 TenantId | 改为 `get_entry_history_for_tenant(tenant_id, entry_id)` |
| LTM | `LTMRepository::supersede_entry` | 多步 update/insert，需确认同事务 | 同 transaction：旧版本 deprecated + 新版本 + history + outbox |
| LTM | `get_entry_at_time` / `search_entries_at_time` | 已有 TenantId，但仍依赖 source prefix 校验 | 改为物理 tenant_id + time range |
| KG | `KGRepository::create_entity` | 写 prefixed entity_id，未写 tenant_id | 写 tenant_id，entity_id prefix 仅兼容 |
| KG | `KGRepository::create_relation` | 无 TenantId，insert relation 后更新两个 entity 计数 | 显式 TenantId，验证 source/target 同租户，同 transaction |
| KG | `KGRepository::get_related_entities` | relation 查询未直接绑定 tenant | relation 与 entity 查询均绑定 tenant_id |
| KG | `get_entity_at_time` / `get_entity_history` | 无 TenantId | 改为 tenant-bound history/time-travel |
| KG | `supersede_entity` | 多步 update/insert，需确认同事务 | 同 transaction，并写 audit |
| MM | `MMRepository::create_entry` | tenant 存 JSON metadata / prefix，未写物理 tenant_id | 写 tenant_id column |
| MM | `MMRepository::get_entry_by_id` | 使用 JSON metadata tenant filter | 改为物理 tenant_id，JSON 仅兼容 |
| MM | `MMRepository::update_entry` | 无 tenant 参数，仅按 entry_id update | 改为 tenant-bound update |
| MM | `MMRepository::create_relation` | relation 本身未绑定 tenant | 显式 TenantId，验证 source/target 同租户 |
| MM | `MMRepository::count` | 全局 count | 改为 `count_for_tenant(tenant_id)` |
| MM | `MMRepository::list_entries` | 部分路径用 JSON metadata tenant filter | 全部改为 tenant_id column |
| Search | `memory_search.rs` LTM/Qdrant | Qdrant tenant payload filter 已有基础 | 强制 payload tenant 匹配 + PostgreSQL 回源校验 |
| Transfer | `memory_transfer.rs` | 使用全局 `get_active_user_ids()` | tenant-scoped 枚举，避免跨租户 metadata 扫描 |
| Routers | `memory_search.rs` history/time-travel endpoints | 部分 endpoint 调用无 tenant repository 方法 | 所有 endpoint 使用 `RequestTenantContext` |
| Routers | `knowledge_graph.rs::create_relation` | 调用 KG 无 tenant relation 方法 | route 强制传 tenant，禁止跨租户 relation |
| Routers | `multimodal.rs` | body/query tenant 只能兼容，不可授权 | 全部以 request tenant 为准 |

## 推荐方法签名方向

```rust
pub async fn update_entry(
    tenant_id: &TenantId,
    entry_id: &str,
    patch: UpdateKnowledgeEntry,
) -> Result<bool, AppError>;
```

```rust
pub async fn create_relation(
    tenant_id: &TenantId,
    source_entity_id: &str,
    target_entity_id: &str,
    relation_type: &str,
    weight: f64,
    confidence: f64,
    properties: Option<&Value>,
) -> Result<String, AppError>;
```

```rust
pub async fn get_entry_history(
    tenant_id: &TenantId,
    entry_id: &str,
) -> Result<Vec<KnowledgeEntry>, AppError>;
```

## Legacy 兼容策略

- 新写路径：必须写 `tenant_id` column。
- 兼容读路径：短期可 fallback 到 prefix，但必须记录 audit/metric。
- mutation 路径：不允许 fallback；没有 `tenant_id` 的历史数据只能只读隔离。
- cleanup 阶段：移除生产路径对 prefix 的安全依赖。

## Handoff

- 背景：P0-S2 需要明确 TenantId 生产路径改造边界。
- 输入依据：backend storage repository / router / service 盘点。
- 结论：高风险路径集中在 update/history/time-travel/relation mutation、STM 多步写、MM JSON tenant filter、后台 transfer 全局枚举。
- 风险：若继续保留无 tenant mutation，会绕过后续 RLS / schema 设计。
- 待确认项：是否允许保留无 tenant wrapper 作为 test-only 或 migration-only API。
- 下一跳角色：backend-engineer / qa-engineer。
- 当前阶段：design-review。
- 目标阶段：handoff-ready。
- 就绪状态：ready-for-review。
- readiness proof：本文件列出生产路径改造清单，可映射到 RED tests。
- accepted_by：待 backend-engineer 确认。
- 阻塞项：代码尚未改造。
