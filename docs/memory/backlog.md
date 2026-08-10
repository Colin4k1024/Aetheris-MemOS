# Backlog

> 跨任务事实源。任何遗留项、技术债、下一阶段候选以本文为准，不再只存在于
> release 备注、artifact 或对话中。
>
> - 建立日期：2026-08-10
> - 建立来源：`docs/artifacts/2026-08-10-truthfulness-and-security-remediation/closeout-summary.md`
> - 更新角色：`tech-lead`

---

## A. 已拍板决策：7 项「声明 vs 实现」缺口 → 决定为**做**（不删）

2026-08-10 由 tech-lead 拍板：这 7 项一律**补齐实现**，而非删除声明。
下表按**实施成本升序**排列，即建议推进顺序。

| 顺序 | 项 | 现状 | 工作内容 | 量级 |
|---|---|---|---|---|
| 1 | **D-5** MCP capability 最小权限 | `capability::authorize` 已接入请求路径，但 `granted` 对**所有调用方硬编码** `[Read, Write, Delete]`（`routers/mcp.rs:289-293`） | 改为按主体（JWT 主体 / 租户 / 角色）解析授予集合，未声明即拒 | S |
| 2 | **D-4** MCP wasm 沙箱接入 | `mcp/sandbox.rs` 是**真的 wasmtime 实现**（含 fuel 限制、capability 策略），但零引用；`call_tool` 原生执行工具 | 按 ADR-0004 的「可信第一方平面 + 不可信扩展平面」双平面模型接线；第一方工具保持原生，不可信扩展走沙箱 | M |
| 3 | **D-6** open-core 真 feature gating | `feature = "enterprise"` 在 `src/` 出现 **0 次**，gates 不了任何代码；billing 等所谓 Enterprise 模块默认编入 MIT 二进制 | 给 billing / RBAC / governance / 可视化加 `#[cfg(feature = "enterprise")]`；CI 增加「默认构建」与「enterprise 构建」双通道，防止 gate 漂移 | M |
| 4 | **D-3** A2A | `#[cfg(feature = "a2a")]` + `default = []`；git 依赖需联网解析；`a2a_integration.rs` 的 7 个测试**从不运行** | pin 依赖 rev、纳入 CI 构建验证、让 7 个测试真跑 | M |
| 5 | **D-1** gRPC + WebSocket 真实化 | `grpc_auth_interceptor` **零调用者**、无 tonic server、无 `.proto`、无 `build.rs`；`ws_upgrade_handler` **从未挂载**，内层 handler 是 TODO（连上即 Close 1008） | 按 ADR-0007：补 `.proto` + tonic server + service impl；挂载 WS upgrade 路由并实现真实 handler；两者复用 `hoops::jwt::authenticate()` 统一鉴权核心 | L |
| 6 | **D-2** `kernel/` + `layers/` 接真实存储 | 整套 trait 抽象设计良好但**线上主链路完全绕开**；`layers/*.rs` 是 `RwLock<HashMap>` 内存 stub，`ltm_layer` 的 search 是 `contains()` 子串匹配、score 恒 1.0 | 把 `layers/*` 后端从内存换为 `db::*Repository`；让 `routers/procedural.rs` 的 GraphRAG 不再跑在空 stub 上；此后 `hybrid_search.rs` 的正确 RRF 实现才有真实输入 | L |
| 7 | **D-7** P3 自适应（按 ADR-0008） | analyzer / predictor / scheduler / weight 全为写死系数，**零遥测学习**；`TrainablePredictor::fit_from_samples` 只有声明无实现；`config_recommendation.rs` 零调用者且头部「基于规则+历史数据」为**虚假声明** | ⚠️ **有前置条件**：ADR-0008 明确要求「先由 eval harness 证明自适应显著优于静态最优配置，之后才放行」。而 `eval_harness.rs::run` 当前硬编码 `passed:true`、`coherence:1.0`、从不调用 scheduler。**故第一步必须是把 eval harness 做成真的**，再谈离线拟合 | XL |

**D-7 补充**：无论最终是否放行学习闭环，`config_recommendation.rs` 头部那句
「基于规则 + 历史数据 / training_samples」都必须立即修正——它从不查询
`training_samples`，属虚假声明。

---

## B. 生产阻塞项（未完成，优先级高于 A）

| 项 | 现状 | 来源 |
|---|---|---|
| **审计不可靠** | fire-and-forget，队列满 / 通道关 / 未初始化均**静默丢弃 + 计数**，无重试无背压；只在 Postgres 下启动，**SQLite 下审计全丢**。当前不能作为合规日志 | W3-7 |
| **无告警、无 dashboard** | 指标质量不错（`outbox_pending_total`、`outbox_dead_letter_total`、`tenant_quota_usage_ratio` 均已导出），但 `monitoring/` 里**一条告警规则都没有**，Grafana 只配 datasource。outbox 堆积、死信增长、鉴权失败均无人告警 | W4 |
| **liveness / readiness 探针不存在** | CLAUDE.md 与 README 都声称有，实际没有；`/api/v1/memory/v1/health` 返回 `self_healing` 的**硬编码假数据**（`healthy:true` + 1–4ms 假时延），运维会误信 | W4 |
| **对账扫描器零调用者** | `services/vector_reconciliation.rs` 已完整实现（missing / orphan / tenant_mismatch / content_hash_mismatch），但无守护进程、无路由、无任何调用点。它正是 outbox 投递失败后的兜底 | W4 |

---

## C. 治理与多租户缺口

| 项 | 现状 |
|---|---|
| 治理层未覆盖多个路由 | `/billing`、`/tenants`、`/snapshot`、`/memory-pool`、`/v1/agents` 有鉴权但**无角色门禁**，计费与租户管理无门禁 |
| `claim_batch` 无租户公平性 | outbox 消费**进程全局**，一个租户堆积会拖慢**所有**租户的向量索引。当前设计如此，非 bug |
| RBAC 尚不能区分权限 | 每人都是自己单用户租户的 Owner。真正的角色区分需要 **org 层租户模型**（`tenant_id` 与 `user_id` 解耦） |
| SQLite 模式无任何 DB 层隔离 | 且 `db.url` 为空时会**静默降级**到 SQLite（`config/mod.rs:129-138`） |

---

## D. 实测撞出的候选（非代码审查所得）

1. **DB check 约束以 500 而非 400 暴露给调用方**：`session_type` 仅接受 `conversation`/`task`/`query`，`source_type` 仅接受 `document`/`api`/`database`/`web`/`user_input`；传非法枚举值得到「内部错误」，集成方无从排查
2. **`config.toml` 与 `local.toml` 的 embedding 模型不一致**（768 vs 1024），切换配置必然触发 `vector_guard` 并拒绝启动
3. **`vector_guard` 错误提示不完整**：建议「Drop the Qdrant collection and re-index」，但签名实际存于 `~/Library/Application Support/adaptive-memory/vector_signatures.json`，只删 collection 无法恢复
4. **Neo4j「可选」在运行上不成立**：`init_neo4j_indexes` 确是 best-effort，但 neo4rs 重试退避**同步阻塞启动主线程**，错密码能让服务器几十秒起不来；占位符密码不像 JWT 那样 fail-fast
5. **Ollama 不可达时 LTM 写入完全不可用**（依赖 LLM 摘要，返回 500）
6. **两个 doc 示例编译不过**：`tenant/context.rs:103`、`runtime/langchain_adapter.rs:157`，标了 ```` ```ignore ```` 掩盖了这一点
7. **「参数被接受、被忽略、无报错」需全面排查**：`sdks/rust` 的 `list_sessions` 曾接收 `user_id`/`limit` 却完全忽略（已修），同类模式可能还有
8. **8 份 ADR 全为 `Proposed`**，代码已按其中数份实现，需补 Design Review 收口
9. **统一 query 参数命名 + 契约测试**（根因项）：7 个 `*Query` 结构体中 3 个 rename 为 camelCase、4 个保持 snake_case，Axum 匹配不上即静默丢弃。已直接造成 4 个 bug，只修症状会复发

---

## E. 技术债（较低优先）

- 手写 OpenAPI → utoipa 宏自动生成（当前只覆盖 MVP 子集，Scalar 不是权威清单）
- 双 JWT：`web/jwt.rs` 已标 DEPRECATED、接受 query-string token、不注入租户上下文，仍被编译
- `ci.yml:136` 前端 `npm run lint` 仍 `continue-on-error: true`
- `sdks/rust` 新增 `urlencoding` 依赖，`reqwest` 的 `.query()` 可免掉
- `search_knowledge_by_entity_for_tenant` 中 `LEFT JOIN knowledge_entries ke` 的 `ke` 在 SELECT/WHERE 中从未引用，只会放大行数（靠 `DISTINCT` 兜着）

---

**最后更新**：2026-08-10
**更新角色**：tech-lead
