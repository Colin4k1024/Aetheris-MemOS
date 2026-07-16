# Launch Acceptance: Memory Storage Reliability

## 验收概览

| 字段 | 内容 |
|------|------|
| 验收对象 | Memory Storage Reliability P0/P1 Remediation |
| 当前状态 | blocked |
| 验收时间 | 待定 |
| 验收环境 | staging / production-like，待 DevOps 补充 |
| 验收角色 | qa-engineer / tech-lead |
| 执行角色 | backend-engineer / devops-engineer |
| 关联 PRD | `docs/artifacts/2026-07-06-memory-storage-reliability/prd.md` |
| 关联测试计划 | `docs/artifacts/2026-07-06-memory-storage-reliability/test-plan.md` |
| 关联部署上下文 | `docs/artifacts/2026-07-06-memory-storage-reliability/deployment-context.md` |
| 关联发布计划 | `docs/artifacts/2026-07-06-memory-storage-reliability/release-plan.md` |

## 验收范围

### 业务范围

- 记忆存储在多租户场景下具备显式租户隔离。
- LTM 写入与 Qdrant 索引同步具备可恢复 eventual consistency。
- STM / KG / LTM 多步写入具备事务一致性。
- 历史无法归属数据进入只读隔离。
- 支持 read-after-write fallback：按 ID 回源读取。
- 默认支持 50 并发的容量与性能基线。

### 技术范围

- PostgreSQL 自建方案的 schema、RLS、restore drill。
- Qdrant cluster 方案和 rebuild / repair drill。
- Neo4j 生产必选的 restore drill。
- Outbox worker、dead-letter、reconciliation dry-run / repair。
- migration dry-run、rollback / forward-fix。
- audit event 关键路径。

### 非功能范围

- 租户隔离。
- 事务一致性。
- 可恢复性。
- 50 并发下的写入、搜索、outbox lag 和恢复影响。
- 发布回滚。
- 审计脱敏。

### 不在本次范围内

- 企业 T1/T2/T3/T4 应用等级门禁。
- 告警 owner / pager routing；用户已确认告警暂时不考虑。
- 跨地域 active-active。
- 替换 PostgreSQL / Qdrant / Neo4j。
- 非 memory storage 的 planner、runtime、billing、workflow 可靠性。

## 验收证据

| 证据项 | 当前状态 | 证据入口 | 验收结论 |
|--------|----------|----------|----------|
| PRD | 完成 | `prd.md` | 已具备输入 |
| Architecture | 完成 | `docs/architecture/memory-storage-reliability.md` | 已具备输入 |
| Team execution plan | 完成 | `team-execution-plan.md` | 已具备输入 |
| Deployment context | 完成草案 | `deployment-context.md` | 需随真实环境补充 |
| Release plan | 完成草案 | `release-plan.md` | 当前 blocked |
| `tenant_id` schema migration | 未完成 | 待 PR / migration | 阻塞 |
| Backfill report + read-only isolation | 未完成 | 待执行报告 | 阻塞 |
| RLS missing context tests | 未完成 | 待测试结果 | 阻塞 |
| Tenant negative tests | 未完成 | 待测试结果 | 阻塞 |
| Transaction fault injection | 未完成 | 待测试结果 | 阻塞 |
| Outbox / reconciliation E2E | 未完成 | 待测试结果 | 阻塞 |
| PostgreSQL restore drill | 未完成 | 待 drill evidence | 阻塞 |
| Qdrant cluster / rebuild drill | 未完成 | 待 drill evidence | 阻塞 |
| Neo4j restore drill | 未完成 | 待 drill evidence | 阻塞 |
| 50 并发验证 | 未完成 | 待性能/容量报告 | 阻塞 |
| Rollback / forward-fix drill | 未完成 | 待 drill evidence | 阻塞 |
| 审计日志保留与脱敏规则 | 未确认 | 待 tech-lead 决策 | 阻塞 |

## 风险判断

### 已满足项

- 可靠性问题已完成审查并形成 PRD、Delivery Plan、Test Plan、Architecture、ADR 和 Team Execution Plan。
- 用户已确认关键策略：不挂企业应用等级门禁、默认 50 并发、PostgreSQL 自建、Qdrant cluster、Neo4j 生产必选、告警暂不考虑、历史数据只读隔离、outbox 延迟可接受、read-after-write fallback 支持。
- 发布准入草案已定义 deployment context、release plan 和 launch acceptance。

### 可接受风险

- 告警暂时不考虑：本阶段不作为 handoff-ready 门禁，但进入正式生产发布前应重新评估。
- outbox eventual consistency：用户已接受，前提是支持 read-after-write fallback 和 `index_status=pending` 语义。
- 历史无法归属数据只读隔离：可接受，前提是不猜测归属、不删除，且 RLS enforce 前有隔离报告。

### 阻塞项

- P0 代码和 migration 未完成。
- PostgreSQL 自建 restore drill 未完成。
- Qdrant cluster / rebuild drill 未完成。
- Neo4j restore drill 未完成。
- 50 并发验证未完成。
- rollback / forward-fix drill 未完成。
- 审计日志保留周期和敏感字段脱敏规则未确认。

## 上线结论

当前结论：**blocked，不允许上线 / 不允许声明企业高可靠完成**。

允许进入下一阶段的条件：

1. P0-S1~P0-S7 foundation stories 完成 design review。
2. Build stories 完成代码、migration 和测试。
3. `test-plan.md` P0 测试矩阵全部有执行证据。
4. deployment-context 和 release-plan 中的环境、恢复、回滚信息从草案更新为真实执行证据。
5. QA 出具通过或有条件通过建议。
6. tech-lead 接受残余风险。

正式上线前提条件：

- `cargo fmt --check` 通过。
- `cargo clippy --all-targets --all-features -- -D warnings` 通过。
- `cargo test --all-features` 通过。
- `cargo sqlx prepare --check` 通过。
- `AMS_E2E=1 cargo test --test memory_platform_e2e` 通过。
- `AMS_E2E=1 cargo test --test memory_reliability_e2e` 通过。
- PostgreSQL/Qdrant/Neo4j 恢复演练通过。
- 50 并发验证通过或残余风险被 tech-lead 接受。
- rollback / forward-fix drill 通过。

## 确认记录

| 角色 | 结论 | 备注 |
|------|------|------|
| qa-engineer | blocked | P0 evidence 未完成 |
| devops-engineer | blocked | restore / rebuild / rollback drill 未完成 |
| backend-engineer | blocked | schema / RLS / outbox / reconciliation 未实现 |
| architect | blocked | design review 需继续收口 |
| tech-lead | blocked | 审计规则与 P0 证据未完成 |

## Handoff

- 背景：memory storage reliability 已完成团队组建和发布准入草案，需要进入 P0 实施与验证。
- 输入依据：PRD、Delivery Plan、Test Plan、Architecture、ADR、Team Execution Plan、Deployment Context、Release Plan。
- 结论：当前 launch acceptance 为 blocked。
- 风险：代码、migration、恢复演练、50 并发验证和审计规则未完成。
- 待确认项：企业审计日志保留周期和敏感字段脱敏规则。
- 下一跳角色：backend-engineer / qa-engineer / devops-engineer。
- 当前阶段：release-prep planning。
- 目标阶段：execute。
- 就绪状态：blocked。
- readiness proof：发布准入文件已建立，但 P0 证据未完成。
- accepted_by：qa-engineer / tech-lead 待正式确认。
- 阻塞项：见“阻塞项”。
