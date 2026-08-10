# Delivery Plan — Aetheris MemOS 真实性与安全整改

- **任务编号**：2026-08-10-truthfulness-and-security-remediation
- **主责角色**：tech-lead
- **输入依据**：`docs/TEAM_TECH_WALKTHROUGH.md`（源码逐条核对结论，基线 `dev` @ `a9a3e5a`）
- **当前阶段**：`plan`
- **成文日期**：2026-08-10

---

## 0. 结论先行

1. 本计划覆盖 walkthrough 中列出的**全部问题**，分 **7 个 Wave**，其中 Wave 0-3 是上生产前的硬门槛。
2. **顺序不可随意调换**。有三处强依赖，调换会导致"修了但证明不了"或"修了反而把生产打挂"（见 §2 依赖图）。
3. **发现一个此前未被识别的功能性 P0**：当前在"鉴权打开 + 治理打开"的正确生产配置下，
   核心记忆接口会全部返回 `403` —— 系统只有在 `jwt.disabled` 或治理被跳过时才可用。见 W3-1。
4. **有 7 项不是工作项而是决策项**（做 or 删），需要先拍板再排期，不要默认按"实现"投入人力。见 §5。
5. **本计划刻意不给日期**。上一轮 `2026-08-03` 的教训是"1 天完成 P0-P3"式的自我宣告，
   本计划改用**证据化放行标准**：每个 Wave 有明确的可执行验证命令，跑不出证据不算完成。

---

## 1. Wave 总览

| Wave | 主题 | 性质 | 相对工作量 | 阻塞生产？ |
|---|---|---|---|---|
| **W0** | 让验证成为可能（测试真实性 + CI） | 前置条件 | S | ✅ 是（其余全部依赖它） |
| **W1** | 租户隔离真正生效 | 安全红线 | M | ✅ 是 |
| **W2** | 边界防护（限流 / 注册） | 安全红线 | S | ✅ 是 |
| **W3** | 治理从"形同虚设/卡死"变成可用 | 安全 + 功能红线 | L | ✅ 是 |
| **W4** | 可靠性与可观测补齐 | 运维就绪 | M | ⚠️ 强烈建议 |
| **W5** | 对外一致性（文档 / SDK / 前端漂移） | 可信度 | S-M | ❌ 否 |
| **W6** | 决策项收口（做 or 删） | 治理 | 决策为主 | ❌ 否 |

---

## 2. 依赖关系（三处不可调换）

```
W0（测试可信）
  └──> W1（切 DB 角色）        ← 没有 W0，RLS 修复无法证明
         └──> W1 内部严格有序：
              mm.rs fail-open  →  管理员扫描路径  →  切角色  →  跑渗透测试

W3 内部严格有序（顺序反了会把生产打挂）：
   角色分配/播种  →  配额计数  →  启用强制  →  MCP 纳入治理
   (先让 RBAC 能通过)          (先让配额能计数)   (再收紧)     (最后扩面)
```

**三条必须遵守的顺序约束：**

1. **W0 必须在 W1 之前**。RLS 渗透测试目前 `DATABASE_URL` 未设置时静默跳过并报 ok
   —— 不先修这个，切完角色也拿不到"隔离生效"的证据，等于白改。
2. **W1 内部：`db/mm.rs` 和管理员扫描路径必须在切角色之前**。
   先切角色会：MM 层依然漏（fail-open 路径绕开 RLS）+ 两条管理员跨租户扫描被策略挡住导致回填/KG 搜索挂掉。
3. **W3 内部：角色分配必须在启用强制之前**。当前强制已经在代码里生效但角色无处分配
   —— 这正是"鉴权打开就 403"的根因。顺序反了等于把线上打挂。

---

## 3. 工作拆解

### W0 — 让验证成为可能（前置）

| # | 工作项 | 依据 | 量级 |
|---|---|---|---|
| W0-1 | 消除"假绿"测试：6 个 DB 门控集成测试（`rls_isolation_pg`、`rls_kg_pg`、`rls_mm_pg`、`rls_stm_pg`、`tenant_scope_pg`、`memory_platform_e2e`）改为 `#[ignore]` 或缺环境时显式 fail，禁止 early-return 报 ok | 最有价值的安全测试目前贡献 0 证据 | S |
| W0-2 | `db/tenant_scope.rs:71` 的 `.expect("Failed to connect to test database")` 改为优雅跳过 | 本地无 DB 时 3 个测试硬 panic，导致 `cargo test` 永远是红的，团队会习惯性忽略 | S |
| W0-3 | CI 增加 Qdrant service（PG 已有），让 outbox→Qdrant 路径进入自动化 | CI 目前只起 PG | S |
| W0-4 | `clippy` 与 `cargo audit` 去掉 `continue-on-error: true`，改为阻塞 | 现在两者都不拦任何东西 | S |
| W0-5 | 合并 `ci.yml` 与 `backend-ci.yml`（两个 workflow 高度重复，会漂移） | — | S |
| W0-6 | 给 `tests/tenant_isolation.rs` 加显著注释：它只验证 `TenantId::prefix()` 字符串格式，**不是**数据库隔离 | 极易被误读为隔离覆盖 | XS |

**W0 放行标准**
```bash
cd backend && cargo test 2>&1 | tail -20     # 本地无 DB 时应为全绿（跳过的显式标 ignored）
DATABASE_URL=postgres://... cargo test       # 有 DB 时 RLS 测试真实执行且可见
```
- CI 日志中能看到 RLS 测试**实际运行**（不是 SKIP）
- clippy / audit 失败能真正阻断合并

---

### W1 — 租户隔离真正生效（安全红线）

⚠️ **严格按 1→2→3→4 顺序执行。**

| # | 工作项 | 依据 | 量级 |
|---|---|---|---|
| W1-1 | 修 `db/mm.rs` fail-open：移除 `begin_optional_tenant_tx` 的 `None → 裸连接池` 路径（11 处），改为租户必填；`MMRepository::count`（`mm.rs:579`）加租户过滤 | 唯一 fail-open 的 repository，切角色也堵不住 | M |
| W1-2 | 处理**一条**刻意的管理员跨租户扫描：`db/ltm.rs::list_qdrant_tenant_backfill_entries`（走裸池、查询无租户过滤、`source_id LIKE 't:%'` 故意匹配全部租户，调用方 `backfill_qdrant_tenant_metadata` 按租户分组修复 Qdrant payload）—— 改走独立 owner 连接或改写为逐租户循环 | 切角色后会 RLS fail-close 返回 0 行 → 回填端点静默变 no-op。⚠️ **原计划写"两条"是错的**：`db/kg.rs::search_knowledge_by_entity_for_tenant` 经复核**已正确租户作用域**（租户必填 + 无条件 `begin_tenant_tx` + `entity_id LIKE prefix%`），无需改动 | M |
| W1-2b | **把 `aetheris_app` 角色创建从 `docker/initdb/` 搬进 sqlx migration** | 🔴 **实测发现**：initdb 脚本只在数据目录为空的首次初始化时执行。本地卷创建于 2026-07-16，脚本 07-17 才加入 → **角色在运行库里根本不存在**（`SELECT count(*) FROM pg_roles WHERE rolname='aetheris_app'` = 0）。生产库永远不是全新的，等于这个安全控制永不生效 | M |
| W1-3-pre | 🔴 **新增阻塞项：把迁移从应用启动流程解耦** | **实测发现**：应用启动时用 `sqlx::migrate::Migrator` 无条件跑迁移（`main.rs:67`），而受限角色 `aetheris_app` 在 schema public 上是 `USAGE=true, CREATE=false`（正确的最小权限），**没有 DDL 权限** → 切过去后启动即 `permission denied for schema public` 并 panic。切角色**不是换连接串**，它要求迁移改由 owner 在独立步骤执行 | M |
| W1-3a | **本地**切换连接角色并验证隔离生效 | ⚠️ 原判断"本地靠 pg_hba `trust` 无密码可连、零风险"**经实测为错**：`trust` 只覆盖容器自身 loopback；宿主机应用连发布端口时来源是 Docker 网关 IP，仍落到 `scram-sha-256`，需密码。本地已用 `ALTER ROLE` 设开发密码解除。**依赖 W1-3-pre** | S |
| W1-3b | **部署**切换：`backend/config.toml` 与 `backend/docker.toml`（⚠️ **两者均已被 git 跟踪**）+ 密码来源决策（`ALTER ROLE` / compose secret / secrets manager） | 同样依赖 W1-3-pre。容器网络连接必须有密码（实测 `fe_sendauth: no password supplied`） | M |
| W1-4 | 禁止 SQLite 静默降级：`db.url` 为空时在非 dev 环境 fail-fast，而不是悄悄退到无 RLS 的 SQLite | `config/mod.rs:129-138` | S |
| W1-5 | 校正口径：RLS 实际是 **8 张表**（不是文档的 13 张） | — | XS |

**W1 放行标准**
- `rls_isolation_pg` / `rls_kg_pg` / `rls_mm_pg` / `rls_stm_pg` / `tenant_scope_pg` **全部真实执行且通过**
- 手工验证：以 `aetheris_app` 连库，不设 GUC 时对 8 张 RLS 表的查询返回 **0 行**（fail-close）
- 回归：Qdrant 租户回填、KG 实体搜索仍可用（证明 W1-2 处理到位）

---

### W2 — 边界防护（安全红线，与 W1 可并行）

| # | 工作项 | 依据 | 量级 |
|---|---|---|---|
| W2-1 | serve 时改用 `into_make_service_with_connect_info::<SocketAddr>()`（`main.rs:165/180` 两处），让 `ConnectInfo` 真正注入 | 现在恒走客户端可控的 XFF 分支，限流形同虚设 | S |
| W2-2 | 增加可信代理白名单：只有来自白名单的请求才采信 `X-Forwarded-For` | 修完 W2-1 后仍需正确处理反代场景 | S |
| W2-3 | `POST /register` 补限流（当前挂在根路由，既无鉴权也无限流） | 账号刷注册 / 用户枚举 | S |

**W2 放行标准**
- 负向测试：轮换 `X-Forwarded-For` **不能**绕过限流
- 负向测试：登录接口连续失败会被限流（撞库保护真实生效）
- 无 XFF header 时不再让所有客户端共用 `"unknown"` 桶

---

### W3 — 治理变成"既生效又可用"（安全 + 功能红线）

> 🔴 **本 Wave 包含一个功能性 P0**：当前 `pre_store` 已在查 `write` 权限，
> 但角色**从未播种、唯一的 `assign_role` handler 在未挂载的 `routers/tenant.rs:202`**
> → `blocking_has_permission` 恒为 `false` → **鉴权打开时核心记忆接口全部 403**。
> 系统目前只在 `jwt.disabled`（治理被整体跳过）下"可用"。**必须先补 1，再动 3。**

⚠️ **严格按 1→2→3→4 顺序执行。**

| # | 工作项 | 依据 | 量级 |
|---|---|---|---|
| W3-1 | **先让 RBAC 能通过**：挂载角色分配入口 + 启动时播种默认角色（或租户创建时自动授予 Owner） | 不做这步，后面每一步都会把生产打挂 | M |
| W3-2 | **让配额能计数**：中间件补调 `post_store`（现在只调 pre-hook，`used` 永远为 0）；租户创建时 provision 配额；未知租户的默认策略明确（当前默认 allow） | 配额永不触发 | M |
| W3-3 | **再收紧**：确认 RBAC 强制生效并补负向测试；`GOVERNANCE_FAIL_CLOSED` 默认值改为 fail-closed | 当前默认 fail-open | M |
| W3-4 | 修 `jwt.disabled` 短路顺序：它目前在 fail-closed 判断**之前** return，导致 fail-closed 保护不了关闭鉴权的部署 | `governance.rs:110-112` | S |
| W3-5 | **最后扩面**：让 `classify()` 覆盖 MCP 工具调用路径（`.../mcp/tools/call` 目前匹配不到任何模式 → 中间件跳过 → MCP 写/删完全无配额无 RBAC） | MCP 是主要 Agent 写入通道 | M |
| W3-6 | 评估是否把 `/billing`、`/tenants`、`/snapshot`、`/memory-pool`、`/v1/agents` 也纳入治理层（当前有鉴权但无角色门禁） | 计费/租户管理无角色控制 | M |
| W3-7 | 审计可靠性：`dropped_count()` 暴露为指标并告警；明确"审计是尽力而为"还是"合规必达"，后者需要背压/重试；SQLite 下审计全丢需显式声明 | `audit_writer.rs:70-87` | M |

**W3 放行标准**
- **正向**：鉴权打开 + 治理打开时，正常用户可以正常读写记忆（证明 403 死结已解）
- **负向**：越权角色被拒 / 超配额被拒 / MCP 写入受治理（三条端到端测试）
- `jwt.disabled=true` 时不再静默跳过治理（或显式拒绝在生产环境启用该开关）

---

### W4 — 可靠性与可观测补齐

| # | 工作项 | 依据 | 量级 |
|---|---|---|---|
| W4-1 | **补 outbox 行为测试**：`ON CONFLICT DO NOTHING`、`claim_batch`、重试退避、`reclaim_stale`、幂等重放 —— 当前只测了 key 字符串格式 | 全项目最扎实的一块**却没有回归保护**，风险不对称 | M |
| **W4-1b** | **`claim_batch` 无租户公平性**（新增，由 W4-1 的测试暴露） | `claim_batch` 不按 `tenant_id` 作用域，outbox 消费是**进程全局**的：一个租户堆积大量待投递事件会拖慢**所有**其他租户的向量索引。这是当前设计而非 bug，但属多租户故事的缺口——也是那批测试必须靠互斥锁 + 租户后置过滤才能隔离的原因（它们无法自然隔离）。若要做，需引入按租户轮询或配额化的领取策略 | M |
| W4-2 | 接上 `vector_reconciliation`：加守护进程或运维端点（代码已完整实现，零调用者） | 低成本高收益 | S |
| W4-3 | 补真实的 liveness / readiness 探针；readiness 检查 PG / Qdrant 连通性 | CLAUDE.md 和 README 都写了，**实际不存在** | M |
| W4-4 | 处理 `self_healing` 假数据端点：`check_health` 返回硬编码 `healthy:true` + 1-4ms 假时延，**且对外服务于 `/api/v1/memory/v1/health`** —— 实现真探针或摘除端点 | 对外提供虚假健康状态，运维会误信 | M |
| W4-5 | compose healthcheck 从 `/`（裸根）改为 `/health` | — | XS |
| W4-6 | 补 Prometheus 告警规则（outbox 堆积 / 死信增长 / 鉴权失败 / 配额拒绝）+ Grafana dashboard | 指标质量不错，但**无告警无看板** | M |
| W4-7 | 确认 `set_outbox_pending()` 有后台 poller 保持新鲜，否则 gauge 不可信 | — | S |
| **W4-8** | **统一后端 query 参数命名约定 + 加契约测试**（新增，根因项） | 🔴 **实测**：7 个 `*Query` 结构体里 **3 个 rename 成 camelCase、4 个保持 snake_case**（`ExplainQuery`/`LimitQuery`/`ListMMQuery` 有 rename；`ListTracesQuery`/`ListSessionsQuery`/`ListEntitiesQuery`/`UserListQuery` 无）。Axum 的 `Query` 按字段名匹配，**不匹配就静默丢弃且不报错**。这一条不一致已直接造成 **4 个已确认 bug**：Rust SDK `list_sessions` 参数从未发送（且返回类型不匹配、运行时必失败）、Python SDK 发 `userId` 而后端要 `user_id`、前端 `getDecisionTraces` 发 `taskId`/`page`/`pageSize` 三个全丢。**注意影响分级**：前两个在真实调用路径上；前端那个位于**零调用点的死代码**中（页面实际用的是 `getDecisionTrace` 单数 → `adaptive/trace`），属于潜伏陷阱而非当前用户可见故障。只修症状会继续复发，必须统一约定并加契约测试锁死 | M |

**W4 放行标准**
- outbox 重放/幂等有测试覆盖并通过
- 对账扫描能在测试环境跑出 missing/orphan 报告
- `/health/readiness` 在 Qdrant 停掉时正确返回 not-ready
- 至少 4 条告警规则生效并演练过一次

---

### W5 — 对外一致性（文档 / SDK / 前端漂移）

| # | 工作项 | 依据 | 量级 |
|---|---|---|---|
| W5-1 | 校正 `CLAUDE.md`：删除不存在的 `/api/v1/health/liveness`、`/health/readiness`、`/api/v1/tenant/context`、`/api/v1/tenant/quota`、`/api/v1/distributed/epoch/status` | 照着写会踩空 | S |
| W5-2 | 校正 `IMPLEMENTATION_STATUS.md`：按 walkthrough 重写能力真值表 | 双向失真 | S |
| W5-3 | 校正 `docs/ROADMAP.md`：v0.8"分布式集群 Implemented"改为"进程内单机协调，HA 委托托管基建（ADR-0005）" | **技术尽调雷点** | S |
| W5-4 | 8 份 ADR 全是 `Proposed`，代码已按其中数份实现 —— 补 Design Review 收口转 `Accepted` | 治理债 | M |
| W5-5 | SDK 修 3 个具体 bug：Python/Rust 的 `api/mcp/initialize` → `/api/initialize`；LangChain `pyproject.toml` 的 `build-backend` 写错导致 `pip install` 失败；Rust SDK 孤儿模块 + README 文档死 API | 用户第一接触面 | M |
| W5-6 | 前端 `src/services/memory/auth.ts` 4 个认证路径修正，并关掉 mock 跑一次真后端联调 | 一接真后端登录就 404 | S |
| W5-7 | 手写 OpenAPI → utoipa 宏自动生成（当前只覆盖 MVP 子集，Scalar 不是权威清单） | 已在原 backlog | L |

**W5 放行标准**
- 文档中每一个端点都能在 `routers/mod.rs` 找到注册
- `pip install` SDK 成功；README 示例可编译运行
- 前端在**关闭 mock** 的情况下完成一次登录 + 主要页面走通

---

### W6 — 决策项收口

见 §5，需要先拍板。

---

## 4. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| **W3 顺序做反，生产 403 全面爆炸** | 中 | 高 | 严格 1→2→3→4；每步后跑正向冒烟；灰度发布 |
| **W1 切角色后管理员路径静默挂掉** | 中 | 中 | W1-2 必须先于 W1-3；切换后专项回归回填与 KG 搜索 |
| 修了但证明不了（重蹈 8-03 覆辙） | 中 | 高 | W0 前置；每个 Wave 的放行标准都是**可执行命令**，不是自我宣告 |
| `aetheris_app` 权限不足导致运行时报错 | 中 | 中 | 先在测试环境切换观察一轮，再上生产 |
| fail-closed 默认值上线后误拒合法请求 | 中 | 中 | 先在测试环境以 fail-closed 跑一个观察窗口，监控 `denied` 指标 |
| 范围膨胀（W5/W6 挤占 W0-W3） | 高 | 中 | W0-W3 未完成前，W5/W6 不占用主力人力 |

---

## 5. 决策项（做 or 删，需先拍板再排期）

这些**不是工作项**。默认按"实现"投入人力是错的，请先给结论：

| # | 决策 | 选项 A | 选项 B（通常更优） |
|---|---|---|---|
| D-1 | **gRPC / WebSocket** | 补 `.proto` + tonic server / 挂载 WS 路由 | **删掉类型定义与未接线函数**并同步文档 —— 当前中间态维护成本 > 价值 |
| D-2 | **`kernel/` + `layers/`** | 接上真实 repository（有长期价值） | 明确标注为实验区，避免新人误读 |
| D-3 | **A2A** | 保留 feature 并补 CI 编译验证 | 删除 —— 默认不编译，`a2a_integration.rs` 7 个测试从不运行 |
| D-4 | **MCP wasm 沙箱** | 按 ADR-0004 接入不可信扩展平面 | 暂缓，但**从文档中移除"沙箱"表述** |
| D-5 | **MCP capability** | 做按主体最小权限 | 至少停止对所有调用方硬编码 `[Read,Write,Delete]` |
| D-6 | **open-core 边界** | 真正做 feature gating（`feature = "enterprise"` 目前在 `src/` 出现 **0 次**，gates 不了任何代码） | 放弃 open-core 叙事，统一 MIT |
| D-7 | **P3 自适应** | 按 ADR-0008 走离线拟合 + eval 证明 | **正式采纳"启发式配置选择"叙事**；同时修正 `config_recommendation.rs` 头部"基于规则+历史数据"的虚假声明（它从不查 `training_samples` 且零调用者） |

**建议**：D-1 选 B、D-3 选 B、D-6 先选 B、D-7 选 B。理由一致 —— **减少"声明 > 实现"的表面积**，
这正是上一轮遗留问题的根源，也是技术尽调最容易翻车的地方。

---

## 6. 节点检查

| 节点 | 条件 |
|---|---|
| 方案评审 | §5 决策项全部有结论；W3 顺序约束被相关同学确认理解 |
| W0 完成 | 测试真实性证据（CI 日志显示 RLS 测试实际运行） |
| W1 完成 | 渗透测试通过 + 管理员路径回归通过 |
| W2 完成 | XFF 绕过负向测试通过 |
| W3 完成 | 正向冒烟（不再 403）+ 三条负向测试通过 |
| 生产准入 | W0-W3 全部完成；W4 的 readiness 与告警至少满足最小集 |
| 发布准备 | `deployment-context.md` + `launch-acceptance.md` 按 artifact 规范补齐 |

---

## 7. 明确的非目标

- 不在本轮引入新的记忆能力或新协议
- 不在本轮重构 `kernel/`（除非 D-2 选 A）
- 不在本轮启动 P3-full 自适应学习
- 不追求"一轮全清"——W5/W6 可以滚动进行

---

**关联文档**：`docs/TEAM_TECH_WALKTHROUGH.md`（问题清单与证据）、
`docs/adr/ADR-0004`（MCP 沙箱）、`ADR-0005`（HA 选型）、`ADR-0008`（自适应与 eval）
