# ADR-0002: Memory Vector Outbox And Reconciliation

## 决策信息

- 编号：ADR-0002
- 决策标题：LTM 与 Qdrant 采用 durable outbox + async worker + reconciliation
- 状态：Accepted
- 实现状态：部分落地 —— durable outbox + async worker + 对账扫描器已落地并在 `main.rs` 启动；尚缺 outbox/drift 指标写入（当前恒为 0，backlog B-5，已接 outbox 部分）、`claim_batch` 租户公平性调度（C-2）、worker crash/retry/drift 端到端演练证据。
- 日期：2026-07-06（提出）；2026-08-11（按实现核实收口状态）
- Owner：architect / backend-engineer
- 收口责任人：tech-lead（Design Review Board 收口，2026-08-11）
- 关联需求：`docs/artifacts/2026-07-06-memory-storage-reliability/prd.md`
- 关联架构：`docs/architecture/memory-storage-reliability.md`

## 实现核实与缺口（2026-08-11 收口）

按代码核实，PostgreSQL durable outbox + async worker + reconciliation 模型**已落地**，故收口为 `Accepted`，剩余缺口在可观测性（metrics）与部分测试证据。

**已落地：**

- durable outbox：`backend/src/db/vector_outbox.rs`（transactional outbox 表 + `claim_batch` 用 `FOR UPDATE SKIP LOCKED`，有 5 个行为测试含并发验证）。
- async worker：`backend/src/services/outbox_worker.rs`，在 `backend/src/main.rs:112` 通过 `init_outbox_worker()` 启动（PG only）。
- reconciliation 扫描器：`backend/src/services/vector_reconciliation.rs`，在 `backend/src/main.rs:116` 通过 `init_reconciliation_scanner(&config.reconciliation)` 接线（W1.1，作为 outbox 的兜底 drift 扫描）。

**尚未落地 / 缺口（对应 backlog B-5、C-2）：**

1. **outbox / drift metrics 恒为 0**（backlog B-5）：本 ADR 后续动作要求「增加 outbox backlog 和 drift metrics，Prometheus 可采集并有告警规则」。`outbox_pending` gauge、outbox `dead_letter` counter、qdrant upsert success/failure 等指标虽已注册但**从未写入**，需完成 instrumentation（backlog B-5 及其子项）。相关告警已放在 `monitoring/alerts-staged/aetheris-pending-instrumentation.yml` 待接线。
2. **`claim_batch` 无租户公平性调度**（backlog C-2）：当前批量领取不做跨租户公平性，单租户大量积压可能饿死其他租户。
3. **worker crash / retry / drift E2E**：后续动作要求 reliability E2E 有执行证据；当前有 outbox 单元/行为测试，但完整 crash/retry/drift 端到端演练证据仍归 ADR-0003 运维准入范畴，尚未在仓内留下证据。

## 背景与约束

当前 LTM 写入路径先写 Qdrant，再写 PostgreSQL；如果 PostgreSQL 写入失败，会尝试删除 Qdrant point。这说明实现已经意识到跨存储双写一致性风险，但该补偿逻辑不能覆盖进程崩溃、网络抖动、补偿删除失败、重复写入、恢复后索引漂移等企业生产场景。

企业高可靠要求下，PostgreSQL 与 Qdrant 的职责需要明确：

- PostgreSQL 是 LTM 事实源，负责关系事实、租户隔离、history、审计和事务。
- Qdrant 是可重建的向量检索索引，负责 similarity search，不应成为不可恢复事实源。

约束：

- 保留 Qdrant 作为当前向量数据库。
- 不追求 PostgreSQL 与 Qdrant 同步强一致。
- 新写入记忆可以接受短暂 eventual consistency，但必须可观测、可恢复。
- 所有 Qdrant 写入必须具备 tenant payload 和幂等语义。

非目标：

- 不在本 ADR 中替换 Qdrant。
- 不实现跨区域 active-active 向量索引。
- 不把 Redis 或内存队列作为唯一 outbox 事实源。

## 备选方案

| 方案 | 适用条件 | 优点 | 风险或成本 | 不选原因 |
|------|----------|------|------------|----------|
| 继续同步双写 + 补偿删除 | 本地开发、低并发演示 | 代码简单，写后可立即搜索 | 崩溃不可恢复，补偿失败会产生孤儿向量 | 不满足企业可靠性 |
| 先写 DB 再同步写 Qdrant | 希望 DB 为事实源但仍同步返回 | 比先写 Qdrant 更清晰 | Qdrant 失败会让用户写入失败；重试和恢复仍弱 | 可靠性不足 |
| 使用消息队列作为 outbox | 已有稳定 MQ 平台 | 解耦明显 | 需要引入新基础设施，事务一致性仍需 outbox pattern | 当前仓库无此基础设施 |
| PostgreSQL durable outbox + worker + reconciliation | 企业可靠性、保留现有栈 | 可恢复、可审计、幂等、无需新增核心中间件 | 实现复杂，搜索有短暂延迟 | 采用 |

## 决策结果

采用 PostgreSQL durable outbox + async worker + reconciliation 的 LTM/Qdrant 一致性模型。

决策内容：

1. PostgreSQL LTM entry / metadata / history 是事实源。
2. LTM 写入 transaction 内同时写入 outbox event。
3. outbox event 至少包含：`tenant_id`、`entry_id`、`operation`、`payload_hash`、`idempotency_key`、`status`、`attempt_count`、`next_retry_at`、`last_error`。
4. worker 异步处理 pending events，对 Qdrant 执行幂等 upsert / delete。
5. Qdrant payload 必须包含 `tenantId`、`entryId`、`contentHash` 或等价校验字段。
6. worker 成功后将 event 标记为 applied；失败按 backoff 重试，超过阈值进入 dead-letter 并告警。
7. reconciliation job 定期比较 PostgreSQL active entries 与 Qdrant points，发现 missing、orphan、tenant mismatch、content hash mismatch 并生成 repair report。
8. 搜索结果必须回源 PostgreSQL 做 tenant 和 status 校验。

影响范围：

- `backend/migrations/`：新增 outbox / reconciliation / audit 相关 schema。
- `backend/src/services/memory_storage.rs`：LTM 写入从同步双写改为 DB fact + outbox。
- `backend/src/services/qdrant.rs`：幂等 upsert/delete、payload 校验、repair 支持。
- `backend/src/services/memory_search.rs`：搜索结果回源校验。
- `backend/src/services/`：新增或扩展 outbox worker 与 reconciliation service。
- `backend/tests/`：新增 outbox retry、worker crash、reconciliation drift 测试。

兼容性 / 迁移影响：

- 现有 Qdrant points 需要 tenant payload 和 content hash backfill。
- 迁移期可保留同步写入作为兼容保护，但最终以 outbox 为可靠性边界。
- 搜索新写入 LTM 可能出现短暂延迟，需要在 API 和文档中说明。

失败或回退思路：

- worker 失败时不丢数据，outbox 保留 pending / failed 状态。
- Qdrant 大面积损坏时，通过 PostgreSQL active entries + embedding service rebuild。
- 如果 outbox worker 引入问题，可暂停 worker，事实写入仍保存在 PostgreSQL，待修复后重放。
- 不回退到“Qdrant 是事实源”的模型。

## 企业内控补充

- 应用等级：待 tech-lead 确认；涉及企业 AI agent 长期记忆，建议按高可靠数据服务处理。
- 技术架构等级：跨存储一致性和可恢复性属于发布准入核心证据。
- 关键组件：PostgreSQL、Qdrant、embedding service、outbox worker、reconciliation job。
- 平台偏离：不新增外部 MQ，优先使用 PostgreSQL transactional outbox，降低基础设施复杂度。
- 资产文档入口：`docs/architecture/memory-storage-reliability.md`、`docs/artifacts/2026-07-06-memory-storage-reliability/test-plan.md`。

## 后续动作

| 动作 | Owner | 完成条件 |
|------|-------|----------|
| 设计 outbox schema 和状态机 | architect / backend-engineer | migration proposal 通过 review |
| 改造 LTM 写入为 fact + outbox transaction | backend-engineer | DB 成功、Qdrant worker 失败场景可重放 |
| 实现幂等 Qdrant upsert/delete | backend-engineer | duplicate event test 通过 |
| 实现 reconciliation dry-run 和 repair | backend-engineer | missing/orphan/mismatch 测试通过 |
| 增加 outbox backlog 和 drift metrics | backend-engineer / devops-engineer | Prometheus 可采集并有告警规则 |
| 在测试计划中加入 worker crash/retry/drift E2E | qa-engineer | reliability E2E 有执行证据 |
