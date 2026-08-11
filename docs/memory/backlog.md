# Backlog

> 跨任务事实源。任何遗留项、技术债、下一阶段候选以本文为准，不再只存在于
> release 备注、artifact 或对话中。
>
> - 建立日期：2026-08-10
> - 建立来源：`docs/artifacts/2026-08-10-truthfulness-and-security-remediation/closeout-summary.md`
> - 更新角色：`tech-lead`

## 任务追踪对照

2026-08-11 全量登记进会话任务清单，共 **30 项**（原 29 项 + 新发现 1 项）。
`#N` 为任务 ID。**会话任务清单是易失的，本文仍是唯一持久事实源**——
任务状态变化必须回写本文，不能只留在任务清单里。

| 分类 | 项数 | 任务 ID | 状态 |
|---|---|---|---|
| B 生产阻塞 | 5 | #1 #2 #4 #31 #39 | **#1 #2 #31 代码完成待真机验证**；#4 未开始；#39 为本次新发现 |
| A 声明 vs 实现 | 7 | #5 #6 #7 #8 #9 #10 #11 | **#5 完成**；#8 部分完成（1/3）；其余未开始 |
| C 治理与多租户 | 4 | #32 #13 #14 #15 | 未开始 |
| D 实测候选 | 9 | #16 #17 #18 #19 #20 #21 #22 #23 #24 | **#19 #20 #23 完成**；其余未开始 |
| E 技术债 | 7 | #25 #26 #27 #28 #29 #33 #34 | **#28 #29 完成**；#33 #34 为本次新发现 |

**已登记的硬依赖**：`#6 ← #5`（沙箱双平面需先有按主体授予集合，**#5 已完成，#6 解除阻塞**）、
`#17 ← #16`（先统一命名+契约测试，否则手工排查要做两遍）。
其余项均可独立开工。

**2026-08-11 批次进展**：11 项动过（8 项完成 / 3 项代码完成待真机验证），
新发现 3 项（#33 #34 #39）。测试基线 `769 → 786 passed / 0 failed`，
doc test 从 0 运行变为 2 运行。fmt 偏差保持 9 文件 / 65 处（**零新增**，均为 #34 的既有项）。
⚠️ 本机无 Docker，PG / Qdrant / Prometheus / promtool 均未起，
故 B 类三项的真机行为、告警规则的 PromQL 语义均**未验证**。

---

## A. 已拍板决策：7 项「声明 vs 实现」缺口 → 决定为**做**（不删）

2026-08-10 由 tech-lead 拍板：这 7 项一律**补齐实现**，而非删除声明。
下表按**实施成本升序**排列，即建议推进顺序。

| 顺序 | 项 | 现状 | 工作内容 | 量级 | 任务 |
|---|---|---|---|---|---|
| 1 | **D-5** MCP capability 最小权限 | ✅ **完成（未提交）**。`routers/mcp.rs` 改为按主体解析：从 RBAC 取调用方 Role，经 `capability::capabilities_for_role` 推导授予集合；无角色记录时**回落到 Reader**（最小权限），不继承写权限。授予集合从 `Role::has_permission` **派生**而非复制，故 MCP 平面与 REST 平面不会漂移（有防漂移测试）。⚠️ **今日可观测行为不变**：每人仍是自身单用户租户的 Owner（见 C-3），三项写权限齐全；变的是授予来源从常量变为主体，待 org 层租户模型落地即自动生效。Reader 现已无法经 MCP 调 `memory_write` / `memory_forget` | 改为按主体解析授予集合，未声明即拒 | S | #5 |
| 2 | **D-4** MCP wasm 沙箱接入 | ✅ **完成（未提交）**。`call_tool` 顶部新增 `classify_plane`，**必须先于** Plane A 的 `capability::authorize`——否则其 deny-by-default 会把一切非第一方工具当 `UnknownTool` 拒掉，Plane B 永远进不去。FirstParty→原生；Extension→`SandboxProxy`→`WasmSandbox::execute_wasm`（真 wasmtime）；Unknown→拒绝+审计。**顺带修掉一个假沙箱**：`SandboxProxy::execute_tool` 原本**忽略传入的 capability policy、直接调原生 `tool.execute()`**，且 `execute_wasm` 改动前**零调用点零测试**——真沙箱躺着而唯一入口绕过它。若只把这个 proxy 接进 live 路径，会得到「看起来接好了的假隔离」，比不接更危险。第一方工具**故意保持原生**（需特权 DB 访问，套 wasm 只能把 DB 重新开成宽 host fn，隔离收益≈0）。删除了误导性的 `SandboxedTool` trait。12 项测试。⚠️ **生产 registry 为空**——「通路存在」≠「Plane B 可用」；`execute_wasm` 要求同时授予全部 4 个 capability 才能跑任何模块，是**占位符且方向反了**（纯计算 wasm 本不需任何 host capability）。决策已沉淀至 `docs/memory/decisions.md` | 按 ADR-0004 双平面模型接线 | M | #6 |
| 3 | **D-6** open-core 真 feature gating | `feature = "enterprise"` 在 `src/` 出现 **0 次**（2026-08-11 复核仍为 0），gates 不了任何代码；billing 等所谓 Enterprise 模块默认编入 MIT 二进制 | 给 billing / RBAC / governance / 可视化加 `#[cfg(feature = "enterprise")]`；CI 增加「默认构建」与「enterprise 构建」双通道，防止 gate 漂移 | M | #7 |
| 4 | **D-3** A2A | **部分完成 1/3**：`Cargo.toml:84-85` 已 pin `rev=0b19af0e2805455c01f8f2b7fb52c5d5ec1bce95`，features 已改为真依赖 `a2a = ["dep:a2a", "dep:a2a-server"]`。但 CI 无 `--features a2a` 通道，`a2a_integration.rs` 的 7 个测试**仍从不运行** | 纳入 CI 构建验证、让 7 个测试真跑 | M | #8 |
| 5 | **D-1** gRPC + WebSocket 真实化 | `grpc_auth_interceptor`（`protocol/grpc.rs:21`）**零调用者**、无 tonic server、无 `.proto`、无 `build.rs`；`ws_upgrade_handler`（`protocol/websocket.rs:381`）**从未挂载**，内层 handler 是 TODO（连上即 Close 1008） | 按 ADR-0007：补 `.proto` + `build.rs` + tonic server + service impl；挂载 WS upgrade 路由并实现真实 handler；两者复用 `hoops::jwt::authenticate()` 统一鉴权核心 | L | #9 |
| 6 | **D-2** `kernel/` + `layers/` 接真实存储 | 整套 trait 抽象设计良好但**线上主链路完全绕开**；`layers/{stm,ltm,kg,mm}_layer.rs` **四个文件全部**是 `RwLock<HashMap>` 内存 stub，`ltm_layer` 的 search 是 `contains()` 子串匹配、score 恒 1.0 | 把 `layers/*` 后端从内存换为 `db::*Repository`；让 `routers/procedural.rs` 的 GraphRAG 不再跑在空 stub 上；此后 `hybrid_search.rs` 的正确 RRF 实现才有真实输入 | L | #10 |
| 7 | **D-7** P3 自适应（按 ADR-0008） | analyzer / predictor / scheduler / weight 全为写死系数，**零遥测学习**；`TrainablePredictor::fit_from_samples` 只有声明无实现；`config_recommendation.rs` 零调用者且头部「基于规则+历史数据」为**虚假声明** | ⚠️ **有前置条件**：ADR-0008 明确要求「先由 eval harness 证明自适应显著优于静态最优配置，之后才放行」。而 `eval_harness.rs:105-107` 仍硬编码 `passed:true`、`actual_coherence:1.0`、从不调用 scheduler。**故第一步必须是把 eval harness 做成真的**，再谈离线拟合 | XL | #11 |

**D-7 补充**：无论最终是否放行学习闭环，`config_recommendation.rs` 头部那句
「基于规则 + 历史数据 / training_samples」都必须立即修正——它从不查询
`training_samples`，属虚假声明。

---

## A0. 🔴 P0-6 / P0-7：跨租户泄漏（2026-08-11 新发现，优先级高于本文所有其他项）

**这两项不在原始 29 项清单里，是做 C-1（治理门禁）时撞出来的。**

### P0-6 path 参数类 —— ✅ 已修（未提交），任务 #56

**11 个 handler**（比初判多 1 个：`tenant.rs::get_user_role` 用元组 path）从 URL path 取
`tenant_id`，所在 3 个文件对 `RequestTenantContext` 的引用数**均为 0**——完全不与调用方
认证身份比对。任何已认证用户改一下 URL 就能访问其他租户的数据。

**修法**：新增 `RequestTenantContext::authorize_path_tenant(&self, path_tenant) -> Result<(), AppError>`
（`tenant/context.rs`），每个 handler 首行调用（authz 先行，早于日志与 body 校验）。
不符返回 **403 而非 404**——404 会让端点变成租户存在性探测器；错误消息**不含任何
tenant id**，避免既当探测器又当 id 回显。`get_tenant` 等原本的 404 保留在「访问自己租户
但它不存在」这一路径上。

**防漂移守卫**（本项最有价值的产出）：`every_path_tenant_handler_calls_authorize` 扫描
`src/routers` 每个文件的每个 `pub async fn`，凡签名同时含 `Path` 与 `tenant_id` 的 handler
必须在函数体出现 `authorize_path_tenant`。基于**签名而非硬编码清单**，所以第 12 个
handler 会被自动抓到。设 `scanned >= 4` 下限防扫描器静默失效。**实测会咬人**：删掉
`get_current_valid` 的 authorize 后测试 FAILED 并点名该函数。

**两处核实推翻了初判**：
1. **`routers/tenant.rs` 整个文件是死代码**（`routers/mod.rs:39` 的 `#[allow(dead_code)] mod tenant;`，
   `/tenants` 实际挂 `multi_tenant_router::*`），所以 11 个里真正在线可打的只有 **4 个**
   （billing 2 + multi_tenant 2）。7 个死 handler 仍防御性修了——将来挂载时不会重新引入漏洞。
   死代码去留见 C-5（#65）。
2. **dev 模式行为变化**：`jwt.disabled=true` 时 `ctx.tenant_id = "anonymous"`，所以访问
   `/billing/usage/<非 anonymous>` 现在会 403。**这是正确的安全语义**，已拍板**不加
   config 耦合的 dev 旁路**（会削弱安全属性且难测）。若 demo 硬编码了别的 tenant_id，
   该修的是 demo。

### P0-7 body 参数 / 无参类 —— 进行中，任务 #64

同类缺陷还有 6 处不走 path：`billing.rs` 的 `get_usage`（**跨租户读账单**）/ `init_tenant` /
`record_usage` 从 **body** 取 tenant；`multi_tenant_router.rs` 的 `register_tenant` /
`check_access`（访问策略探测器）；以及 **`list_tenants` 无参、枚举全部租户 id**。

### 为什么现有防护都挡不住（这是可复用的教训）

1. **C-1 刚加的角色门禁挡不住**——每人都是自身租户 Owner，全部通过角色检查，然后照样
   读 path/body 里指定的任意租户。门禁回答「你有没有这个权限」，**不是**「这个
   `tenant_id` 是不是你的」。也因此 P0-7 里否掉了「给 `list_tenants` 加 ManageTenant 门禁」
   ——那道门禁对所有人放行，等于没加。
2. **RLS 也挡不住**——RLS 按 `begin_tenant_tx` 设的 GUC 过滤，而这些 handler 传的是
   path/body 里的 `tenant_id`，RLS 会「正确地」把别人的数据返回给你。

**授权门禁与数据作用域机制可以都存在、都正常工作，却仍然不构成租户隔离**——缺的是
handler 边界上的身份-资源绑定。与已修的 P0-2（`db/mm.rs` 租户 fail-open）同类，只是上移了一层。

---

## B. 生产阻塞项（未完成，优先级高于 A）

| 项 | 现状 | 来源 | 任务 |
|---|---|---|---|
| **审计不可靠** | ✅ **完成（未提交）**。改为「有界队列 + 落盘兜底 + 启动回放」无损模型。**发现并修了原表述漏掉的第二个丢弃点**：`flush_batch` 的 INSERT 失败原本只 log+continue，**DB 宕机时每条已入队事件都静默丢**——对合规而言比队列满更致命（队列满只在高峰丢，DB 宕机是全丢）。设计要点：(1) 幂等**只用在回放路径**（审计表本有 `event_id PRIMARY KEY`，回放用 `ON CONFLICT DO NOTHING`；热路径仍用会报冲突的版本——那里的冲突是真 bug 不该吞）；(2) 回放前把活动 spill 文件原子改名成快照，新事件写新文件，回放期间不丢；(3) 截断尾行逐行跳过+计数不 fatal；(4) spill 上限 100MB，超限才降级为「丢弃+计数+ERROR」——这是唯一真丢数据的路径且必然告警；(5) **SQLite 明确不支持**（无审计表，spill 攒着没人回放只会泄漏磁盘）。4 个新指标（`audit_spilled/replayed/dropped/truncated_skipped_total`）。回放链路已验证真实接线：`main.rs:110` → `init_audit_writer` → `spawn(audit_writer_worker)` → 首条语句 `replay_spilled_on_startup()`。⚠️ **未验证**：端到端「DB 宕机→落盘→恢复→回放灌库」无 Docker/PG 未实测；**回放仅在启动时触发**，运行中 DB 恢复需等重启（保住了「不丢」，未优化恢复延迟） | W3-7 | #4 |
| **无告警、无 dashboard** | ✅ **代码完成（未提交）**。`monitoring/alerts/aetheris-alerts.yml` **7 条** LIVE 规则（后端不可达 + 四类对账漂移 + **对账扫描器停摆** + outbox p99 延迟）；14 面板 Grafana dashboard + provisioning；`prometheus.yml` 用**绝对路径** `/etc/prometheus/alerts/...` 挂 `rule_files`（相对路径按进程 CWD 解析，官方镜像 `WORKDIR=/prometheus` 是数据卷，且 glob 无匹配**不会导致启动失败** → 静默零加载）；docker-compose 挂载 `./monitoring/alerts`。指标未接仪表的规则隔离在 `alerts-staged/`（**故意不挂载**，见 #39）。⚠️ 无 promtool 与 Docker，**PromQL 语义未验证**，仅验了 YAML/JSON 合法性、job 名匹配与 rule_files→挂载点→宿主文件的路径链一致性 | W4 | #31 |
| **liveness / readiness 探针不存在** | ✅ **代码完成（未提交）**。新增 `routers/probes.rs`：`/livez`（**不查任何外部依赖**——liveness 失败会重启容器，而重启修不好挂掉的数据库）+ `/readyz`（真实 round-trip PG `SELECT 1` + Qdrant gRPC `health_check`，失败返回 503 并逐依赖列出原因）。挂在根路径、免鉴权（kubelet 无法出示 JWT）。3 项测试。**测试期间抓到一个真 bug**：`readyz` 原会 panic 而非返回 503（`config::get()` 的 `expect` 深埋在 `QdrantClient::new()` 里），已加防护。`self_healing` 的假时延已改为 `null` 并加 `is_dependency_probe: false`；CLAUDE.md 已同步 | W4 | #2 |
| **对账扫描器零调用者** | ✅ **代码完成（未提交）**。`init_reconciliation_scanner` 在 `main.rs:116` 被调用（PG-only，紧随 outbox worker）。含 `ReconciliationConfig`、**6 个 gauge**、13 项测试。默认 `dry_run` 只读 + 60s 间隔下限 + 60s 首扫延迟；扫描失败只记日志不退出循环。dead_code 警告 8 → **0**。**补了停摆检测**：四个漂移 gauge 在两次扫描间保持旧值，故扫描循环卡死时它们仍读「平静」——兜底自己死了却无人知晓。新增 `reconciliation_last_scan_timestamp_seconds`，**仅在扫描成功时 stamp**（失败时 stamp 会把「每轮都失败」误报成健康），启动时 seed 一次（未设时读 0 会让告警每次开机就触发）。⚠️ **未做真机验证**：四类漂移检测未经实跑确认 | W4 | #1 |
| **多数指标已注册但从未写入** | `set_outbox_pending` / `inc_outbox_dead_letter` / `set_tenant_quota_usage` / `inc_outbox_qdrant_upsert_*` / `set_ltm_entries_total` / `set_stm_sessions_active` / `record_request` / `record_search_duration` 在 exporter 之外**零调用者**，指标恒为 0。仅 `record_outbox_processing_duration` 与 `set_reconciliation_*` 有真实写入。⚠️ closeout 与本文原先都称这些指标「质量不错、均已导出」——**第三处同类失真**：字面成立（出现在 `/metrics`）但无信息量 | 2026-08-11 新发现 | #39 |

---

## C. 治理与多租户缺口

| 项 | 现状 | 任务 |
|---|---|---|
| 治理层未覆盖多个路由 | ✅ **完成（未提交）**。**原表述不准**：这 5 组不是均匀裸奔。`/billing`、`/snapshot`、`/memory-pool` 的 `governance_middleware` **早就挂在链上**，缺口在 `classify()` 对这些路径一律返回 `None` → 直接放行——**中间件在跑，只是什么都不分类**（又一处「存在≠起作用」）。只有 `/tenants`、`/v1/agents` 是真没挂中间件。另发现根因：`GovernanceHookImpl` 只实现 `pre_store→Write` / `pre_search→Read`，`pre_update`/`pre_delete` 是 trait 默认 `Allow`，所以 `Operation` 那条路**永远表达不了** `ManageBilling`/`ManageTenant`。新增强类型通道 `classify_permission(method, path) -> Option<Permission>`：billing→`ManageBilling`（仅 Owner）、tenants DELETE→`DeleteTenant`、tenants 其他→`ManageTenant`、snapshot→`ManageMemory`、memory-pool 与 agents→`ManageAgents`。8 项测试含防漂移守卫。不依赖 enterprise hooks 是否初始化，故 SQLite dev 下同样生效。⚠️ **今日行为基本不变**（每人是自身租户 Owner），真正生效待 C-3 | #32 |
| `claim_batch` 无租户公平性 | outbox 消费**进程全局**，一个租户堆积会拖慢**所有**租户的向量索引。当前设计如此，非 bug | #13 |
| RBAC 尚不能区分权限 | 每人都是自己单用户租户的 Owner。真正的角色区分需要 **org 层租户模型**（`tenant_id` 与 `user_id` 解耦），属架构级变更，需先出 ADR | #14 |
| SQLite 模式无任何 DB 层隔离 | ✅ **完成（未提交）**。`db.url` 为空时的静默降级已改为**默认拒绝启动**：新增 `db.allow_sqlite_fallback`（默认 `false`）+ `APP_DB_ALLOW_SQLITE_FALLBACK`，未显式 opt-in 则 `exit(1)` 并给出三条修复路径；opt-in 时打印醒目横幅（同 `jwt.disabled`）。**顺带发现并修了一个更严重的问题**：`config::init()` 在 `main.rs:54` 运行、`otel::init_tracing` 在 `:66`，所以原先那句 `tracing::warn!` 发生在 subscriber 建立**之前、被静默丢弃**——降级此前连警告都没有。已改用 `eprintln!`（与 main.rs 自身的 startup 报错一致）。2 项测试 + 对照组验证（默认改 true 后守卫确实 FAILED）。⚠️ SQLite 本身仍无 DB 层隔离，这一项只关闭了「静默」，未改变 SQLite 的隔离能力 | #15 |

---

## D. 实测撞出的候选（非代码审查所得）

1. **DB check 约束以 500 而非 400 暴露给调用方**（#18）：`session_type` 仅接受 `conversation`/`task`/`query`，`source_type` 仅接受 `document`/`api`/`database`/`web`/`user_input`；传非法枚举值得到「内部错误」，集成方无从排查
2. ✅ **完成**（#19）**`config.toml` 与 `local.toml` 的 embedding 模型不一致**（768 vs 1024）。**原表述有偏差**：`local.toml` 是 gitignored 的本地覆盖，不是提交的配置档；真正的冲突源是两者共用 `collection_name = "long_term_memory"` 导致向量空间碰撞。已在 `config.toml` 注明「换 embedding 模型必须同时换 collection_name」，并让 `vector_guard` 报错给出**非破坏性出路**（改名共存）而非只教人丢数据
3. ✅ **完成**（#20）**`vector_guard` 错误提示不完整**：已补上签名文件绝对路径（`sig_path.display()`），并明确恢复需**两步**（丢 collection + 删签名条目），同时优先推荐非破坏性的改 collection 名方案
4. ✅ **完成**（#21）**Neo4j「可选」在运行上不成立**。已移出启动关键路径：新增 `spawn_neo4j_init`（`OnceLock` 幂等 + `tokio::spawn` + 10s 总超时），HTTP 监听不再等它。**核实推翻了原表述的两点**：(a) `init_neo4j_indexes().await.expect(...)` **永远不会 panic**——该函数无论如何都返回 `Ok(())`，那个 `.expect` 是死保险；真正的问题是它串行发 3 条 query，每条最多被 neo4rs 的 60s 退避阻塞（`pool.rs:29-34` 的 `max_elapsed_time=60s`），最坏 **~180s**。(b) `Graph::new` 只建**惰性**连接池、根本不发起 I/O，所以原先紧随其后的 "Successfully connected" 日志是在**没有任何连接**时打印的——又一条虚假日志，已加真实连通性探针（`RETURN 1`）后才打。占位符密码按「拒连 + 告警 + 继续」处理（不 `exit(1)`——Neo4j 是可选依赖，与 JWT secret 作为认证核心必须 fail-fast 的语义不同）。新增 `Neo4jStatus{Disabled|Connecting|Connected|Failed}` 使真实状态可观测。**故意未接入 `/readyz`** 并把该决策写进 doc 注释防后人误接（可选依赖挂掉不该把健康实例摘出 LB）。4 项纯逻辑测试。⚠️ 「错密码不再阻塞几十秒」是**代码结构层面**结论，本机无 Neo4j 未实测时序
5. ✅ **完成（未提交）**（#22）**Ollama 不可达时 LTM 写入完全不可用**。已按拍板方案把 LLM 摘要降级为可选。**核实推翻了原表述的两点**：(a) **没有独立 summary 列**——摘要直接存进 `content`（`NOT NULL`），`content_hash = sha256(summary)`，关键词搜索走 `content ILIKE`，embedding 拿 `content` 向量化。所以「摘要降级」不是「某字段留空」，而是**存入内容、哈希、向量全都不同**；因此用新列 `summary_status` 而非 `content IS NULL` 判断（后者不成立，降级时 content 非空）。(b) LTM 写入在热路径上调 Ollama **两次**（摘要 + embedding），且 **embedding 拿摘要去向量化**——所以摘要降级单独做**并不能**让「Ollama 不可达时写入成功」在单 Ollama 部署下成立，缺口见 D-e2（#61）。错误分类边界：`Unavailable`/`Upstream` → 降级但留 WARN + `ltm_summary_degraded_total{reason}` 指标；**`Malformed`（可达但返回垃圾）→ 仍 500**；embedding 失败 → 仍 500。唯一从 500 变成功的只有 LLM 不可达/上游错误且都可观测。⚠️ 运行时降级行为无 Ollama 未实测；补摘要后台任务**明确未做**（另开工作项），但已提供 `list_entries_pending_summary` 查询入口 + 部分索引。**补摘要时必须重算向量**（摘要变则 content 变则 hash 与向量都要变），已在代码注释留痕
6. ✅ **完成**（#23）**两个 doc 示例编译不过**：`tenant/context.rs`、`runtime/langchain_adapter.rs` 已修好并去掉 ```` ```ignore ````，`cargo test --doc` 从 **0 运行变为 2 运行**
7. **「参数被接受、被忽略、无报错」需全面排查**（#17）：`sdks/rust` 的 `list_sessions` 曾接收 `user_id`/`limit` 却完全忽略（已修），同类模式可能还有
8. ✅ **完成**（#24）**8 份 ADR 全为 `Proposed`**。已逐份核实实现真相并收口。**关键改进**：把原先挤在一个字段里的两件事拆成两个正交字段——`状态`（只描述决策是否采纳：Accepted/Proposed/Superseded/Rejected）与 `实现状态`（只描述代码：已完整落地/部分落地/未落地）。挤在一起会让扫状态表的人只看到 `Accepted` 就以为做完了，那正是本轮整改要修的病。结果：0001/0002/0004 = Accepted+部分落地；**0003/0007 = Accepted+未落地**（0003 门禁本身 blocked，未通过的门禁功能上等于无门禁；0007 鉴权核心已统一但 HTTP 之外无一协议被真正服务）；**0005 = Accepted+已完整落地**（唯一一份）；0006/0008 保持 Proposed。无 Superseded/Rejected——已落地的都按 ADR 方向落地，未落地的是尚未做而非走了替代方案。8 份均补 `Owner` + `收口责任人`；0006/0008 如实写「待 DRB 收口（仍 open）」而不假称已收口
9. **统一 query 参数命名 + 契约测试**（#16，**根因项，D 类中优先级最高**）：7 个 `*Query` 结构体中 3 个 rename 为 camelCase、4 个保持 snake_case，Axum 匹配不上即静默丢弃。已直接造成 4 个 bug，只修症状会复发

---

## E. 技术债（较低优先）

- ✅ **完成**（#26）**双 JWT 已收口**：`web/jwt.rs` 整文件删除（含 `web/mod.rs` 的 mod 声明与 re-export）。核实为**零代码调用者**——所有 `JwtClaims`/`auth_middleware`/`decode_token` 生产用点走的都是 `hoops::jwt::*`；`crate::web::` 唯一被消费的是 `cors_layer`。`hoops/jwt.rs` 是其**严格超集**（多出 query-token 拒绝、租户上下文注入、`jwt.disabled` dev 模式、鉴权失败指标、显式过期检查）；唯一结构差异是 `JwtClaims` 多一个 `iat` 字段，全仓无读取方。全仓无 WebSocket/img/下载链接依赖 query-string token，删除不会让任何功能静默失效。这同时闭合了 ADR-0007 的「违反项」（第二条平行鉴权路径不再存在）
- ✅ **完成**（#28）`sdks/rust` 的 `urlencoding` 依赖已移除，改用 `reqwest` 的 `.query()`；`cargo tree` 确认 0 命中，7 项测试通过。**已知编码差异**：空格从 `%20` 变为 `+`（form-urlencoded），二者对标准解析器等价且已加测试固定
- ✅ **完成**（#29）`search_knowledge_by_entity_for_tenant` 的无用 `LEFT JOIN knowledge_entries ke` 已删除。`DISTINCT` **保留**——真正制造重复行的是 `LEFT JOIN relations r`（按 source 或 target 匹配，一个有 N 条关系的实体产出最多 N 个重复 `e` 行）。⚠️ 该函数用运行时 `query_as` 而非 `query!` 宏，故 `cargo check` 通过**不校验 SQL**；结果集等价性为**静态推理**，本机无 PG 未实跑
- （#25）手写 OpenAPI → utoipa 宏自动生成（当前只覆盖 MVP 子集，Scalar 不是权威清单）
- （#26）~~双 JWT：`web/jwt.rs`~~ → 见上，已完成
- （#27）`ci.yml:158` 前端 `npm run lint` 仍 `continue-on-error: true`
- （#28）~~`sdks/rust` 新增 `urlencoding` 依赖~~ → 见上，已完成
- （#29）~~`search_knowledge_by_entity_for_tenant` 无用 `LEFT JOIN`~~ → 见上，已完成
- （#33，**2026-08-11 新发现**）CI clippy 无 `-D warnings`：`ci.yml:102` 是 `cargo clippy --all`，只去掉了 `continue-on-error`，警告一律放行。closeout-summary 称「clippy 改为阻塞」**表述不准**。前提是先清理存量警告（`cargo check` 516 条 / `cargo clippy --all` 628 条）
- （#34，**2026-08-11 新发现，已完成**）✅ `cargo fmt --all -- --check` 门禁。**我上轮的表述是错的**：我说「CI 显示通过」，但核实后发现**分支从未推送、远端无此分支、CI 从未运行过**——那是我从证据缺失里推出的结论，不是核实的（与 §4.5 同类错误）。真实情况是首次 push 时 CI 必定失败。已修：`cargo fmt --all` 把 65 处偏差清零；并修根因——新增 `rust-toolchain.toml` pin 到 `1.96.1`，`ci.yml` 从 `dtolnay/rust-toolchain@stable` 改为 `@master` + 显式版本。此前无 pin，CI 的 `@stable` 在运行时解析，与贡献者本地 rustfmt 可以不同版本，这正是偏差能被提交进来的机制

---

## F. 未合并

`fix/truthfulness-and-security-remediation` 分支（10 commits）状态仍为
`ready-for-review`，**尚未合并到 `dev`**。上次拍板的「review 后合并」未执行。
合并前置见 closeout-summary §6 与「运维前提」：存量库必须先
`ALTER ROLE aetheris_app WITH LOGIN PASSWORD '<managed-secret>'`。

---

**最后更新**：2026-08-11（批量批次：8 项完成 + 3 项代码完成待真机验证 + 新发现 #33 #34 #39）
**更新角色**：tech-lead
