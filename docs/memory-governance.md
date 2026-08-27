# 记忆治理与可观测性（#124 Epic 收口，#130）

企业级记忆生命周期的治理面：谁能看、谁能改、错了怎么查、怎么回滚、怎么观测。对应实现：

- API：`backend/src/routers/memory_governance.rs`（`/api/v1/governance`）
- 状态机与谓词契约：`docs/adr/ADR-0011`、`backend/src/models/belief.rs`
- 管理界面：前端 `MemoryGovernance` 页（`/memory-governance`）
- 黄金验收：`backend/tests/golden_acceptance_pg.rs`（8 场景，30+ 断言对话轮）

## API 面（全部租户隔离，tenant 取自认证态）

| 方法 | 路径 | 权限 | 说明 |
|---|---|---|---|
| GET | `/v1/governance/beliefs` | 成员（非管理员服务端钉死自己的 subject） | 信念列表，`include_history` 展开版本时间线（仅管理员） |
| GET | `/v1/governance/beliefs/{id}` | 成员（subject 范围） | 单条信念 + 证据 |
| GET | `/v1/governance/beliefs/{id}/trace` | 成员（subject 范围） | **完整可追溯面**：信念 + provenance 事件 + 审计链 |
| POST | `/v1/governance/beliefs/{id}/confirm` | Admin/Owner | 人工确认 needs_confirm → active |
| POST | `/v1/governance/beliefs/{id}/deny` | Admin/Owner | 否决 → rejected（终态，闭合窗口） |
| POST | `/v1/governance/beliefs/{id}/archive` | Admin/Owner | 归档（退出现行集） |
| POST | `/v1/governance/beliefs/{id}/rollback` | Admin/Owner | **回滚**：闭合当前边、恢复其前驱为现行 |
| POST | `/v1/governance/subjects/{subject}/forget` | Admin/Owner | GDPR 遗忘：归档该 subject 全部开边 |
| GET | `/v1/governance/candidates?status=pending\|quarantined` | Admin/Owner | 待确认 / 隔离队列 |
| GET | `/v1/governance/principals/{id}/aliases` | Admin/Owner | 主体身份键 |
| POST | `/v1/governance/principals/merge` | Admin/Owner | 匿名主体显式合并（可逆） |
| POST | `/v1/governance/principals/unmerge` | Admin/Owner | 撤销合并 |
| GET | `/v1/governance/stats` | 成员 | 活跃信念数 + 队列深度（仪表盘） |

所有变更走 supersede/close，绝不原地改写历史；每次操作落 `memory_audit_events`。

## 从错误行为到回滚（#124 验收 5 的操作路径）

1. `GET /beliefs?subject=...` 找到驱动行为的现行边；
2. `GET /beliefs/{id}/trace` 看到：SPO 与有效窗口、证据（不可变 `memory_events` 的 event_id + 内容哈希）、审计链（谁、何时、为何写入/变更）；
3. `POST /beliefs/{id}/rollback` 恢复已知好版本（或 `deny`/`archive`）。

## 指标（Prometheus，`/metrics`）

| 指标 | 含义 |
|---|---|
| `recall_requests_total` / `recall_items_total` | 召回核心请求与进入 Working Memory 的条目 |
| `memory_belief_active` | 现行信念数（治理 stats 端点刷新） |
| `consolidation_*`（6 项 + 时长直方图） | 巩固运行、冲突、stale、承诺到期、对账差异、失败 |

Epic 的四个上线盯防指标映射：召回命中 = `recall_items/requests`；过期事实误用 = 黄金负向（recall 只取 active + as_of）；跨用户泄漏 = 租户/principal 负向套件；写入后行为漂移 = 黄金回滚场景 + 审计链。

## 黄金验收（`golden_acceptance_pg.rs`，DATABASE_URL 门控）

1. 换工作：默认新雇主，历史 as_of 仍答旧雇主
2. 跨设备连续；共享平板永不串人
3. 网页转账指令永久隔离，重放同样隔离
4. HR 变更一个巩固周期（SLA）内闭合旧边
5. 管理员定位 belief/event/provenance 并成功回滚
6. 90 天等效负载：Working Memory 有界、活跃信念数稳定
7. 跨租户/跨主体负向全绿
8. 治理 RBAC（成员视图钉死、变更拒绝、管理员放行）+ OpenAPI 契约

## SDK

- Rust：`sdks/rust` — `governance_list_beliefs` / `governance_belief_trace` / `governance_rollback` / `governance_candidates`
- Python：`sdks/python` — 同名 snake_case 方法
- OpenAPI：`/api-doc/openapi.json`（utoipa 自动生成，含全部治理路由）
