# Deployment Context: Memory Storage Reliability

## 环境清单

| 环境 | 用途 | 访问入口 | 部署目标 | 数据约束 | 当前结论 |
|------|------|----------|----------|----------|----------|
| local/dev | 本地开发、功能验证 | `docker-compose.yml`、backend `:8008`、frontend `:8000` | 单节点 PostgreSQL/Qdrant/Neo4j/Redis/backend/frontend | 不接入真实租户数据 | 仅作为开发环境，不作为生产 HA 证据 |
| staging | P0 集成验证、migration dry-run、50 并发验证 | 待 DevOps 补充 | PostgreSQL 自建、Qdrant cluster、Neo4j 必选环境的预生产等价拓扑 | 可使用脱敏多租户样例数据 | 待建设 |
| production-like | restore/rebuild/rollback drill | 待 DevOps 补充 | 尽量贴近生产拓扑 | 使用脱敏或批准后的演练数据 | 待建设 |
| production | 正式运行环境 | 待 Tech Lead / DevOps 补充 | PostgreSQL 自建 HA、Qdrant cluster、Neo4j 必选 | 真实租户数据 | 当前不允许声明 release-ready |

## 部署入口

### 主入口

- Backend：Rust / Axum service，当前本地入口为 `backend` service 和 `cargo run`。
- PostgreSQL：本轮采用自建方案，生产拓扑需补 primary/replica、failover、PITR 或等价能力。
- Qdrant：采用 cluster 方向，需补 cluster 配置、snapshot 或 rebuild 策略。
- Neo4j：生产必选，需补 cluster 或等价高可用、backup/restore 方案。
- Redis：不作为不可恢复事实源，只能承接缓存或短期协调能力。

### 手工入口

- Migration dry-run：后续 release-plan 中定义具体命令。
- Backfill report：后续实现 `tenant_id` backfill 后提供命令和输出路径。
- Qdrant rebuild：后续通过 PostgreSQL + outbox/reconciliation 或 snapshot restore 执行。
- Neo4j restore：后续按 dump/backup restore runbook 执行。

### 回退入口

- 应用镜像回滚：仅适用于非破坏性应用发布。
- Migration 回退：本项目优先采用 expand-contract 和 forward-fix；已写入生产事实数据后禁止 destructive rollback。
- Outbox worker 回退：可暂停 worker，保留 PostgreSQL fact 和 pending events，修复后重放。
- Qdrant 回退：以 PostgreSQL 为事实源，通过 reconciliation / rebuild 修复，不回滚 DB 适配 Qdrant。

### 前置条件

- P0-S1~P0-S7 foundation stories 完成 design review。
- `tenant_id` schema / RLS / outbox / reconciliation 方案已锁定。
- 50 并发基线验证入口已定义。
- 历史无法归属数据只读隔离策略已落地。
- 企业审计日志保留周期和敏感字段脱敏规则需在 release 前确认。

## 配置与密钥

| 配置项 | 用途 | 来源 | 当前要求 |
|--------|------|------|----------|
| `DATABASE_URL` | PostgreSQL 连接 | 环境变量 / secret manager | production 必须指向 PostgreSQL 自建拓扑，不允许 SQLite fallback |
| `APP_JWT_SECRET` | JWT 签名 | secret manager | production 禁止默认值，禁止 `jwt.disabled=true` |
| Qdrant endpoint / auth | 向量索引 | secret manager / config | cluster endpoint、auth/TLS 决策需记录 |
| Neo4j URI / auth | KG backend | secret manager / config | 生产必选，禁止默认密码，需记录 restore/cluster 策略 |
| Backup storage credentials | 备份归档 | secret manager | PostgreSQL/Qdrant/Neo4j restore drill 前必须确认 |
| Outbox worker config | 向量同步 | config | batch size、retry、dead-letter、pause/resume 入口需记录 |

安全约束：

- production 禁止 `jwt.disabled=true`。
- production 禁止默认 secret，例如 `change-me-in-production-32chars`、`password`、`admin`。
- production 配置需说明 PostgreSQL TLS、Qdrant auth/TLS、Neo4j auth/TLS 决策。
- 示例和日志不得泄漏真实 token、密码、租户数据或私人 endpoint。

## 运行保障

### Feature flag / 灰度控制

建议后续实现以下开关：

- `tenant_id_dual_write_enabled`
- `tenant_id_dual_read_enabled`
- `rls_enforce_enabled`
- `vector_outbox_enabled`
- `vector_outbox_worker_enabled`
- `reconciliation_dry_run_enabled`
- `read_after_write_fallback_enabled`

灰度策略：

1. 先 expand migration，不启用 RLS enforce。
2. 开启 dual-write，验证新写数据有 `tenant_id`。
3. 执行 backfill report，只读隔离无法归属数据。
4. 开启 TenantId 强制路径和 negative tests。
5. 开启 outbox worker，保留 dry-run reconciliation。
6. 验证 50 并发下写入、搜索、outbox lag 和恢复影响。
7. 再评估 RLS enforce 与 cleanup。

### 监控

告警暂不纳入当前 handoff-ready 门禁，但正式生产发布前仍建议补齐：

- DB/Qdrant/Neo4j availability。
- Outbox backlog、oldest pending age、dead-letter。
- Reconciliation drift：missing/orphan/tenant mismatch/content hash mismatch。
- Tenant isolation violation。
- 50 并发压测下 latency、error rate、outbox lag。
- Backup age、restore drill status。

### 值守安排

当前用户确认：告警暂时不考虑。

因此本阶段不要求 on-call owner 作为 handoff-ready 前置条件。若进入正式生产发布，需由 tech-lead 重新确认告警接收渠道、值守 owner 和升级路径。

### 观察窗口

建议 release-plan 定义：

- 发布后至少覆盖一个 outbox retry 周期。
- 发布后至少覆盖一个 reconciliation dry-run 周期。
- 发布后至少覆盖一次 50 并发 smoke 或压力验证窗口。
- 发布后至少完成 PostgreSQL restore、Qdrant rebuild、Neo4j restore 的 production-like drill 记录。

## 恢复能力

### PostgreSQL

目标：自建 PostgreSQL 方案。

必须补充：

- primary/replica 或等价高可用拓扑。
- PITR / WAL archive 或等价备份机制。
- restore drill runbook。
- migration dry-run 与 forward-fix 策略。
- `tenant_id` backfill 报告与只读隔离报告。

### Qdrant

目标：Qdrant cluster。

必须补充：

- cluster 拓扑、replication / shard 策略。
- snapshot restore 路径。
- PostgreSQL + outbox/reconciliation rebuild 路径。
- payload 校验：`tenantId`、`entryId`、`contentHash`。
- drift repair 默认 dry-run，执行修复需 admin 明确确认。

### Neo4j

目标：生产必选。

必须补充：

- cluster 或等价高可用方案。
- dump/backup restore runbook。
- KG 查询 smoke。
- tenant boundary 验证。
- 如果 Neo4j 只是增强索引，应重新进入 tech-lead 决策；当前用户已确认生产必选。

### Backend / Outbox

必须补充：

- Outbox worker pause/resume。
- pending/failed/dead-letter 重放。
- read-after-write fallback：按 ID 回源 PostgreSQL 读取。
- Qdrant worker 故障时，PostgreSQL fact 不丢失。

## 当前阻塞项（更新于 2026-07-17）

- ~~`tenant_id` schema、RLS、outbox、reconciliation 尚未实现。~~ **部分完成**：
  - ✅ Schema-level RLS 已交付（LTM/STM/KG/MM 四层，PR-3）
  - ✅ `tenant_scope` 执行器已交付（`begin_tenant_tx` + GUC，PR-1）
  - ✅ LTM→Qdrant outbox 已交付（DB+outbox 单事务 + async worker，PR-4/PR-5）
  - ❌ Reconciliation scanner 尚未实现（missing/orphan/tenant_mismatch/content_hash_mismatch 修复）
- PostgreSQL 自建 HA / restore drill 尚未完成。
- Qdrant cluster / rebuild drill 尚未完成。
- Neo4j 必选恢复演练尚未完成。
- 50 并发基线验证尚未完成。
- 企业审计日志保留周期和敏感字段脱敏规则仍待确认。
- ⚠️ 部署角色安全：默认 `memory` 角色可能为 superuser → RLS 成为 NO-OP。生产必须使用非 BYPASSRLS 应用角色（见 [P1 deployment-context](../2026-07-16-enterprise-productionization/deployment-context.md)）。

## 放行结论

当前 deployment context 结论：**P1 地基建有进展，但尚未达到生产就绪**。

已交付：schema RLS + outbox + 治理 middleware scaffold。仍需完成：reconciliation scanner、DB 备份/恢复演练、非 BYPASSRLS 部署角色、50 并发验证、审计保留策略。
