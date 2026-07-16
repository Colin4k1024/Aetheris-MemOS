# Transaction Boundaries: Memory Storage Reliability

## 结论

STM、KG、LTM、MM 的多步写入必须升级为明确 transaction boundary。当前多个路径先做权限/归属校验，再执行多条 SQL；如果中间失败，可能留下半写、错误计数或 fact/outbox 不一致。P0 必须先用 RED tests 固定失败场景，再实现事务化。

本文件支撑 P0-S4：multi-step write transaction boundaries。

## 总体规则

- 一个业务状态转换只允许有一个 transaction boundary。
- 权限/tenant 校验、事实写入、history/audit/outbox 写入应在同一个 tenant-scoped transaction 中完成。
- 对外部系统 Qdrant 的调用不进入 DB transaction；通过 durable outbox 异步执行。
- 失败时只能留下完整旧状态，不能留下半新状态。
- 所有 mutation SQL 必须绑定 `tenant_id`。

## STM 边界

### STM add_message

当前路径：

1. 查询 `context_sessions` 验证 session 属于 tenant。
2. 插入 `session_messages`。
3. 更新 `context_sessions.context_length`。

风险：

- message insert 成功后 context_length update 失败，会留下半写 message。
- messages 当前未写物理 `tenant_id`。

目标 transaction：

```text
begin tenant tx
  set local tenant context
  SELECT context_sessions WHERE tenant_id = $tenant_id AND session_id = $session_id FOR UPDATE
  INSERT session_messages(tenant_id, ...)
  UPDATE context_sessions SET context_length = context_length + token_count
  INSERT memory_audit_events(...)
commit
```

RED tests：

- `stm_add_message_rolls_back_when_context_length_update_fails`
- `stm_add_message_rejects_cross_tenant_session`
- `stm_add_message_writes_message_tenant_id`

### STM delete_session

当前路径：

1. 查询 session 验证归属。
2. 删除 `session_messages`。
3. 删除 `context_sessions`。

风险：

- messages 删除成功后 session 删除失败，留下不一致状态。
- 按 session_id 删除未绑定 tenant_id。

目标 transaction：

```text
begin tenant tx
  SELECT context_sessions WHERE tenant_id = $tenant_id AND session_id = $session_id FOR UPDATE
  DELETE session_messages WHERE tenant_id = $tenant_id AND session_id = $session_id
  DELETE context_sessions WHERE tenant_id = $tenant_id AND session_id = $session_id
  INSERT memory_audit_events(...)
commit
```

RED tests：

- `stm_delete_session_rolls_back_when_message_delete_fails`
- `stm_delete_session_rolls_back_when_session_delete_fails`
- `tenant_a_cannot_delete_tenant_b_session`

## KG 边界

### KG create_relation

当前路径：

1. 插入 `relations`。
2. 更新 source entity `relation_count`。
3. 更新 target entity `relation_count`。

风险：

- relation 插入成功但计数更新失败。
- source/target 可能跨租户。
- 方法无 `TenantId` 参数。

目标 transaction：

```text
begin tenant tx
  SELECT source entity WHERE tenant_id = $tenant_id AND entity_id = $source FOR UPDATE
  SELECT target entity WHERE tenant_id = $tenant_id AND entity_id = $target FOR UPDATE
  INSERT relations(tenant_id, ...)
  UPDATE entities SET relation_count = relation_count + 1 WHERE tenant_id = $tenant_id AND entity_id = $source
  UPDATE entities SET relation_count = relation_count + 1 WHERE tenant_id = $tenant_id AND entity_id = $target
  INSERT memory_audit_events(...)
commit
```

RED tests：

- `tenant_a_cannot_create_relation_to_tenant_b_entity`
- `kg_create_relation_rolls_back_when_source_count_update_fails`
- `kg_create_relation_rolls_back_when_target_count_update_fails`
- `kg_create_relation_writes_relation_tenant_id`

### KG supersede_entity / history

目标 transaction：

- 锁定当前 entity。
- 更新旧 entity valid_until / superseded_by。
- 插入新 entity 或 entity version。
- 写 audit。

RED tests：

- `kg_supersede_entity_rolls_back_when_version_insert_fails`
- `tenant_a_cannot_read_tenant_b_entity_history`
- `tenant_a_cannot_time_travel_tenant_b_entity`

## LTM 边界

### LTM create/update/supersede with outbox

目标：PostgreSQL 是事实源；Qdrant 只由 outbox worker 处理。

目标 transaction：

```text
begin tenant tx
  INSERT/UPDATE knowledge_entries(tenant_id, ...)
  INSERT knowledge_entry_versions(tenant_id, ...)
  INSERT memory_vector_outbox(tenant_id, entry_id, operation, payload_hash, idempotency_key, status='pending')
  INSERT memory_audit_events(...)
commit
```

风险：

- fact 写入成功但 outbox 写入失败会导致向量索引永远缺失。
- supersede 旧版本 update 与新版本 insert 分离会导致 history 断裂。
- update_entry 当前无 TenantId。

RED tests：

- `ltm_fact_is_not_committed_when_outbox_insert_fails`
- `outbox_event_is_not_committed_when_ltm_fact_insert_fails`
- `ltm_supersede_rolls_back_when_history_append_fails`
- `tenant_a_cannot_update_tenant_b_ltm_entry`
- `tenant_a_cannot_read_tenant_b_ltm_history`
- `ltm_write_returns_pending_index_status_and_read_after_write_fallback`

### LTM soft_delete

目标 transaction：

- 锁定 tenant-bound entry。
- status 改为 deprecated。
- 写 delete outbox event。
- 写 audit。

RED tests：

- `tenant_a_cannot_soft_delete_tenant_b_ltm_entry`
- `ltm_delete_rolls_back_when_outbox_insert_fails`

## MM 边界

### MM update_entry

当前路径：

- 无 tenant 参数，仅按 entry_id update。

目标 transaction：

- `UPDATE multimodal_entries SET ... WHERE tenant_id = $tenant_id AND entry_id = $entry_id`。
- 写 audit。

RED tests：

- `tenant_a_cannot_update_tenant_b_mm_entry`
- `mm_update_writes_no_changes_when_entry_not_in_tenant`

### MM create_relation

目标 transaction：

- 锁定 source/target entries，同 tenant 校验。
- 插入 `modality_relations(tenant_id, ...)`。
- 写 audit。

RED tests：

- `tenant_a_cannot_create_mm_relation_to_tenant_b_entry`
- `mm_create_relation_rolls_back_when_relation_insert_fails`

## Outbox worker 事务边界

Worker claim：

```text
begin tx
  SELECT pending/failed due rows FOR UPDATE SKIP LOCKED
  UPDATE status='processing', locked_at=now(), locked_by=$worker
commit
```

Worker result：

```text
begin tx
  on success: UPDATE status='applied', applied_at=now()
  on retryable failure: UPDATE status='failed', attempt_count += 1, next_retry_at = ...
  on max attempts: UPDATE status='dead_letter', dead_lettered_at=now()
  INSERT audit event for dead_letter or repair
commit
```

Qdrant call happens outside claim transaction to avoid holding locks during network IO.

RED tests：

- `outbox_claim_uses_skip_locked`
- `outbox_duplicate_event_is_idempotent`
- `outbox_dead_letters_after_retry_threshold`
- `worker_crash_leaves_event_reclaimable`

## Handoff

- 背景：P0-S4 需要明确多步写事务边界。
- 输入依据：backend repository 盘点、ADR-0001、ADR-0002。
- 结论：STM add/delete、KG relation、LTM fact/history/outbox、MM relation 是首批事务化重点。
- 风险：如果先实现 RLS enforce 而不事务化，会把权限边界和一致性问题混在一起，排障困难。
- 待确认项：fault injection 使用 test-only hooks、mock repository，还是 DB trigger 方式。
- 下一跳角色：backend-engineer / qa-engineer。
- 当前阶段：design-review。
- 目标阶段：handoff-ready。
- 就绪状态：ready-for-review。
- readiness proof：本文件定义事务边界与 RED tests。
- accepted_by：待 backend-engineer / qa-engineer 确认。
- 阻塞项：代码尚未事务化。
