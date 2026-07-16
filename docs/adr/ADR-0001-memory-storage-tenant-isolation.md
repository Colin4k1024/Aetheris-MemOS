# ADR-0001: Memory Storage Tenant Isolation Baseline

## 决策信息

- 编号：ADR-0001
- 决策标题：Memory storage 采用显式 `tenant_id` + PostgreSQL RLS + 应用层 TenantId 校验
- 状态：Proposed
- 日期：2026-07-06
- Owner：architect / backend-engineer
- 关联需求：`docs/artifacts/2026-07-06-memory-storage-reliability/prd.md`
- 关联架构：`docs/architecture/memory-storage-reliability.md`

## 背景与约束

企业高可靠审查发现，当前 STM / LTM / KG / MM 的租户隔离主要依赖 `source_id`、`user_id`、`entity_id` 中的 `t:{tenant}` 前缀，以及应用层查询约定。该方式在 MVP 阶段能快速形成命名空间隔离，但不能满足企业内部多租户高可靠要求。

主要问题：

- schema 中缺少统一显式 `tenant_id`，数据库无法独立阻止跨租户访问。
- update、history、time-travel、relation mutation 等路径容易遗漏 tenant 过滤。
- prefix 规则分散在 repository、service 和后台任务中，可审计性弱。
- 历史数据和兼容 wrapper 可能绕过 request tenant。

约束：

- 需要兼容已有 `t:{tenant}` prefix 数据。
- 不能一次性破坏现有 E2E 和 SDK contract。
- 迁移必须支持 expand-contract，允许 backfill 与双读写阶段。
- PostgreSQL 是 memory-owned relational fact 的主要事实源。

非目标：

- 不在本 ADR 中定义完整跨地域隔离方案。
- 不替换 PostgreSQL。
- 不要求所有非 memory 模块同步采用同一模型。

## 备选方案

| 方案 | 适用条件 | 优点 | 风险或成本 | 不选原因 |
|------|----------|------|------------|----------|
| 继续使用 `t:{tenant}` prefix | PoC、本地开发、低风险单租户 | 改动小、兼容当前代码 | 容易遗漏过滤，数据库无强约束，审计弱 | 不满足企业多租户高可靠 |
| 仅在应用层强制 TenantId | repository 全面改造但不动 schema | 实施成本中等 | 数据库仍无法兜底，SQL 漏洞或遗漏路径仍有风险 | 防线不足 |
| 显式 `tenant_id` + 应用层 TenantId | 需要清晰查询边界 | 可审计，索引明确 | 仍缺少 DB policy 兜底 | 可作为迁移中间态，但不是最终态 |
| 显式 `tenant_id` + PostgreSQL RLS + 应用层 TenantId | 企业多租户生产 | 三层防线，默认拒绝，审计友好 | migration、backfill、连接上下文复杂 | 采用 |

## 决策结果

采用显式 `tenant_id` schema + PostgreSQL RLS + 应用层 TenantId 校验的三层隔离模型。

决策内容：

1. STM / LTM / KG / MM 及其 history、relation、outbox、audit 等 memory-owned tables 必须具备 `tenant_id` 或明确的兼容迁移策略。
2. 所有 repository 方法的生产路径必须显式接收 `TenantId`，不允许依赖隐式 default tenant。
3. 所有 update、delete、history、time-travel、relation mutation 必须绑定 tenant 过滤条件。
4. PostgreSQL RLS 或等价数据库 policy 默认拒绝缺失租户上下文的 memory 数据访问。
5. `t:{tenant}` prefix 在迁移期可作为 backfill 依据和兼容标识，但不再作为长期主安全边界。
6. 所有跨租户负向测试必须纳入 P0 验收。

影响范围：

- `backend/migrations/`
- `backend/src/db/stm.rs`
- `backend/src/db/ltm.rs`
- `backend/src/db/kg.rs`
- `backend/src/db/mm.rs`
- `backend/src/services/memory_storage.rs`
- `backend/src/services/memory_search.rs`
- `backend/src/services/memory_transfer.rs`
- memory storage routers 和 tenant context middleware
- 相关 E2E、tenant isolation 和 migration tests

兼容性 / 迁移影响：

- 需要 expand → backfill → dual-read/write → enforce → cleanup。
- 历史无法归属数据需要 quarantine 或只读隔离。
- default tenant wrappers 需要标注 deprecated，并从生产路径移除。

失败或回退思路：

- enforce 前保留 prefix fallback，避免一次性切换失败。
- 如果 RLS context 配置导致误拒绝，回退到应用层 TenantId + schema 约束中间态，但不得回退到纯 prefix 隔离。
- backfill 失败时阻塞 NOT NULL / RLS enforce 阶段。

## 企业内控补充

- 应用等级：待 tech-lead 根据业务评定等级和底线评定等级确认；无法确认前按 T2/T3 高风险内控口径处理。
- 技术架构等级：记忆存储涉及多租户数据和 AI agent 长期上下文，应按需纳入资产可视可控、审计、备份恢复和告警准入。
- 关键组件：PostgreSQL、Qdrant、Neo4j、Redis、backend memory services。
- 平台偏离：当前 prefix isolation 是 MVP 兼容机制，不满足长期企业多租户隔离基线。
- 资产文档入口：`docs/architecture/memory-storage-reliability.md`、`docs/artifacts/2026-07-06-memory-storage-reliability/`。

## 后续动作

| 动作 | Owner | 完成条件 |
|------|-------|----------|
| 产出 tenant_id migration 和 backfill plan | backend-engineer | migration dry-run 和数据归属报告完成 |
| 为 memory-owned tables 增加 tenant-scoped indexes/constraints | backend-engineer | schema verification tests 通过 |
| 设计并实现 RLS context 设置方式 | backend-engineer | 缺失 context negative test 通过 |
| 改造 update/history/time-travel/relation 方法签名 | backend-engineer | repository tests 覆盖所有生产路径 |
| 更新测试计划和放行门禁 | qa-engineer | `test-plan.md` P0 矩阵有执行证据 |
| 评审迁移风险和回滚策略 | tech-lead / architect | design review 通过 |
