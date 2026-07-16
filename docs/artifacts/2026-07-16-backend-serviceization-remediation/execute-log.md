# Execute Log — 后端服务化审查与 P0/P1 修复

- **日期**：2026-07-16
- **主责角色**：backend-engineer
- **关联入口**：用户请求"重新审查整个后端的代码 + 服务化是否完成" → 分层审查 → 清理 P0 → P1 → 解决 a2a 构建阻塞 → F1–F6 审查发现修复
- **范围**：`backend/`（Rust/Axum，约 5 万行）安全默认值、数据层正确性、供应链/可构建性、审查发现的正确性/性能问题；不含 P2 产品决策项
- **验证方式**：静态审查 + rustfmt + **离线 `cargo check` / `cargo test --lib`（本轮已打通，见"验证证据"）**

---

## 结论先行

1. **服务化未完成**：REST 记忆服务（MVP 基座）真实可用；但"多协议（A2A/gRPC/WS）/自适应自进化/企业级可靠性"三大对外叙事在代码层均名实不符（详见分层审查结论，本文只记录已落地的修复）。
2. **本轮已修复并编译验证**：P0 安全默认值（鉴权/密钥/裸奔端点）、P1 数据正确性（事务边界 + 跨租户读泄漏）、P1.7 Qdrant 度量、MM 防御性加固、供应链（依赖 pin + Cargo.lock 纳管）、F1–F6 审查发现（跨租户计数泄漏、限流加固、信号串号、诚实性、扫描游标、rerank 并行）。
3. **a2a 构建阻塞已解除**：a2a 改为默认关闭的 feature 并注释其 git 依赖，默认后端**离线 `cargo check` 通过（0 error）**，从而首次对本轮全部改动完成编译验证。
4. **剩余**：重新启用 A2A（可选、需联网）；F2 的 XFF 欺骗（需改连接 IP，涉及反代语义，未擅动）；P2 产品决策项。

---

## 计划 vs 实际

| 计划（来自分层审查） | 实际 | 偏差原因 |
|---|---|---|
| 清理 P0（鉴权默认/裸奔端点/密钥/依赖 pin） | ✅ 全部完成 | — |
| P1 事务边界（5 个多步写） | ✅ 完成（5 函数/11 写） | — |
| P1 跨租户读泄漏（history/time-travel） | ✅ 完成（3 仓库函数 + 3 路由） | — |
| P1.7 Qdrant Euclid→Cosine | ✅ 完成（3 处配置） | — |
| MM update_entry 跨租户写 | ✅ 防御性加固完成 | 实为死代码路径（见关键决定），无活跃利用面 |
| P0.4 pin a2a + 提交 Cargo.lock | ⚠️ 部分：langchain 已 pin、Cargo.lock 已可离线生成并纳管；**a2a 依赖离线无法 pin**，改为 feature-gating 解绑 | 本机无 github 网络路由 |
| F1–F6 审查发现修复 | ✅ 全部完成并编译验证 | F2 的 XFF 欺骗未修（需改连接 IP，涉及反代语义） |
| 编译验证 | ✅ 由"离线不可编译"转为"离线 `cargo check` 通过 + 单测通过" | a2a feature-gating 的附带收益 |

---

## 关键决定

1. **MCP 鉴权冲突裁决（P0.2）**：两份子审查对 `/api/mcp/*` 是否鉴权结论相反。以代码为准核实 `routers/mcp.rs:140-149`——`/tools`、`/tools/call`、`/resources*` 已在自身 `auth_middleware` 之后，仅 `/initialize` 公开。**结论：MCP 无需改动**，P0.2 只处理 `data_io` 与 `/v1/tracing`。
2. **JWT 校验放 `main()` 而非 `config::init()`（P0.3）**：`init()` 被大量测试调用，若在此 `exit(1)` 会误伤测试；`main()` 是唯一生产入口且测试不经过它。校验逻辑抽为纯函数 `check_jwt_secret(disabled, secret)` 便于单测。
3. **a2a 用 feature-gating + 注释 git 依赖（而非 optional）**：实测 `optional = true` 仍会在解析阶段 fetch git 依赖 → 离线失败。因此把依赖注释掉（移出解析图）+ `#[cfg(feature="a2a")]` 门 + 占位 `a2a = []`。依据：A2A handler 目前是假数据壳，默认关闭不损失真实功能。
4. **Qdrant 改 Cosine 而非改打分方向（P1.7）**：Cosine 与检索层"高分优先/[0,1]/阈值过滤"的既有假设一致，改动最小；并注明**已存在集合需重建**才生效。
5. **MM update_entry 为防御性加固**：核实 `store_multimodal_memory` 零调用方（死代码），真实路径 `store_mm` 直接带租户建条目且不调 update_entry → 跨租户写无活跃利用面。仍按 MMRepository 约定补 `tenant_id` + JSON 租户条件。同类 `ltm.update_entry` 零调用方，标注"接线时再加固"，本轮不动。
6. **F6 rerank 并行化踩到 HRTB 坑（教训）**：初版用 `futures::buffer_unordered` 并发打分，触发 `Send is not general enough`（高阶生命周期），且报错不落在 rerank 本身、而是**级联污染所有 await 它的 axum handler**（如 `mcp.rs call_tool` 的 `&TenantId`），一度误判根因在别处。最终改用 **chunked `join_all`（每批 8）+ `score_relevance` 改关联函数收 owned 输入**，既保留有界并发又消除 HRTB。全靠每步 `cargo check` 才定位；`buffer_unordered` 在 axum handler 链中需谨慎。
7. **F3 signaling_bus 修正暴露既有测试固化了 bug 行为**：原全局广播下"多订阅者都收到"的测试其实固化了串号 bug；改成按 workflow_id 路由后同步修正测试，并新增"跨 workflow 不投递"回归测试。
8. **F2 只修可控项**：内存无界（加 `MAX_TRACKED_KEYS` 淘汰）与 `/login` 未限流（加 10/60s 独立限流器）已修；**XFF 欺骗未修**——正确解法要改用连接 IP，涉及反向代理语义与 `main.rs` server 装配，风险高，标注为后续项而非擅动。
9. **F4/F5 诚实性与低风险优先**：Neo4j 索引确为空操作但无法从代码推断 schema（不擅自猜标签建约束），故只让它 `warn` 未实现 + 移除误导性成功日志；information_guard 扫描器加滚动游标（仍限默认租户，多租户覆盖列为后续）。

---

## 阻塞与解决

- **阻塞**：拉取后 `8e6fd04` 引入未 pin 的 git 依赖 `a2a`/`a2a-server`（`a2a-rs.git`），`backend/Cargo.lock` 又被 gitignore，导致离线/内网/CI 无法构建；本机对 github 无网络路由（禁用沙箱亦不通）。
- **根因细节**：`optional` 依赖在 cargo 解析阶段仍需 fetch git manifest；缓存 db 为空且 lock 无 a2a 条目 → 离线解析必失败。
- **解决**：a2a 依赖注释 + feature-gating（默认关）。默认构建移出 a2a 后离线解析/编译通过；Cargo.lock 随之可离线生成（无 a2a、langchain 锁 `cfc709a`）并解除 gitignore。

---

## 影响面（按类别，file:line）

> 不含会话开始前已存在的本地改动：`docs/LATEST_TEST_REPORTS.md`、`docs/MVP_MEMORY_PLATFORM_FIXLIST.md`、`backend/.tsp/`。

### P0 安全
- `backend/docker.toml:13` —— `jwt.disabled true→false`（默认开启鉴权）
- `backend/src/routers/mod.rs`（api_router 装配）—— `data_io::router()` 与 `/v1/tracing` 套 `auth_middleware`
- `backend/src/config/mod.rs` —— 新增 `validate_jwt_security` / `check_jwt_secret` + 常量 denylist + 4 条单测
- `backend/src/main.rs` —— config 加载后 fail-fast 校验 JWT 密钥
- `backend/Cargo.toml` —— langchain-rust `rev=cfc709a…`；a2a 依赖注释 + `a2a` feature 占位
- `.gitignore` —— 解除 `backend/Cargo.lock` 忽略
- `.env.example` / `docker-compose.yml` —— 移除弱默认 `JWT_SECRET`，compose 改 `${JWT_SECRET:?…}` 强制设置；healthcheck 改用公开端点 `/`

### P1 数据正确性
- 事务边界：`backend/src/db/stm.rs`（add_message、delete_session）、`backend/src/db/kg.rs`（create_relation、supersede_entity）、`backend/src/db/ltm.rs`（supersede_entry）
- 跨租户读泄漏：`backend/src/db/ltm.rs`（get_entry_history）、`backend/src/db/kg.rs`（get_entity_at_time、get_entity_history）加 `tenant_id` + 前缀过滤 + 违规记录；`backend/src/routers/memory_search.rs`（get_ltm_history、get_kg_entity_at_time、get_kg_entity_history）加 `RequestTenantContext` 提取并下传
- P1.7 Qdrant：`backend/src/config/qdrant_config.rs`、`backend/config.toml`、`backend/docker.toml` —— 默认度量 `Euclid→Cosine`
- MM 加固：`backend/src/db/mm.rs`（update_entry 加 `tenant_id` + JSON 租户条件）、`backend/src/services/multimodal_memory.rs`（调用方补参）

### 构建/供应链
- a2a feature-gating：`backend/src/main.rs`、`backend/src/lib.rs`、`backend/src/routers/mod.rs`（`#[cfg(feature="a2a")]`）
- `backend/Cargo.lock`（新增，未跟踪，待 `git add`）
- `backend/src/a2a/{agent_card,handler,router,streaming}.rs` —— 纯 rustfmt 格式化（清理 8e6fd04 未 fmt 的漂移）

### F 系列（审查发现修复）
- **F1** 跨租户计数泄漏 + 假健康：`backend/src/services/memory_fusion.rs` —— `count_mm` 加 JSON 租户过滤；`get_status` 健康位改为"count 查询是否成功"（原 `count>=0` 恒真）
- **F2** 限流加固：`backend/src/hoops/rate_limit.rs`（`MAX_TRACKED_KEYS` 上限 + 超限淘汰过期 key）、`backend/src/routers/mod.rs`（`/login`、`/login/account` 加 10/60s 独立限流器）
- **F3** 信号串号：`backend/src/distributed/signaling_bus.rs` —— 由单一全局 broadcast 改为按目标 `workflow_id` 的 per-workflow 通道；修正原固化 bug 的测试 + 新增"跨 workflow 不投递"回归测试
- **F4** Neo4j 诚实性：`backend/src/db/neo4j.rs`（`init_neo4j_indexes` warn 未实现）、`backend/src/main.rs`（移除误导性"initialized successfully"日志）
- **F5** 扫描游标 + 异步写：`backend/src/services/information_guard.rs`（`SCAN_OFFSET` 滚动游标，不再复扫同 50 行）、`backend/src/services/memory_storage.rs`（journal 写用 `spawn_blocking` 卸载出异步热路径）
- **F6** rerank 并行：`backend/src/services/rerank.rs` —— chunked `join_all`（每批 8）并发打分，`score_relevance` 改关联函数收 owned 输入（规避 HRTB Send）

**数据库/接口影响**：无 schema 变更、无接口签名对外变化（仓库内部函数签名新增 `tenant_id` 参数，调用方已同步）。事务改动只影响失败路径（回滚），成功语义不变。F3 改动了 `SignalingBus` 内部结构（去掉全局 `sender` 字段），公共方法签名不变。

---

## 验证证据

- `SQLX_OFFLINE=true cargo check --offline`（默认 features）：**`CARGO_RC=0`，0 error**（含 P0/P1/F1–F6 全部改动）。
- `cargo test --offline --lib`：`config::` 6 passed、`signaling_bus::` 5 passed（含新增跨 workflow 回归测试）、`rate_limit::` 5 passed，**全绿**。
- `rustfmt --check`：所有本轮改动的 `.rs` 文件 clean。
- 触及本轮文件的编译告警均为**既有 dead-code**（`never used`/`never read`），无本轮新增。
- `Cargo.lock`：无 a2a 条目、langchain 锁定 `cfc709a71fee02083ced078252d73c1f76fe9849`。
- **F6 过程记录**：初版 `buffer_unordered` 触发 HRTB `Send is not general enough`，`cargo check` 报 11 error（级联到 axum handler），换 chunked `join_all` 后归零——见"关键决定 #6"。
- ⚠️ 未验证：`--features a2a` 路径（需联网拉 a2a 依赖，本机无网络）。

---

## 未完成项

1. **（可选/需联网）重新启用 A2A**：uncomment+pin `backend/Cargo.toml` 的 a2a 依赖 → `a2a=["dep:a2a","dep:a2a-server"]` → `cargo build --features a2a`。启用前应先把 handler 接真实服务（当前为假数据壳）。
2. **F2 XFF 欺骗未修**：限流 key 仍取客户端可控的 `x-forwarded-for`，可轮换绕过。正确解法要改用连接 IP（`ConnectInfo`），涉及反向代理语义与 `main.rs` server 装配，风险较高，留作后续。
3. **F5 多租户扫描覆盖**：完整性扫描器现仍限默认租户（仅补了滚动游标），多租户覆盖列为后续。
4. **P2 产品决策项**：A2A 真实实现 / gRPC·WS 死代码 / MCP 沙箱 mock / 分布式 stub 假集群 / "自适应·自进化"名实不符 / LTM↔Qdrant outbox·对账接线 / 企业治理 hooks 接入 / DB 备份·HA·告警·回滚。
5. **企业级可靠性 remediation**：schema 级租户隔离/RLS、outbox 落地等（对应 `docs/artifacts/2026-07-06-memory-storage-reliability/`）本轮未触及。

---

## 行为变更须知

1. `cargo run` 与 `docker compose up` 现在**要求强 `APP_JWT_SECRET`**（占位/过短/未设将 fail-fast），除非显式 `jwt.disabled=true`（仅 loopback dev）。
2. Docker 部署默认**开启鉴权**；此前依赖无鉴权的 demo/E2E 流程需带 token。
3. `/api/data/export`、`/api/data/import`、`/api/v1/tracing/*` 现需 JWT。
4. compose healthcheck 改打公开端点 `/`（原 `/api/v1/memory/health` 已在鉴权层内）。
5. Qdrant 默认度量 Cosine；**已存在集合需 drop 重建**方生效。
6. A2A 路由默认**不编译**；需 `--features a2a`。
7. `/login`、`/login/account` 现有 **10 次/60 秒**限流（防爆破）；超限返回 429。
8. rerank 现按每批 8 并发调用 Ollama 打分（原为逐个串行）；Ollama 侧瞬时并发上升，延迟下降。

---

## 联网收尾步骤（P0.4 / a2a）

```bash
cd backend
# 1) 提交已可离线生成的 lock（无 a2a、langchain 已 pin）
git add backend/Cargo.lock

# 2)（可选）重新启用 A2A：
#    - uncomment Cargo.toml 的 a2a/a2a-server 依赖
cargo update -p a2a-lf -p a2a-server-lf          # 解析出 commit
grep -A2 'a2a-.*-lf' Cargo.lock                   # 复制 commit 填入 rev
#    - 把 feature 改回 a2a = ["dep:a2a","dep:a2a-server"]
cargo build --features a2a                        # 验证 A2A 路径编译
```
