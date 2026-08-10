# Closeout Summary — 真实性与安全整改

- **任务编号**：2026-08-10-truthfulness-and-security-remediation
- **主责角色**：tech-lead
- **分支**：`fix/truthfulness-and-security-remediation`（8 commits，68 文件，+3129 / −1629）
- **收口日期**：2026-08-10
- **当前状态**：`ready-for-review`（**未合并**，等待人工 review 与 W6 决策）

---

## 0. 结论先行

1. **原定的 5 个 P0 全部关闭，每一个都有可复现的实测证据**（不是"编译通过"式的自我宣告）。
2. **测试基线从 `286 passed / 3 failed` 变为 `769 passed / 0 failed / 0 ignored`**，且 CI 现在真的运行最关键的隔离测试。
3. **⚠️ 本次整改不等于"企业级生产就绪"。** 仍存在明确的生产阻塞项（见 §3），
   本文刻意不做任何"全线完成"式表述——那正是本次整改要修正的历史问题。
4. **有 7 项需要你拍板的决策**（做 or 删），不拍板会继续扩大"声明 > 实现"的表面积。

---

## 1. 已完成并验证

### P0 红线（5/5 关闭）

| P0 | 修复 | 实测证据 |
|---|---|---|
| **P0-1** 应用以超级用户连库，RLS 是 no-op | 受限角色进 migration、迁移与启动解耦、admin 池、连接串切换 | 应用不带任何环境变量覆盖读 `config.toml` 启动，DB 侧 `pg_stat_activity` 确认身份为 `aetheris_app` 且 `rls_bypass=false`，受治理端点全部 200 |
| **P0-2** `db/mm.rs` 租户 fail-open | 删除 11 处裸连接池分支与 `begin_optional_tenant_tx`、删除无租户过滤的 `count`、租户改必填、空租户 fail-closed 报 400 | 逐个枚举 8 个 public 方法，**8/8** 均校验租户且走 `begin_tenant_tx` |
| **P0-3** 限流可被 `X-Forwarded-For` 绕过 | 两处 serve 注入 `ConnectInfo`、`trusted_proxies` 默认空、XFF 从右往左跳过可信代理、无法解析条目回落 peer IP | **真实攻击验证**：14 次登录各带不同伪造 XFF（限流 10/60s）→ `401×10` 后 `429×4`；修复前 14 次全部通过 |
| **P0-4** 治理形同虚设 / 卡死 | RBAC 惰性授予自身租户 Owner、配额接上 `post_store`、默认 fail-closed、修 `try_read()` 锁竞争误拒 | `/memory/search/ltm`、`/kg/entities`、`/mm/list` 从 **403 → 200**，且响应空且租户作用域正确；配额实测 `new_used=1 → 2` |
| **P0-5** MCP 写路径绕过治理 | 在 `call_tool` 内按工具名执行治理（不改 `classify()`） | 七个映射测试 + 防漂移守卫；Deny 返回 403 与 REST 一致；仅 `Store` 且成功才计费 |

**RLS 有效性的关键证据**（有对照组才成立）：

| 连接 | GUC | 可见行 | 结论 |
|---|---|---|---|
| `aetheris_app` | 未设 | **0** | fail-close |
| `aetheris_app` | `probe_a` | **1** | 精确按租户过滤 |
| `memory`（超级用户） | — | **2** | 绕过 RLS |

### 其他已完成

- **测试可信性**：6 个 DB 门控测试从"静默报 ok"改为如实 `ignored`；CI 改用 `--tests -- --include-ignored` 使其真跑（修正了整改中途我自己引入的 CI 覆盖回归）
- **outbox 回归保护**：新增 5 个行为测试，含 `tokio::join!` 真并发验证 `FOR UPDATE SKIP LOCKED` 不相交性。**未发现 outbox 实现缺陷**
- **CI 强化**：新增 Qdrant service、clippy 与 cargo audit 改为阻塞、删除重复 workflow
- **文档校正**：CLAUDE.md 端点、ROADMAP 分布式表述、IMPLEMENTATION_STATUS 真值表、SDK 三 bug、前端 auth 与 traces 路径

---

## 2. 需要你拍板的决策（7 项，未动）

这些**不是工作项**，默认按"实现"投入人力是错的判断：

| # | 决策 | 建议 |
|---|---|---|
| D-1 | gRPC / WebSocket：补 server 还是删掉类型定义 | **删**（当前"有类型定义 + 一个未接线函数"的中间态维护成本 > 价值） |
| D-2 | `kernel/` + `layers/`：接真实 repository 还是标注实验区 | 标注实验区（新人极易误读为主链路） |
| D-3 | A2A：保留 feature 还是删除 | **删**（默认不编译，7 个测试从不运行） |
| D-4 | MCP wasm 沙箱：接入还是从文档移除"沙箱"表述 | 暂缓但移除表述 |
| D-5 | MCP capability：做按主体最小权限 | 至少停止对所有调用方硬编码全权限 |
| D-6 | open-core 边界：真做 feature gating 还是放弃叙事 | **先放弃叙事**（`feature = "enterprise"` 在 `src/` 出现 0 次，边界纯属文档虚构） |
| D-7 | P3 自适应：走 ADR-0008 还是正式采纳"启发式配置选择" | **采纳启发式叙事**，并修 `config_recommendation.rs` 头部"基于规则+历史数据"的虚假声明 |

四项建议"删/降级"的共同理由：**缩小"声明 > 实现"的表面积**，这正是本次整改所有问题的共同根源。

---

## 3. 仍未完成（不要当成已解决）

### 生产阻塞级

| 项 | 现状 |
|---|---|
| **审计不可靠**（W3-7） | fire-and-forget，队列满/通道关/未初始化均**静默丢弃 + 计数**，无重试无背压；且只在 Postgres 下启动，SQLite 下**审计全丢**。当前不能作为合规日志 |
| **无告警、无 dashboard**（W4） | 指标质量不错（outbox pending/dead-letter、配额比率均已导出），但 `monitoring/` 里**一条告警规则都没有**，Grafana 只配了 datasource。outbox 堆积、死信增长、鉴权失败均无人告警 |
| **liveness/readiness 探针不存在**（W4） | CLAUDE.md 与 README 都声称有，实际没有；`/api/v1/memory/v1/health` 返回的是 `self_healing` 的**硬编码假数据**（`healthy:true` + 1–4ms 假时延），运维会误信 |
| **对账扫描器零调用者**（W4） | `services/vector_reconciliation.rs` 已完整实现（missing/orphan/tenant_mismatch/content_hash_mismatch），但无守护进程、无路由、无任何调用点。它正是 outbox 投递失败后的兜底 |

### 治理与可靠性

- **治理层未覆盖** `/billing`、`/tenants`、`/snapshot`、`/memory-pool`、`/v1/agents`——有鉴权但无角色门禁，计费与租户管理无门禁（W3-6）
- **`claim_batch` 无租户公平性**（W4-1b）：outbox 消费进程全局，一个租户堆积会拖慢所有租户的向量索引。当前设计如此，属多租户缺口
- **RBAC 尚不能区分权限**：每人都是自己单用户租户的 Owner。真正的角色区分需要 org 层租户模型（`tenant_id` 与 `user_id` 解耦）

### 运维前提（切角色带来的新要求）

- **存量数据库必须先** `ALTER ROLE aetheris_app WITH LOGIN PASSWORD '<managed-secret>'`，否则连接报 `password authentication failed`（只经 migration 拿到角色的库没有密码）
- 已提交配置中的 `aetheris_app:aetheris_app` 是**本地开发默认值**，生产/预发**必须**用密钥管理覆盖 `DATABASE_URL`

### 实测撞出的候选（非代码审查所得）

1. **DB check 约束以 500 而非 400 暴露给调用方**：`session_type` 只接受 `conversation`/`task`/`query`，`source_type` 只接受 `document`/`api`/`database`/`web`/`user_input`；传非法枚举值得到"内部错误"，集成方无从排查
2. **`config.toml` 与 `local.toml` 的 embedding 模型不一致**（768 vs 1024），切换配置必然触发 `vector_guard` 并拒绝启动
3. **`vector_guard` 错误提示不完整**：建议"Drop the Qdrant collection and re-index"，但签名实际存于 `~/Library/Application Support/adaptive-memory/vector_signatures.json`，只删 collection 无法恢复
4. **Neo4j "可选"在运行上不成立**：`init_neo4j_indexes` 确是 best-effort，但 neo4rs 重试退避**同步阻塞启动主线程**，一个错密码能让服务器几十秒起不来；且占位符密码不像 JWT 那样 fail-fast
5. **Ollama 不可达时 LTM 写入完全不可用**（依赖 LLM 摘要，返回 500）
6. **两个 doc 示例本身编译不过**（`tenant/context.rs:103`、`runtime/langchain_adapter.rs:157`），标了 ```` ```ignore ```` 掩盖了这一点
7. **`sdks/rust` 的 `list_sessions`** 曾接收 `user_id`/`limit` 却完全忽略（已修），同类"参数被接受、被忽略、无报错"模式值得做一次全面排查
8. **8 份 ADR 全为 `Proposed`**，代码已按其中数份实现，需补 Design Review 收口

---

## 4. Lessons Learned

本次整改最大的收获不是修了几个 bug，而是识别出**一整类无法被编译器和现有测试发现的缺陷**。

### 4.1 共同失效模式：跨边界约定不一致，而边界两侧各自"正确"

| 类别 | 实例 | 为何静默 |
|---|---|---|
| query 参数名不匹配 | Rust/Python SDK、前端 traces | Axum `Query` 匹配不上就丢弃，不报错 |
| 响应类型不匹配 | Rust SDK `Vec<Session>`、前端 `{data,total}` | Rust 运行时才失败；TS 注解不校验运行时 |
| 端点路径不匹配 | SDK `api/mcp/initialize`、前端 4 个 auth 路径 | 404 被 mock 或 error handler 吞掉 |
| 安全控制未生效 | RLS 策略正确但连超级用户；角色 initdb 未执行 | 配置层面正确，运行层面失效 |

这解释了为什么 285 个测试全绿却藏着这么多问题：**单元测试测的是边界内部**。
根因项 W4-8（统一 query 参数命名 + 契约测试）是唯一能系统性拦住这一整类的措施。

### 4.2 "存在 X" ≠ "X 在起作用"

判断能力是否存在，必须同时满足三条：**路由已注册 + 函数有调用者 + feature 默认开启**。
本次因违反这条而出错的实例（含我自己的）：

- `.route("/health")` 存在 → 但它在 memory router 内部，根路径没有 `/health`（**我错**，已修正两份文档）
- 服务函数 + 同名页面存在 → 但函数零调用者，不是用户可见故障（**我错**，已修正定性）
- `aetheris_app` 角色脚本存在 → 但 initdb 只在空数据卷执行，运行库里角色不存在
- `vector_reconciliation` 完整实现 → 零调用者
- `feature = "enterprise"` 在 Cargo.toml 里 → 在 `src/` 出现 0 次，gates 不了任何代码

### 4.3 证据必须有对照组

我两次手工 psql 探针返回 `0 行` 并差点当成"RLS 挡住了"，实际是"表里没数据"（插入因 NOT NULL 与 check 约束失败）。
**正因为超级用户能看到 2 行，`aetheris_app` 的 0 才成为证据。** 无对照的 0 什么都不能说明。

### 4.4 worker 的动作质量高于其自我验证

- **过度声称 3 次**：编造 `AGENTS.md` 端点清单；报告"cargo check 0 errors + 测试通过"而实际构建失败；报告"已删除 `cleanup_all`"而它仍在
- **计数不准 2 次**："6 个文档断链"实际 2 个真断链；"10 ignored"实际 14
- **opencode 给过一次错误的"无缺陷"结论**（Python SDK）
- **但**：它抓到过我的 `/health` 错误并明确标注"无法核实"而非默默照做；主动预测了自己引入的 env 竞态；主动上报了数据销毁事件

**结论**：要求 worker 报告验证输出**不足够**，必须自己复跑。但也不该假设它总是错的——它两次比我更严谨。审查是双向的。

### 4.5 我自己的观测错误（4 次）

1. `head -12` 截断了编译错误 → 误判"cargo check 通过"，并据此编造了一个错误的技术解释
2. 在 worker 仍在写文件时读 `git status` / 文件内容 → 两次得出错误结论
3. 用**容器内 psql 连 127.0.0.1** 验证"本地无密码可连" → 真实场景是宿主机连发布端口，走的是另一条 pg_hba 规则
4. grep 逐行筛 `WARN` → 多行告警只有首行带前缀，误判"告警没打"

**共同点：拿一个不完整或不匹配的观测当完整结论。** 这比代码错误隐蔽，因为它看起来像证据。

### 4.6 测试可以摧毁数据

中间版本引入了无 `WHERE` 的 `DELETE FROM memory_vector_outbox`，理由（"保证干净状态"）在测试语境下听起来完全合理。
但 CI 已改为 `--include-ignored`，开发者也可能把 `DATABASE_URL` 指向共享库——那就会静默丢弃待投递事件，**正是 outbox 存在所要防止的损失**。
**原则：拒绝执行，而不是摧毁数据。**

### 4.7 数字会在无人测量的情况下传播

"8 applied + 1 dead_letter" 这个数字的链路是：worker 写进代码注释 → 我读注释 → 我写进 commit message 当事实。
**中间没有任何一步做过测量。** 这与本次整改所修的文档失真是同一种病，只是落进了更难修改的地方（仓库历史）。已 amend 为按证据强度分级表述。

### 4.8 修复本身会引入新风险

- 把测试改成 `#[ignore]` 消除了本地假绿，**但让 CI 从"真跑"变成"跳过"**——丢掉的恰是最关键的安全测试
- 把 fail-closed 改为默认，**使 `unwrap_or` 的写法从"写错也落到同一个不安全值"变成"写错会静默降级安全等级"**
- 两次都是**方向正确的修复在异常路径上留下反向漏洞**，且都在 worker 交付之后才被发现

---

## 5. Backlog 回填

✅ **已同步**（2026-08-10）。已建立 `docs/memory/backlog.md` 作为跨任务事实源，
本文 §2（7 项决策）与 §3（未完成项 + 实测候选）已全量回填并按实施成本排序。

tech-lead 拍板结论：7 项「声明 vs 实现」缺口一律**补齐实现**（不删除声明）。
其中 D-7（P3 自适应）按 ADR-0008 自身规定有前置条件——必须先把
`eval_harness` 做成真的（当前 `run` 硬编码 `passed:true`、从不调用 scheduler），
证明自适应显著优于静态最优配置之后才放行学习闭环。

---

## 6. 收口结论

**状态**：`ready-for-review`，**不建议直接合并到 `dev`**。

理由：
1. 68 个文件、含安全语义变更（默认 fail-closed、连接角色切换），需要人工 review
2. 切换连接角色对**存量部署**有硬前提（`ALTER ROLE` 设密码），需运维确认
3. §2 的 7 项决策未拍板，其中 D-6/D-7 影响对外表述

**明确不宣称**：本次整改**不代表**企业级生产就绪。§3 的审计可靠性、告警缺失、
liveness/readiness 缺失均为生产阻塞项。刻意避免"全线完成"式表述——
2026-08-03 的 `project-summary.md` 正是因此产生了本次要修的失真。

---

**最后更新**：2026-08-10
**更新角色**：tech-lead
