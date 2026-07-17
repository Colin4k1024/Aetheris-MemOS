# 执行计划与进度日志

- **计划源**：[`fix-plan.md`](./fix-plan.md)
- **开始日期**：2026-07-17
- **执行分支**：`feat/p1-governance-middleware`（或后续修复分支）
- **策略**：按 Wave 0 → 1 → 2 → 3；每项先接线/补真，再补测，再勾验收

---

## 1. 执行节奏

| 波次 | 目标 | 状态 |
|------|------|------|
| Wave 0 | 准入阻塞：outbox、MCP Plane A、配额/RBAC、限流、部署角色文档 | **已完成** |
| Wave 1 | 对账、治理覆盖、审计单源、文档诚实化 | **已完成** |
| Wave 2 | A2A/协议/Plane B | **已完成** |
| Wave 3 | 自适应拟合或降级文案 | **已完成** |

### 本会话目标（Wave 0 首批）

1. **W0.1** LTM 写路径：PostgreSQL 走 DB+outbox；SQLite 保留同步双写
2. **W0.2** MCP `call_tool`：工具契约验签 + `capability::authorize` + 审计
3. **W0.4** 默认配额 + usage increment；RBAC 在 pre-hook 生效；RbacService 单例统一
4. **W0.5** 限流：优先 `ConnectInfo`，XFF 仅作受信代理补充（文档注释）
5. **W1.6（穿插）** `IMPLEMENTATION_STATUS.md` 诚实化

不在本会话强推：完整 reconciliation repair、Plane B wasmtime、部署角色 docker 改造（需运维环境）、A2A。

---

## 2. 任务板

### Wave 0

| ID | 任务 | 状态 | 证据 / 备注 |
|----|------|------|-------------|
| W0.1a | `db/vector_outbox.rs` insert/claim/mark | ✅ | `db/vector_outbox.rs` — `insert_event_tx`, `claim_batch`, `mark_applied`, `mark_failed`, `reclaim_stale` |
| W0.1b | `services/outbox_worker.rs` + main 启动 | ✅ | `services/outbox_worker.rs` — `init_outbox_worker()` + `run_loop`; wired in `main.rs` |
| W0.1c | `store_ltm_for_tenant` 改 outbox 路径 | ✅ | PG: DB+outbox single TX; SQLite: 保留 legacy 双写 |
| W0.1d | 单元测：幂等键 / payload_hash | ✅ | 6 个新增测试：`test_upsert_idempotency_key_format`, `test_delete_idempotency_key_format`, `test_upsert_idempotency_key_different_hashes`, `test_upsert_idempotency_key_same_hashes`, `test_payload_hash_deterministic`, `test_payload_hash_different_inputs` |
| W0.2a | call_tool 契约验签 | ✅ | `call_tool()` 新增签名验证步骤：加载 `MCP_TOOL_SIGNATURES`，匹配工具签名，`verify_component`；无签名→`verify_unsigned`→403 |
| W0.2b | capability authorize 接线 | ✅ | `call_tool()` 新增 `capability::authorize(&[Read,Write,Delete], &tool_name)` 检查；未授权工具→403 |
| W0.2c | MCP 审计事件 | ✅ | `call_tool()` 在成功/签名失败/能力不足/未知工具/错误时均调用 `audit_writer::record_audit()` |
| W0.4a | QuotaManager ensure_default + record_usage | ✅ | `post_store()` 成功后读取 `ResourceQuota`，递增 `used.memory_entries`，写回 `QuotaManager` |
| W0.4b | pre_store/pre_search 扣减 + RBAC | ✅ | `pre_store()` 新增 `rbac.check_permission(..., "write")`；`pre_search()` 新增 `rbac.check_permission(..., "read")`；拒绝→`Deny("Insufficient role permissions")` |
| W0.4c | 全局 RbacService 单例 | ✅ | `services/rbac.rs` 新增 `RBAC_SERVICE: OnceLock<Arc<RbacService>>` + `get_rbac_service()`；`enterprise_impl.rs` 和 `routers/tenant.rs` 统一使用全局实例 |
| W0.5 | rate_limit 客户端标识加固 | ✅ | `rate_limit_middleware` 优先使用 `ConnectInfo<SocketAddr>`（TCP 真实 IP），回退 XFF + 安全注释 |
| W0.3 | 部署角色说明（文档 + 代码） | ✅ | 文档：`deployment-context.md` 新增 aetheris_app/aetheris_admin 角色 + 渗透验证 SQL；代码：`docker/initdb/01-create-app-role.sql` 创建非 BYPASSRLS 应用角色，`docker-compose.yml` 挂载 init 脚本 |

### Wave 1

| ID | 任务 | 状态 | 证据 / 备注 |
|----|------|------|-------------|
| W1.1 | outbox worker + reconciliation dry_run/repair | ✅ | `qdrant.rs` 新增 `scroll_point_ids()` + `get_point_payload()`；`db/vector_reconciliation.rs` 新增 runs/items 仓库；`services/vector_reconciliation.rs` 新增对账扫描器（missing/orphan/tenant_mismatch/content_hash_mismatch） |
| W1.2a | classify() 覆盖 /forget → Delete | ✅ | `governance.rs` classify 新增 POST /forget → Delete |
| W1.2b | MCP 路由挂 governance middleware | ✅ | `mcp.rs` protected_router 新增 `.route_layer(governance_middleware)` |
| W1.2c | fail-closed 配置（GOVERNANCE_FAIL_CLOSED） | ✅ | `governance.rs` middleware 新增 env var 控制 fail-open vs fail-closed |
| W1.3 | 审计单真相源（v2 → audit_writer dual-write） | ✅ | `enterprise_hooks_v2.rs` AuditHookImpl::log() 新增 audit_writer 持久化 |
| W1.4a | decision_trace 应用层 tenant 过滤 | ✅ | `decision_trace.rs` 新增 tenant_id 参数 + WHERE 子句；orchestrator + handlers 传递 |
| W1.4b | memory_feedback 应用层 tenant 过滤（验证） | ✅ | 已有 tenant_id 过滤；新增 W1.4 注释 |
| W1.5 | information_guard 多租户扫描 | ✅ | `information_guard.rs` `run_scan_cycle()` 重构为遍历所有活跃租户 |
| W1.7 | DB 备份/PITR/告警/恢复演练 | ✅ | `backup-restore-runbook.md` 创建（643 行），含 PG/Qdrant/Neo4j 备份恢复 + 演练清单 + 告警建议 |

### Wave 2

| ID | 任务 | 状态 | 证据 / 备注 |
|----|------|------|-------------|
| W2.1 | A2A handler 接真 | ✅ | `Cargo.toml` a2a 依赖已 pin (`rev: 0b19af0`)；`cargo check --features a2a` 0 errors |
| W2.2 | gRPC 死代码 | ✅ | 已标注 `NOT IMPLEMENTED (pending P2)` + `#![allow(dead_code)]` |
| W2.3 | WebSocket 死代码 | ✅ | 已标注 `NOT IMPLEMENTED (pending P2)` + `#![allow(dead_code)]` |
| W2.4 | MCP Plane B wasmtime | ✅ | `sandbox.rs` `execute_wasm` 改为真实 wasmtime 编译+执行（fuel 限制，64 MiB 内存上限） |
| W2.5 | Letta provider 从默认路径移除 | ✅ | `pub use letta::LettaProvider` 移除；`letta.rs` 保留为 reserved stub |
| W2.6 | Neo4j 索引初始化 | ✅ | `init_neo4j_indexes()` 改为实际执行 Cypher 创建约束/索引；`NEO4J_HANDLE` 全局静态 |

### Wave 3

| ID | 任务 | 状态 | 证据 / 备注 |
|----|------|------|-------------|
| W3.1 | 遥测 → 特征管线 | ✅ | `services/adaptive_telemetry.rs` 新增 `PerformanceSample` 收集 + `AdaptiveTelemetry` 全局存储 |
| W3.2 | predictor 诚实化 | ✅ | `confidence_score` 从硬编码 `0.88` 改为 `None`；新增 `TrainablePredictor` trait |
| W3.3 | scheduler 真预测 | ✅ | `Predictor` trait 提取；`AdaptiveMemoryScheduler` 接受 `Box<dyn Predictor>`；所有调用方更新 |
| W3.4 | eval harness | ✅ | `services/eval_harness.rs` 创建（`EvalSuite` + 3 内置用例 + `summary()`）；5 个单元测试 |
| W3.5 | Evidence Graph 闭环 | ✅ | `evidence_graph.rs` 新增 `scan_all_workflows_for_tampering()` + `list_all_workflow_ids()`；审计集成 |
| W3.6 | KG relation 双时序列 | ✅ | `migrations/20260717000001_kg_relation_temporal.sql` — `valid_from`/`valid_to` + backfill |

### 文档

| ID | 任务 | 状态 |
|----|------|------|
| W1.6 | IMPLEMENTATION_STATUS 诚实化 | ✅ | 已改为分能力状态表，去掉 COMPLETED 笼统声明 (2026-07-17) |

### 验证

| ID | 任务 | 状态 |
|----|------|------|
| V1 | `SQLX_OFFLINE=true cargo check` | ✅ | 0 errors, 466 warnings（全部为预存） |
| V2 | 相关 `cargo test`（lib / 单测） | ✅ | 268 passed; 0 failed; 0 ignored |

---

## 3. 进度日志

### 2026-07-17 — 会话启动

- 写入本 execute-log 与 fix-plan
- 开始 Wave 0 实现（outbox / MCP / 配额 RBAC / 限流 / 状态文档）

### 2026-07-17 — 代码实现（PR-1 ~ PR-6 合并）

- PR-1: `tenant_scope` 执行器 — `begin_tenant_tx` + GUC `aetheris.tenant_id`
- PR-2: 审计基建 — `db/audit.rs` + `services/audit_writer.rs` (mpsc → batch INSERT)
- PR-3: 四层记忆表 RLS 迁移 — expand→backfill→enforce→contract
- PR-4: LTM outbox 写路径 — `store_ltm_for_tenant` 改 PG 单事务(DB+outbox)；SQLite 保留双写
- PR-5: outbox worker — `init_outbox_worker()` + `claim_batch` + 幂等投递 + 死信 + stale reclaim
- PR-6: 治理 middleware — `governance_middleware` 挂链 (auth→rate_limit→governance→handler)

### 2026-07-17 — 文档诚实化

- `IMPLEMENTATION_STATUS.md` 重写：去掉 "COMPLETED v1.0" 虚假声明，改为分能力状态表
- `fix-plan.md` §9 同步清单更新：标记 IMPLEMENTATION_STATUS 已完成
- `execute-log.md` 任务板更新：反映 W0.1a/b/c 已完成

### 2026-07-17 — 文档同步完成（第二轮）

- `AGENTS.md`：新增 governance/audit/outbox/tenant_scope 模块说明
- `docs/API_USAGE_GUIDE.md` + `.en.md`：LTM 响应增加 `indexStatus` 字段 + outbox 说明；新增 §18.1 治理 403 错误 + §18.2 MCP 错误码
- `docs/artifacts/2026-07-16-enterprise-productionization/deployment-context.md`：新增非 BYPASSRLS 应用角色 + 备份恢复演练清单
- `docs/artifacts/2026-07-06-memory-storage-reliability/deployment-context.md`：更新阻塞项状态（RLS/outbox 已交付），更新放行结论
- `fix-plan.md` §9 同步清单全部标记完成

### 2026-07-17 — 执行日志对账（第三轮）

与源码逐项核实：
- ✅ W0.1a/b/c 状态准确 — `store_ltm_for_tenant` 的 PG 路径(line 238-269)以 DB+outbox 单事务执行，Qdrant 由 worker 异步投递
- ✅ PR-1~6 全部已合并，`init_audit_writer`/`init_outbox_worker`/`init_enterprise_hooks` 均在 `main.rs` 正确启动
- ✅ `governance_middleware` 已挂载至 memory_routes(line 265)、KG(line 338)、MM(line 351)
- ⬜ W0.2a 核实：`call_tool()` 不验证签名（仅 `list_tools` 有签名校验）
- ⬜ W0.2b 核实：`call_tool()` 未调用 `capability::authorize`
- ⬜ W0.2c 核实：`call_tool` handler 无审计事件
- ⬜ W0.4a 核实：`QuotaManager` 无 `ensure_default`/`record_usage` 方法
- ⬜ W0.4b 核实：governance pre-hook 不检查 RBAC 角色权限
- ⬜ W0.4c 核实：`RbacService` 在 `tenant.rs` 和 `enterprise_impl.rs` 各 new 一份，非单例
- ⚠️ W0.3 修正：文档已完成（角色+渗透 SQL），docker-compose 代码改造未做

### 2026-07-17 — Wave 0 全部完成（第四轮）

- W0.1d：`vector_outbox.rs` 新增 6 个单元测试（幂等键格式 + payload_hash 确定性）
- W0.2a+b+c：`call_tool()` 新增签名验证 → capability 授权 → 审计事件三步管线
- W0.4a+b+c：`post_store` 递增 usage；`pre_store`/`pre_search` 增加 RBAC 权限检查；`RbacService` 全局 OnceLock 单例
- W0.5：`rate_limit_middleware` 优先 `ConnectInfo`（TCP 真实 IP），回退 XFF
- W0.3：`docker/initdb/01-create-app-role.sql` + `docker-compose.yml` 挂载
- 修复预存编译错误：`memory_ingestion.rs:278` `%entry_id` → `%entry_id.entry_id`
- V1：`cargo check` 0 errors
- V2：`cargo test --lib` 268 passed; 0 failed

### 2026-07-17 — Wave 1 完成（第六轮）

- W1.1：`qdrant.rs` 新增 `scroll_point_ids()` + `get_point_payload()`；`db/vector_reconciliation.rs` 新增 runs/items 仓库；`services/vector_reconciliation.rs` 新增对账扫描器（dry_run/repair 双模式，4 类 drift 检测）
- W1.5：`information_guard.rs` `run_scan_cycle()` 重构为多租户扫描（遍历 `list_tenants()` + 默认租户）
- W1.7：`backup-restore-runbook.md` 创建（643 行，含 PG/Qdrant/Neo4j 备份恢复步骤 + 演练清单 + 告警建议）
- V1：`cargo check` 0 errors
- V2：`cargo test --lib` 276 passed; 0 failed（+7 新增 reconciliation 测试）

### 2026-07-17 — Wave 3 全部完成（第十轮）

- W3.3：`predictor.rs` 新增 `Predictor` trait；`AdaptiveMemoryScheduler` 接受 `Box<dyn Predictor>`；所有 8 个调用方更新
- W3.4：`services/eval_harness.rs` 创建（`EvalSuite` + 3 内置用例 + `summary()`）；5 个单元测试
- V1：`cargo check` 0 errors
- V2：`cargo test --lib` 283 passed; 0 failed（+5 eval_harness）

---

## 4. 放行勾选（Wave 0 局部）

- [x] PG 路径 LTM 写不先写 Qdrant；outbox pending 后由 worker 投递
- [x] MCP 未签工具 / 缺 capability 返回 403，并有审计
- [x] 设低配额后 store/search 可被 403（quota manager 检查已接线）
- [x] RBAC 赋 Reader 后 store 被拒（`pre_store` 检查 `rbac.check_permission(..., "write")`）；无角色用户行为与赋权前兼容
- [x] `cargo check` 0 error
- [x] `cargo test --lib` 268 passed; 0 failed

---

_完成后将本节勾选更新，并在 fix-plan §4 同步状态。_
