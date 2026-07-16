# Test Plan: Memory Storage Reliability

## 测试范围

### 功能范围

本测试计划覆盖企业高可靠整改中的 memory storage 关键路径：

- STM / LTM / KG / MM schema-level tenant isolation。
- router / service / repository 的 TenantId 强制传递。
- update、delete、history、time-travel、relation mutation 的跨租户拒绝。
- STM、KG、LTM 多步写事务一致性。
- LTM 与 Qdrant durable outbox、worker retry、dead-letter、reconciliation。
- Qdrant tenant payload、回源校验和 repair。
- backup / restore / rebuild / rollback operational drills。
- audit event、metrics、alerts 和 release gate evidence。

### 非功能范围

- 数据隔离与安全：跨租户读取、更新、删除、history/time-travel 均必须拒绝。
- 可靠性：故障注入下不产生半写、孤儿向量不可长期残留、outbox 可重放。
- 可恢复性：PostgreSQL、Qdrant、Neo4j 有恢复演练证据。
- 可观测性：关键 failure mode 有 metrics、logs、audit 和 alerts。
- 发布治理：migration、rollback、launch acceptance 有执行证据。

### 不覆盖项

- 非 memory storage 的 planner、runtime、workflow、billing 可靠性。
- 跨地域 active-active。
- 替换 Qdrant / PostgreSQL / Neo4j 的方案验证。
- 供应商托管服务采购和合同 SLA 验证。

## 测试矩阵

| 场景 | 类型 | 前置条件 | 预期结果 |
|------|------|----------|----------|
| 所有 memory-owned 表存在 `tenant_id` 或兼容迁移说明 | Schema verification | migration 已执行 | STM/LTM/KG/MM/history/relation/outbox/audit 表可验证 tenant 归属 |
| tenant-scoped indexes / constraints 存在 | Schema verification | migration 已执行 | `(tenant_id, id)` 或等价索引可查询，唯一约束按租户分区 |
| RLS 缺失 tenant context 时拒绝访问 | Integration / Security | RLS 已启用 | 查询或写入失败，不返回任何租户数据 |
| tenant A 读取 tenant B LTM history | Integration / Negative | 两租户各有 LTM 数据 | 请求被拒绝或返回空，不泄漏 tenant B 数据 |
| tenant A 执行 tenant B KG time-travel 查询 | Integration / Negative | 两租户各有 KG entity history | 请求被拒绝或返回空 |
| tenant A 更新 tenant B MM entry | Integration / Negative | 两租户各有 MM entry | update 被拒绝，tenant B 数据不变 |
| 跨租户 KG relation 创建 | Integration / Negative | 两租户各有 entity | relation 创建失败，无 relation_count 变化 |
| STM add_message 第二步失败 | Unit / Fault injection | 注入 context_length update 失败 | message insert 回滚，无半写 |
| STM delete_session 中途失败 | Unit / Fault injection | 注入删除失败 | session 与 messages 状态一致 |
| KG create_relation 第二个 entity update 失败 | Unit / Fault injection | 注入 relation_count update 失败 | relation insert 回滚，两个 entity 计数不变 |
| LTM fact 写入成功，Qdrant worker 停止 | Integration | outbox worker 暂停 | LTM entry 持久化，outbox pending，恢复 worker 后成功同步 |
| outbox event 重复处理 | Unit / Integration | 同一 idempotency key 处理两次 | Qdrant 状态一致，无重复或错误状态 |
| Qdrant upsert 持续失败 | Integration / Fault injection | Qdrant 返回错误 | event retry 后进入 dead-letter，触发告警 |
| Qdrant 缺少 active LTM vector | Reconciliation | 人为删除 point | dry-run 报告 missing，repair 后恢复 |
| Qdrant 存在 orphan vector | Reconciliation | 人为插入不存在 entry 的 point | dry-run 报告 orphan，repair 后删除或隔离 |
| Qdrant tenant payload 错误 | Integration / Security | point tenantId 被篡改 | 搜索回源校验拒绝返回，reconciliation 报告 mismatch |
| 批量 LTM 部分失败 | Integration | 注入单条失败 | 响应包含成功列表、失败列表、可重试标识和 correlation id |
| MM base64 非法输入 | Unit / API | 请求非法 base64 | 返回明确 4xx 错误，不写入空数据 |
| tenant isolation violation audit | Integration / Audit | 发起跨租户访问 | 持久化审计事件存在且脱敏 |
| PostgreSQL restore drill | Operational | 有备份或 PITR 配置 | 恢复到目标时间点，校验 memory 数据和 RPO/RTO |
| Qdrant rebuild drill | Operational | 清空或替换 Qdrant collection | 从 PostgreSQL + outbox/reconciliation 恢复向量索引 |
| Neo4j restore drill | Operational | 生产依赖 Neo4j | dump/backup restore 后 KG 查询通过 |
| outbox backlog alert | Operational / Alert | 暂停 worker 制造积压 | 告警触发，owner 可见，恢复后告警解除 |
| tenant isolation violation alert | Operational / Alert | 触发跨租户拒绝 | 告警或安全事件按规则记录 |
| migration rollback / forward-fix drill | Release | migration dry-run 环境 | 回滚或 forward-fix 命令可执行，数据一致 |

## 风险

| 风险 | 高风险路径 | 数据准备 | 回归关注点 |
|------|------------|----------|------------|
| RLS context 配置错误 | 所有 DB 查询 | 多租户样例数据、连接池复用 | 缺失 context 是否默认拒绝，连接复用是否污染 tenant |
| backfill 错误 | 历史 STM/LTM/KG/MM 数据 | prefix 正常、prefix 缺失、prefix 冲突样例 | 无法归属数据是否 quarantine，不误归属 |
| 事务边界遗漏 | STM/KG/LTM 多步写 | 故障注入 mock 或测试 repository | 失败后无半写、计数一致 |
| outbox worker 重复或丢失 | LTM/Qdrant 同步 | pending/failed/applied/dead-letter events | 幂等、retry、dead-letter、恢复后重放 |
| reconciliation 误修复 | Qdrant repair | missing/orphan/mismatch point | repair 默认 dry-run，执行前 diff 可审查 |
| restore drill 只验证服务存活 | 备份恢复 | 真实 memory 数据和校验 SQL | 恢复后业务查询、tenant isolation、Qdrant consistency 均验证 |
| 告警无 owner | 运维告警 | 告警测试事件 | pager routing、升级路径、closeout 是否完整 |

## 放行建议

### P0 放行条件

以下全部满足前，不建议进入企业内部生产级高可靠放行：

- Schema verification 通过。
- RLS / database policy negative tests 通过。
- update/delete/history/time-travel/relation 跨租户负向测试通过。
- STM / KG / LTM 多步写事务故障注入测试通过。
- outbox worker retry、dead-letter、idempotency tests 通过。
- reconciliation missing/orphan/mismatch tests 通过。
- PostgreSQL restore drill 通过。
- Qdrant rebuild 或 snapshot restore drill 通过。
- 告警触发和恢复验证通过。
- migration dry-run 与 rollback / forward-fix drill 通过。

### 当前放行结论

当前状态：**不建议放行到企业内部生产级高可靠环境**。

原因：本测试计划是整改契约，相关代码、migration、runbook、告警和演练证据尚未完成。

## 推荐验证命令

代码实现完成后，在 `backend` 目录执行：

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --all-features tenant
cargo test --all-features memory
cargo test --all-features isolation
cargo test --all-features integrity
cargo test --all-features transfer
cargo test --all-features qdrant
cargo sqlx prepare --check
```

环境型 E2E：

```bash
AMS_E2E=1 cargo test --test memory_platform_e2e
AMS_E2E=1 cargo test --test memory_reliability_e2e
```

建议专项审查：

```bash
rg "pub async fn .*update|pub async fn .*history|pub async fn .*at_time|create_relation" backend/src/db backend/src/routers
rg "begin\(|transaction\(|commit\(|rollback\(" backend/src
rg "tenant_id|tenantId|source_id LIKE|prefix\(\)" backend/src/db backend/src/services
rg "record_write|record_isolation_violation|audit|journal|outbox|reconciliation" backend/src
```

## 证据记录格式

每次测试或演练应记录：

- 日期：
- Commit：
- 环境：local / staging / production-like / production
- 执行人：
- 命令或 runbook：
- 输入数据：
- 结果：passed / failed / blocked
- 失败项：
- 修复或缓解：
- 残余风险：
- 是否阻塞发布：yes / no
