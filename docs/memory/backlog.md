# Backlog

> 跨任务事实源。任何遗留项、技术债、下一阶段候选以本文为准，不再只存在于
> release 备注、artifact 或对话中。
>
> - 建立日期：2026-08-10
> - 建立来源：`docs/artifacts/2026-08-10-truthfulness-and-security-remediation/closeout-summary.md`
> - 更新角色：`tech-lead`

## 任务追踪对照

原始清单 29 项，2026-08-11 期间因做别的事撞出 **17 项新发现**（其中 2 项 P0），
共 48 项。`#N` 为会话任务 ID。**会话任务清单是易失的，本文是唯一持久事实源**
——任务状态变化必须回写本文。

**第四批新增的 2 项发现**：`detect_skill` 写意图静默降级为读（§D 第 10 条）、
CI 两个 job 的 cargo cache path 一直未命中（见「CI 配置修正」）。


### 已完成（38 项，2026-08-11 第四批收口）

| 分类 | 已完成 |
|---|---|
| **P0 安全** | P0-6 path 跨租户泄漏（#56）、P0-7 body/无参跨租户泄漏（#64）、C-4 SQLite 静默降级（#15） |
| **B 生产阻塞** | B-1 审计落盘（#4）、B-2 告警+dashboard（#31）、B-3 探针（#2）、B-4 对账接线（#1）、B-5 指标接线（#39）、#44 配额指标、**B-5b 余下 4 个零调用指标（#46）** |
| **A 声明 vs 实现** | A-1 MCP capability 按主体（#5）、A-2 沙箱双平面（#6）、A-4 A2A 入 CI（#8）、A-4b a2a 补挂 auth（#79）、**A-4c a2a 写操作接 in-handler governance（#82）** |
| **C 治理与多租户** | C-1 治理覆盖 5 组路由（#32）、C-2 outbox 租户公平性（#13）、C-5 tenant.rs 去留（#65） |
| **D 实测候选** | D-a 枚举 400（#18）、D-j SDK 非法示例（#77）、D-k 另 6 个 CHECK（#78）、D-b 配置维度（#19）、D-c vector_guard 提示（#20）、D-d Neo4j 阻塞启动（#21）、D-e 摘要降级（#22）、D-e2 embedding 硬依赖（#61）、D-f doc 示例（#23）、D-h ADR 收口（#24）、**D-i query 命名契约+防漂移（#16）**、**D-g 参数静默丢弃排查（#17）** |
| **E 技术债** | E-2 删双 JWT（#26）、E-3 前端 lint 阻塞（#27）、E-4 SDK urlencoding（#28）、E-5 无用 JOIN（#29）、E-7 fmt 门禁+toolchain pin（#34）、**E-7b SDK 入 CI（#83）**、E-8 rel=noopener（#63）、E-9 loading 态（#69）、E-10 Retry-After（#70）、E-11 前端 jest 基建（#80）、**E-12 a2a stream 重复实现+丢结果 bug（#84）** |


### 待处理（10 项）

| 项 | 量级 | 阻塞/前提 |
|---|---|---|
| **A-3** open-core feature gating（#7） | M | |
| **A-5** gRPC + WebSocket 真实化（#9） | L | 需 `.proto` + `build.rs` + tonic server |
| **A-6** kernel/layers 接真实存储（#10） | L | 动核心数据路径，高风险 |
| **A-7** eval harness 做成真的（#11） | XL | ADR-0008 的放行前置本身是假的 |
| **C-3** org 层租户模型（#14） | XL | **等产品方向决策**（ADR-0009 已出，三方案未拍板） |
| **D-e3** 补摘要 + 向量重算（#71） | M/L | 必须同时重算向量 |
| **E-1** utoipa 迁移（#25） | L | |
| **E-6** clippy `-D warnings`（#33） | L | **已拍板本批不动**：534 条里约 380 条是 dead_code 类（142 struct never constructed / 95 fn never used），指向 A-5/A-6 尚未接线的代码。清它等于要先决定那些模块去留，故等 A-5/A-6 落地后存量自然下降再上门禁 |
| **E-11b** 前端测试接入 CI（#81） | S | **前端项，已按指示推到最后**。⚠️ 前一轮「Node 20 实测 6 passed」**未能复现**：`node:20` 容器内 `npm test` 报 `Jest: Failed to parse the TypeScript config file jest.config.ts / Parse config file failed: [config/config.ts]`。ci.yml 里那个 step 已撤回并留注释说明——接了要么红、要么得挂 `continue-on-error`，而不能失败的门禁不是门禁 |
| **D-g2** `detect_skill` 关键词猜测导致写意图静默降级为读（新发现） | M | 见 §D 第 10 条 |

### 前端范围说明（2026-08-11 追加）

用户指示：**放弃前端优化，只做后端任务，前端任务放到最后**。故本轮前端相关项（E-11b）
不再推进，ci.yml 中前端 `npm test` step 已撤回。前端既有成果（E-3 lint 阻塞、E-8、E-9、
E-11 jest 基建修复）保持不动，不回滚。


### C-3 是四项的共同阻塞

A-1、C-1、P0-6、P0-7 都已完成，但**今天的可观测效果都受限**：
`tenant/context.rs:130` 把 `user_id` 直接当 `tenant_id`，所以每人都是自身
单用户租户的 Owner，角色检查恒 true。跨租户已被挡住，但同租户内的角色区分
无从体现。见 `docs/adr/ADR-0009-org-level-tenant-model.md`（Proposed，
三方案未拍板——A 与 C 的取舍取决于产品是否面向企业多用户）。

### 门禁现状（这一批的主要成果之一）

| 门禁 | 状态 |
|---|---|
| `cargo fmt --all -- --check`（backend） | ✅ 0 偏差 + toolchain pinned `1.96.1` |
| `cargo fmt --all -- --check`（sdks/rust） | ✅ 已清零并**首次纳入 CI**（此前整个 crate 从未被任何 job 编译，见 E-7b） |
| 前端 `npm run lint` | ✅ exit 0、0 warning、已阻塞 |
| `cargo test --tests` | ✅ **1013 passed / 0 failed**（此前 975） |
| `cargo test --features a2a --lib` | ✅ 453 passed / 3 ignored |
| `backend-a2a` job | ✅ **7 passed / 2 ignored**（2 个 ignored 均标了真实原因：需 embedding 后端） |
| `cargo test --doc` | ✅ 2 passed |
| `sdks/rust` clippy `--all-targets` | ✅ 0 warning（含 examples type-check） |
| `cargo clippy --all`（backend） | ⚠️ 仍无 `-D warnings`（#33，**已拍板本批不动**，根因是 A-5/A-6 未接线代码的 dead_code） |
| 前端测试 | ⚠️ **未接入 CI**（#81，前端项已推后；且 Node 20 下的通过结论未能复现） |

### CI 配置修正（2026-08-11 第四批）

`backend` 与 `backend-a2a` 两个 job 的 `actions/cache` 都把 `path` 写成裸 `target`，
但 `actions/cache` 的路径**相对 workspace root 解析**，而 `working-directory: backend`
只作用于 `run:` 步骤——所以它们缓存的是不存在的根 `target/`，**编译缓存一直没命中**。
已改为 `backend/target`。这类「配置存在但不起作用」与本轮整改的其余发现同类。


⚠️ **未验证的边界**（2026-08-12 首次 CI 运行后已大幅收窄，见下）：本机无 Docker /
PG / Qdrant / Neo4j / Ollama / promtool。

**CI（PR #79）已实跑验证的**——真实 PostgreSQL 16 + Qdrant 1.9.4，
`1032 passed / 0 failed / 0 ignored`，全日志 `SKIP` 出现 **0 次**（这些测试都带
env-guard 早返回路径，会打 `SKIP` 后仍报 ok，所以「0 次 SKIP」才是它们真跑过的
证据，光看 ok 不够）：

- **RLS 强制生效**：`ltm/kg/stm/mm_rls_blocks_cross_tenant_*` 四个穿透测试。它们用
  superuser 连接**只为 provision 一个受限角色** `aetheris_rls_probe`
  （NOSUPERUSER NOBYPASSRLS），再把 pool 指向它、走真实 Repository——所以断言真的
  在策略生效的角色下执行，不是在绕过 RLS 的 owner 下假过。
- **`claim_batch` 的租户公平性 CTE**：`claim_batch_concurrent_disjoint_sets`。此前
  本文记为「运行时 `query_as` 非 `query!` 宏，`cargo check` 不校验 SQL，仅静态推理」
  ——现已在真库上跑过并发不重复认领。
- 租户 GUC 的事务局部性与不泄漏；outbox 的插入幂等 / mark_applied / dead-letter /
  reclaim_stale 全链。

**仍未实跑**（CI 无对应服务或无对应测试）：

- 审计的落盘→回放**整链**。现有测试是单元级（builder / serde / 共享列 SQL /
  `enqueue_or_spill_closed_queue_spills_to_disk`），没有「DB 宕机→落盘→恢复→回放
  灌库」的端到端验证。
- 对账**四类漂移的真实检出**。现有测试是 enum roundtrip 与 `drift_types` 清单级。
- 告警的 PromQL 语义——CI **无 promtool step**（`grep -c promtool ci.yml` = 0）。
- Neo4j 与 Ollama 相关路径——CI **无这两个 service**，故 D-d 的时序、D-e 的摘要
  降级、D-e2 的 embedding 硬依赖均未在运行时验证。
- 启动 fail-fast 的 `exit(1)`；a2a governance 的配额门（依赖 enterprise hooks，
  默认构建下未初始化即 no-op）。

反过来，CI 证实 RLS 在受限角色下确实会过滤，这让「两个库存 gauge 在加固角色 +
无 GUC 时静默读 0」从理论推测变成**有依据的预期**，不再是可有可无的注脚。

---

## A. 已拍板决策：7 项「声明 vs 实现」缺口 → 决定为**做**（不删）

2026-08-10 由 tech-lead 拍板：这 7 项一律**补齐实现**，而非删除声明。
下表按**实施成本升序**排列，即建议推进顺序。

| 顺序 | 项 | 现状 | 工作内容 | 量级 | 任务 |
|---|---|---|---|---|---|
| 1 | **D-5** MCP capability 最小权限 | ✅ **完成（已提交）**。`routers/mcp.rs` 改为按主体解析：从 RBAC 取调用方 Role，经 `capability::capabilities_for_role` 推导授予集合；无角色记录时**回落到 Reader**（最小权限），不继承写权限。授予集合从 `Role::has_permission` **派生**而非复制，故 MCP 平面与 REST 平面不会漂移（有防漂移测试）。⚠️ **今日可观测行为不变**：每人仍是自身单用户租户的 Owner（见 C-3），三项写权限齐全；变的是授予来源从常量变为主体，待 org 层租户模型落地即自动生效。Reader 现已无法经 MCP 调 `memory_write` / `memory_forget` | 改为按主体解析授予集合，未声明即拒 | S | #5 |
| 2 | **D-4** MCP wasm 沙箱接入 | ✅ **完成（已提交）**。`call_tool` 顶部新增 `classify_plane`，**必须先于** Plane A 的 `capability::authorize`——否则其 deny-by-default 会把一切非第一方工具当 `UnknownTool` 拒掉，Plane B 永远进不去。FirstParty→原生；Extension→`SandboxProxy`→`WasmSandbox::execute_wasm`（真 wasmtime）；Unknown→拒绝+审计。**顺带修掉一个假沙箱**：`SandboxProxy::execute_tool` 原本**忽略传入的 capability policy、直接调原生 `tool.execute()`**，且 `execute_wasm` 改动前**零调用点零测试**——真沙箱躺着而唯一入口绕过它。若只把这个 proxy 接进 live 路径，会得到「看起来接好了的假隔离」，比不接更危险。第一方工具**故意保持原生**（需特权 DB 访问，套 wasm 只能把 DB 重新开成宽 host fn，隔离收益≈0）。删除了误导性的 `SandboxedTool` trait。12 项测试。⚠️ **生产 registry 为空**——「通路存在」≠「Plane B 可用」；`execute_wasm` 要求同时授予全部 4 个 capability 才能跑任何模块，是**占位符且方向反了**（纯计算 wasm 本不需任何 host capability）。决策已沉淀至 `docs/memory/decisions.md` | 按 ADR-0004 双平面模型接线 | M | #6 |
| 3 | **D-6** open-core 真 feature gating | `feature = "enterprise"` 在 `src/` 出现 **0 次**（2026-08-11 复核仍为 0），gates 不了任何代码；billing 等所谓 Enterprise 模块默认编入 MIT 二进制 | 给 billing / RBAC / governance / 可视化加 `#[cfg(feature = "enterprise")]`；CI 增加「默认构建」与「enterprise 构建」双通道，防止 gate 漂移 | M | #7 |
| 4 | **D-3** A2A | ✅ **完成**。依赖已 pin rev、features 接真依赖；新增独立 `backend-a2a` job（不折进 backend，避免 GitHub 拉取抖动牵连安全关键套件；刻意无 continue-on-error）。**首次运行立刻暴露真缺陷**：a2a router 未挂 `auth_middleware`，4/7 因缺 `Extension<RequestTenantContext>` 而 500。已在 A-4b 修复（agent card 按 A2A spec 保持公开——它宣告 security schemes，挡在 auth 后是 discovery 的鸡生蛋问题；其余全走 auth）。现 **7 passed / 1 诚实 ignored**（那 1 个需真实 embedding 后端，标了 `#[ignore]` 原因而非伪造通过）。余 A-4c：写操作接 in-handler governance | 纳入 CI 并让测试真跑 | M | #8 |
| 5 | **D-1** gRPC + WebSocket 真实化 | `grpc_auth_interceptor`（`protocol/grpc.rs:21`）**零调用者**、无 tonic server、无 `.proto`、无 `build.rs`；`ws_upgrade_handler`（`protocol/websocket.rs:381`）**从未挂载**，内层 handler 是 TODO（连上即 Close 1008） | 按 ADR-0007：补 `.proto` + `build.rs` + tonic server + service impl；挂载 WS upgrade 路由并实现真实 handler；两者复用 `hoops::jwt::authenticate()` 统一鉴权核心 | L | #9 |
| 6 | **D-2** `kernel/` + `layers/` 接真实存储 | 整套 trait 抽象设计良好但**线上主链路完全绕开**；`layers/{stm,ltm,kg,mm}_layer.rs` **四个文件全部**是 `RwLock<HashMap>` 内存 stub，`ltm_layer` 的 search 是 `contains()` 子串匹配、score 恒 1.0 | 把 `layers/*` 后端从内存换为 `db::*Repository`；让 `routers/procedural.rs` 的 GraphRAG 不再跑在空 stub 上；此后 `hybrid_search.rs` 的正确 RRF 实现才有真实输入 | L | #10 |
| 7 | **D-7** P3 自适应（按 ADR-0008） | analyzer / predictor / scheduler / weight 全为写死系数，**零遥测学习**；`TrainablePredictor::fit_from_samples` 只有声明无实现；`config_recommendation.rs` 零调用者且头部「基于规则+历史数据」为**虚假声明** | ⚠️ **有前置条件**：ADR-0008 明确要求「先由 eval harness 证明自适应显著优于静态最优配置，之后才放行」。而 `eval_harness.rs:105-107` 仍硬编码 `passed:true`、`actual_coherence:1.0`、从不调用 scheduler。**故第一步必须是把 eval harness 做成真的**，再谈离线拟合 | XL | #11 |

**D-7 补充**：无论最终是否放行学习闭环，`config_recommendation.rs` 头部那句
「基于规则 + 历史数据 / training_samples」都必须立即修正——它从不查询
`training_samples`，属虚假声明。

---

## A0. 🔴 P0-6 / P0-7：跨租户泄漏（2026-08-11 新发现，优先级高于本文所有其他项）

**这两项不在原始 29 项清单里，是做 C-1（治理门禁）时撞出来的。**

### P0-6 path 参数类 —— ✅ 已修（已提交），任务 #56

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

### P0-7 body 参数 / 无参类 —— ✅ 已修，任务 #64

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
| **审计不可靠** | ✅ **完成（已提交）**。改为「有界队列 + 落盘兜底 + 启动回放」无损模型。**发现并修了原表述漏掉的第二个丢弃点**：`flush_batch` 的 INSERT 失败原本只 log+continue，**DB 宕机时每条已入队事件都静默丢**——对合规而言比队列满更致命（队列满只在高峰丢，DB 宕机是全丢）。设计要点：(1) 幂等**只用在回放路径**（审计表本有 `event_id PRIMARY KEY`，回放用 `ON CONFLICT DO NOTHING`；热路径仍用会报冲突的版本——那里的冲突是真 bug 不该吞）；(2) 回放前把活动 spill 文件原子改名成快照，新事件写新文件，回放期间不丢；(3) 截断尾行逐行跳过+计数不 fatal；(4) spill 上限 100MB，超限才降级为「丢弃+计数+ERROR」——这是唯一真丢数据的路径且必然告警；(5) **SQLite 明确不支持**（无审计表，spill 攒着没人回放只会泄漏磁盘）。4 个新指标（`audit_spilled/replayed/dropped/truncated_skipped_total`）。回放链路已验证真实接线：`main.rs:110` → `init_audit_writer` → `spawn(audit_writer_worker)` → 首条语句 `replay_spilled_on_startup()`。⚠️ **未验证**：端到端「DB 宕机→落盘→恢复→回放灌库」无 Docker/PG 未实测；**回放仅在启动时触发**，运行中 DB 恢复需等重启（保住了「不丢」，未优化恢复延迟） | W3-7 | #4 |
| **无告警、无 dashboard** | ✅ **完成（已提交）**。`monitoring/alerts/aetheris-alerts.yml` **7 条** LIVE 规则（后端不可达 + 四类对账漂移 + **对账扫描器停摆** + outbox p99 延迟）；14 面板 Grafana dashboard + provisioning；`prometheus.yml` 用**绝对路径** `/etc/prometheus/alerts/...` 挂 `rule_files`（相对路径按进程 CWD 解析，官方镜像 `WORKDIR=/prometheus` 是数据卷，且 glob 无匹配**不会导致启动失败** → 静默零加载）；docker-compose 挂载 `./monitoring/alerts`。指标未接仪表的规则隔离在 `alerts-staged/`（**故意不挂载**，见 #39）。⚠️ 无 promtool 与 Docker，**PromQL 语义未验证**，仅验了 YAML/JSON 合法性、job 名匹配与 rule_files→挂载点→宿主文件的路径链一致性 | W4 | #31 |
| **liveness / readiness 探针不存在** | ✅ **完成（已提交）**。新增 `routers/probes.rs`：`/livez`（**不查任何外部依赖**——liveness 失败会重启容器，而重启修不好挂掉的数据库）+ `/readyz`（真实 round-trip PG `SELECT 1` + Qdrant gRPC `health_check`，失败返回 503 并逐依赖列出原因）。挂在根路径、免鉴权（kubelet 无法出示 JWT）。3 项测试。**测试期间抓到一个真 bug**：`readyz` 原会 panic 而非返回 503（`config::get()` 的 `expect` 深埋在 `QdrantClient::new()` 里），已加防护。`self_healing` 的假时延已改为 `null` 并加 `is_dependency_probe: false`；CLAUDE.md 已同步 | W4 | #2 |
| **对账扫描器零调用者** | ✅ **完成（已提交）**。`init_reconciliation_scanner` 在 `main.rs:116` 被调用（PG-only，紧随 outbox worker）。含 `ReconciliationConfig`、**6 个 gauge**、13 项测试。默认 `dry_run` 只读 + 60s 间隔下限 + 60s 首扫延迟；扫描失败只记日志不退出循环。dead_code 警告 8 → **0**。**补了停摆检测**：四个漂移 gauge 在两次扫描间保持旧值，故扫描循环卡死时它们仍读「平静」——兜底自己死了却无人知晓。新增 `reconciliation_last_scan_timestamp_seconds`，**仅在扫描成功时 stamp**（失败时 stamp 会把「每轮都失败」误报成健康），启动时 seed 一次（未设时读 0 会让告警每次开机就触发）。⚠️ **未做真机验证**：四类漂移检测未经实跑确认 | W4 | #1 |
| **多数指标已注册但从未写入** | `set_outbox_pending` / `inc_outbox_dead_letter` / `set_tenant_quota_usage` / `inc_outbox_qdrant_upsert_*` / `set_ltm_entries_total` / `set_stm_sessions_active` / `record_request` / `record_search_duration` 在 exporter 之外**零调用者**，指标恒为 0。仅 `record_outbox_processing_duration` 与 `set_reconciliation_*` 有真实写入。⚠️ closeout 与本文原先都称这些指标「质量不错、均已导出」——**第三处同类失真**：字面成立（出现在 `/metrics`）但无信息量 | 2026-08-11 新发现 | #39 |

---

## C. 治理与多租户缺口

| 项 | 现状 | 任务 |
|---|---|---|
| 治理层未覆盖多个路由 | ✅ **完成（已提交）**。**原表述不准**：这 5 组不是均匀裸奔。`/billing`、`/snapshot`、`/memory-pool` 的 `governance_middleware` **早就挂在链上**，缺口在 `classify()` 对这些路径一律返回 `None` → 直接放行——**中间件在跑，只是什么都不分类**（又一处「存在≠起作用」）。只有 `/tenants`、`/v1/agents` 是真没挂中间件。另发现根因：`GovernanceHookImpl` 只实现 `pre_store→Write` / `pre_search→Read`，`pre_update`/`pre_delete` 是 trait 默认 `Allow`，所以 `Operation` 那条路**永远表达不了** `ManageBilling`/`ManageTenant`。新增强类型通道 `classify_permission(method, path) -> Option<Permission>`：billing→`ManageBilling`（仅 Owner）、tenants DELETE→`DeleteTenant`、tenants 其他→`ManageTenant`、snapshot→`ManageMemory`、memory-pool 与 agents→`ManageAgents`。8 项测试含防漂移守卫。不依赖 enterprise hooks 是否初始化，故 SQLite dev 下同样生效。⚠️ **今日行为基本不变**（每人是自身租户 Owner），真正生效待 C-3 | #32 |
| `claim_batch` 无租户公平性 | outbox 消费**进程全局**，一个租户堆积会拖慢**所有**租户的向量索引。当前设计如此，非 bug | #13 |
| RBAC 尚不能区分权限 | 每人都是自己单用户租户的 Owner。真正的角色区分需要 **org 层租户模型**（`tenant_id` 与 `user_id` 解耦），属架构级变更，需先出 ADR | #14 |
| SQLite 模式无任何 DB 层隔离 | ✅ **完成（已提交）**。`db.url` 为空时的静默降级已改为**默认拒绝启动**：新增 `db.allow_sqlite_fallback`（默认 `false`）+ `APP_DB_ALLOW_SQLITE_FALLBACK`，未显式 opt-in 则 `exit(1)` 并给出三条修复路径；opt-in 时打印醒目横幅（同 `jwt.disabled`）。**顺带发现并修了一个更严重的问题**：`config::init()` 在 `main.rs:54` 运行、`otel::init_tracing` 在 `:66`，所以原先那句 `tracing::warn!` 发生在 subscriber 建立**之前、被静默丢弃**——降级此前连警告都没有。已改用 `eprintln!`（与 main.rs 自身的 startup 报错一致）。2 项测试 + 对照组验证（默认改 true 后守卫确实 FAILED）。⚠️ SQLite 本身仍无 DB 层隔离，这一项只关闭了「静默」，未改变 SQLite 的隔离能力 | #15 |

---

## D. 实测撞出的候选（非代码审查所得）

1. **DB check 约束以 500 而非 400 暴露给调用方**（#18）：`session_type` 仅接受 `conversation`/`task`/`query`，`source_type` 仅接受 `document`/`api`/`database`/`web`/`user_input`；传非法枚举值得到「内部错误」，集成方无从排查
2. ✅ **完成**（#19）**`config.toml` 与 `local.toml` 的 embedding 模型不一致**（768 vs 1024）。**原表述有偏差**：`local.toml` 是 gitignored 的本地覆盖，不是提交的配置档；真正的冲突源是两者共用 `collection_name = "long_term_memory"` 导致向量空间碰撞。已在 `config.toml` 注明「换 embedding 模型必须同时换 collection_name」，并让 `vector_guard` 报错给出**非破坏性出路**（改名共存）而非只教人丢数据
3. ✅ **完成**（#20）**`vector_guard` 错误提示不完整**：已补上签名文件绝对路径（`sig_path.display()`），并明确恢复需**两步**（丢 collection + 删签名条目），同时优先推荐非破坏性的改 collection 名方案
4. ✅ **完成**（#21）**Neo4j「可选」在运行上不成立**。已移出启动关键路径：新增 `spawn_neo4j_init`（`OnceLock` 幂等 + `tokio::spawn` + 10s 总超时），HTTP 监听不再等它。**核实推翻了原表述的两点**：(a) `init_neo4j_indexes().await.expect(...)` **永远不会 panic**——该函数无论如何都返回 `Ok(())`，那个 `.expect` 是死保险；真正的问题是它串行发 3 条 query，每条最多被 neo4rs 的 60s 退避阻塞（`pool.rs:29-34` 的 `max_elapsed_time=60s`），最坏 **~180s**。(b) `Graph::new` 只建**惰性**连接池、根本不发起 I/O，所以原先紧随其后的 "Successfully connected" 日志是在**没有任何连接**时打印的——又一条虚假日志，已加真实连通性探针（`RETURN 1`）后才打。占位符密码按「拒连 + 告警 + 继续」处理（不 `exit(1)`——Neo4j 是可选依赖，与 JWT secret 作为认证核心必须 fail-fast 的语义不同）。新增 `Neo4jStatus{Disabled|Connecting|Connected|Failed}` 使真实状态可观测。**故意未接入 `/readyz`** 并把该决策写进 doc 注释防后人误接（可选依赖挂掉不该把健康实例摘出 LB）。4 项纯逻辑测试。⚠️ 「错密码不再阻塞几十秒」是**代码结构层面**结论，本机无 Neo4j 未实测时序
5. ✅ **完成（已提交）**（#22）**Ollama 不可达时 LTM 写入完全不可用**。已按拍板方案把 LLM 摘要降级为可选。**核实推翻了原表述的两点**：(a) **没有独立 summary 列**——摘要直接存进 `content`（`NOT NULL`），`content_hash = sha256(summary)`，关键词搜索走 `content ILIKE`，embedding 拿 `content` 向量化。所以「摘要降级」不是「某字段留空」，而是**存入内容、哈希、向量全都不同**；因此用新列 `summary_status` 而非 `content IS NULL` 判断（后者不成立，降级时 content 非空）。(b) LTM 写入在热路径上调 Ollama **两次**（摘要 + embedding），且 **embedding 拿摘要去向量化**——所以摘要降级单独做**并不能**让「Ollama 不可达时写入成功」在单 Ollama 部署下成立，缺口见 D-e2（#61）。错误分类边界：`Unavailable`/`Upstream` → 降级但留 WARN + `ltm_summary_degraded_total{reason}` 指标；**`Malformed`（可达但返回垃圾）→ 仍 500**；embedding 失败 → 仍 500。唯一从 500 变成功的只有 LLM 不可达/上游错误且都可观测。⚠️ 运行时降级行为无 Ollama 未实测；补摘要后台任务**明确未做**（另开工作项），但已提供 `list_entries_pending_summary` 查询入口 + 部分索引。**补摘要时必须重算向量**（摘要变则 content 变则 hash 与向量都要变），已在代码注释留痕
6. ✅ **完成**（#23）**两个 doc 示例编译不过**：`tenant/context.rs`、`runtime/langchain_adapter.rs` 已修好并去掉 ```` ```ignore ````，`cargo test --doc` 从 **0 运行变为 2 运行**
7. ✅ **完成**（#17）**「参数被接受、被忽略、无报错」全面排查**。撞出 **4 处真实缺陷**：
   (a) `multimodal::get_mm` 绑定 `Query<LimitQuery>` 后整个丢弃——该端点按 `entry_id` 取单条，`limit`/`tenantId` 本无意义，已改为不绑定；
   (b) `visualization::TimelineQuery.layer` 在 **三个** handler 里全都不读，而 `get_timeline` 响应里硬编码 `layer: "ltm"`——`?layer=kg` 被接受、无效、无错，调用方拿到 LTM 数据且标着 `ltm`。已删除该字段（实现跨层过滤等于新造功能，不属修 bug）；
   (c) `dashboard::TimeRangeQuery` 的 `start`/`end` 绑定为 `_query` 全丢，任何时间窗请求都静默返回全时段指标。`MetricsEvent` 本就带 `timestamp`，故已接上过滤，并**明确单位为 epoch 毫秒**（原先单位未声明，是「实现它」之前必须先定的事）+ 4 项边界测试（含空区间必须放行全部，否则「省略 query」与「传空 query」行为会不一致）；
   (d) `workflows::ApprovalCallbackRequest.reason` 被接受，但 `ApprovalManager::approve` **硬编码 `None`**——审批理由永久丢失，记录只有「谁批的」没有「为什么批」，是**审计缺口**。`resolve_approval` 本就接 `Option<String>`，已打通 + 2 项测试，对照组（改回丢弃）实测会 fail。
   另核实 3 处 `memory_storage` 的 `tenantId` body 字段是 P0-7 认定的**安全模式**（handler 按 `tenant_ctx` 作用域、故意不读），非缺陷；但契约上仍宣称可设置，留作观察项。
8. ✅ **完成**（#24）**8 份 ADR 全为 `Proposed`**。已逐份核实实现真相并收口。**关键改进**：把原先挤在一个字段里的两件事拆成两个正交字段——`状态`（只描述决策是否采纳：Accepted/Proposed/Superseded/Rejected）与 `实现状态`（只描述代码：已完整落地/部分落地/未落地）。挤在一起会让扫状态表的人只看到 `Accepted` 就以为做完了，那正是本轮整改要修的病。结果：0001/0002/0004 = Accepted+部分落地；**0003/0007 = Accepted+未落地**（0003 门禁本身 blocked，未通过的门禁功能上等于无门禁；0007 鉴权核心已统一但 HTTP 之外无一协议被真正服务）；**0005 = Accepted+已完整落地**（唯一一份）；0006/0008 保持 Proposed。无 Superseded/Rejected——已落地的都按 ADR 方向落地，未落地的是尚未做而非走了替代方案。8 份均补 `Owner` + `收口责任人`；0006/0008 如实写「待 DRB 收口（仍 open）」而不假称已收口
9. ✅ **完成**（#16）**统一 query 参数命名 + 契约测试**。⚠️ **原表述被核实推翻**：不是「7 个 `*Query` 中 3 个 rename 为 camelCase、4 个 snake_case，Axum 匹配不上即静默丢弃」，而是——(a) rename 是**逐字段**的而非 struct 级（`ExplainQuery` 的 `traceId`/`taskId`、multimodal 的 `tenantId`），**同一 struct 内两种命名混用**；(b) **当前没有任何一处参数是真的传错的**，那 4 个 bug 前几批已修完，前端 typings、Python SDK（`explain()` 且有自己的契约测试）、Rust SDK 与各自对应的结构体**都已对齐**。真实缺口是「两种命名并存 + 无强制机制，下一个人会再踩」。
   **方案（已拍板）**：**不改任何现有命名**——三处 camelCase 各有活着的消费方，改名是对外 breaking change，为整齐而破坏集成方不划算。改为：(1) 给所有 query 结构体加 `deny_unknown_fields`，让拼错的参数**返回 400 而非静默丢弃**，直接堵住根因；(2) 11 项契约测试经 axum **真实 extractor**（`Query::try_from_uri`）固定每个端点接受的拼写，并各自注明消费方；(3) 3 个结构化防漂移守卫。
   **守卫扫出 16 个 query 结构体**（比初判的 9 个多 7 个——是守卫自己发现的）+ 2 个带说明的豁免：`TokenQuery`（`/login/account` 的 URL 会经邮件客户端被追加 `utm_*` 等参数，严格化会让带追踪参数的链接登录失败；且它只有一个参数、缺失时不会静默返回窄结果而是走 body 凭据或显式 401）、`WorkflowEvidenceQuery`（空结构体，无字段可供「丢进去」，加严只会无谓拒绝）。豁免需写 `ALLOWS_UNKNOWN_QUERY_PARAMS:` 注释说明理由，且**豁免总数被 pin 住**，多一个必须是有意识的编辑。
   **三个守卫均经对照组实测会 fail**。过程中**修掉一个「守卫不咬人」的 bug**：最初把注释和属性放在同一个窗口里做 `contains` 检查，而多个结构体的 doc 注释里正好**解释**了 `deny_unknown_fields`——于是删掉真属性、测试照样绿。已改为注释与属性分开取（`partition`），因为这两个信号一个在代码里一个在注释里，混在一起会让守卫失去失败能力。这正是本轮整改要消灭的「门禁存在但不起作用」，只是这次出现在我自己写的门禁里。
10. **（新发现，未修）`detect_skill` 是关键词猜测，写意图会被静默降级为读**：`a2a/handler.rs` 的 `detect_skill` 按 `text.contains("store"/"remember"/"save")` 判断写意图，`else` 分支返回 `Some(MemorySearch)`。所以「persist this」「keep this note」这类措辞会被分类为**搜索**——**不构成写门禁绕过**（到不了写 handler，A-4c 已核实这点是 fail-safe），但用户以为写入了、实际只做了一次无害的读，属**功能正确性/数据感知**问题。`skills.rs` 已有 `MemorySkill::from_id`，但 `handle_message` 没用它、走的是关键词猜测。建议：要么无法识别时归 `None`（fail-closed 到 `general_query`），要么改用显式 skill_id。


---

## E. 技术债（较低优先）

- ✅ **完成**（#26）**双 JWT 已收口**：`web/jwt.rs` 整文件删除（含 `web/mod.rs` 的 mod 声明与 re-export）。核实为**零代码调用者**——所有 `JwtClaims`/`auth_middleware`/`decode_token` 生产用点走的都是 `hoops::jwt::*`；`crate::web::` 唯一被消费的是 `cors_layer`。`hoops/jwt.rs` 是其**严格超集**（多出 query-token 拒绝、租户上下文注入、`jwt.disabled` dev 模式、鉴权失败指标、显式过期检查）；唯一结构差异是 `JwtClaims` 多一个 `iat` 字段，全仓无读取方。全仓无 WebSocket/img/下载链接依赖 query-string token，删除不会让任何功能静默失效。这同时闭合了 ADR-0007 的「违反项」（第二条平行鉴权路径不再存在）
- ✅ **完成**（#28）`sdks/rust` 的 `urlencoding` 依赖已移除，改用 `reqwest` 的 `.query()`；`cargo tree` 确认 0 命中，7 项测试通过。**已知编码差异**：空格从 `%20` 变为 `+`（form-urlencoded），二者对标准解析器等价且已加测试固定
- ✅ **完成**（#29）`search_knowledge_by_entity_for_tenant` 的无用 `LEFT JOIN knowledge_entries ke` 已删除。`DISTINCT` **保留**——真正制造重复行的是 `LEFT JOIN relations r`（按 source 或 target 匹配，一个有 N 条关系的实体产出最多 N 个重复 `e` 行）。⚠️ 该函数用运行时 `query_as` 而非 `query!` 宏，故 `cargo check` 通过**不校验 SQL**；结果集等价性为**静态推理**，本机无 PG 未实跑
- （#25）手写 OpenAPI → utoipa 宏自动生成（当前只覆盖 MVP 子集，Scalar 不是权威清单）
- （#26）~~双 JWT：`web/jwt.rs`~~ → 见上，已完成
- （#27）~~`ci.yml:158` 前端 `npm run lint` 仍 `continue-on-error: true`~~ → 已完成（E-3）；PR #79 的 `frontend` job ✅ 2m8s 证实它在 CI 上确实阻塞且通过
- （#28）~~`sdks/rust` 新增 `urlencoding` 依赖~~ → 见上，已完成
- （#29）~~`search_knowledge_by_entity_for_tenant` 无用 `LEFT JOIN`~~ → 见上，已完成
- （#33，**2026-08-11 新发现**）CI clippy 无 `-D warnings`：`ci.yml:102` 是 `cargo clippy --all`，只去掉了 `continue-on-error`，警告一律放行。closeout-summary 称「clippy 改为阻塞」**表述不准**。前提是先清理存量警告（`cargo check` 516 条 / `cargo clippy --all` 628 条）
- （#34，**2026-08-11 新发现，已完成**）✅ `cargo fmt --all -- --check` 门禁。**我上轮的表述是错的**：我说「CI 显示通过」，但核实后发现**分支从未推送、远端无此分支、CI 从未运行过**——那是我从证据缺失里推出的结论，不是核实的（与 §4.5 同类错误）。真实情况是首次 push 时 CI 必定失败。已修：`cargo fmt --all` 把 65 处偏差清零；并修根因——新增 `rust-toolchain.toml` pin 到 `1.96.1`，`ci.yml` 从 `dtolnay/rust-toolchain@stable` 改为 `@master` + 显式版本。此前无 pin，CI 的 `@stable` 在运行时解析，与贡献者本地 rustfmt 可以不同版本，这正是偏差能被提交进来的机制。**2026-08-12 追记**：上面那句「首次 push 时 CI 必定失败」是在偏差清零**之前**成立的预测；清零 + pin 之后，PR #79 的首次真实 CI 运行六个 check 全绿，该预测未兑现——记录在此以免它被后人当成仍然成立的现状
- （#63，**已完成**）✅ E-8 `DocContent` 的 DOMPurify 配置用 `ADD_ATTR: ['target']` 允许 `target` 但未补 `rel="noopener noreferrer"`（反向 tabnabbing，纵深防御而非活漏洞——现代浏览器对 `target=_blank` 已默认 noopener）。已加 `afterSanitizeAttributes` hook，**合并而非覆盖**已有 rel token
- （#69，**已完成**）✅ E-9 `TaskAnalysis` 提交按钮未接 loading 态。只用 `loading`、**刻意不加 `disabled`**——antd Button 的 loading 已拦截点击，而原生 `disabled` 只来自 `mergedDisabled`，加它会让按钮失焦到 body、结束后不回来
- （#70，**已完成**）✅ E-10 所有 503 响应带 `Retry-After`。非 503（4xx/500）**不带**并有测试断言——在永久性错误上放该头会误导调用方重试不会好转的失败
- （#77，**2026-08-11 新发现**）SDK example 用**非法枚举值**：`sdks/rust/src/models.rs` 的 `session_type` 用 `"chat"`、`source_type` 用 `"documentation"`，两者都不在 CHECK 允许集里。D-a 之前 `documentation` 被后端静默改写为 `user_input`、`chat` 撞 CHECK 报 500；D-a 之后都返回 400。**这不是 D-a 引入的回归，而是 D-a 暴露的既有缺陷**——SDK 一直在教用户传错值。另：Python SDK 在**客户端侧**也做静默重映射（`client.py:253-256`），是同一反模式换了个位置
- （#78，**2026-08-11 新发现**）另有 **6 个 CHECK 约束**以同样方式暴露：STM 消息 `role`、`modality_type`、`relation_type`、`config_type`、`correlation_type`、租户 `role`。应复用 D-a 建立的模式（`models/memory_enums.rs` 的 enum + `ALL` + `include_str!` 读 migration 的防漂移测试），并继承两条约束：**校验放 service 层**（多入口场景下类型层只保护 axum 反序列化那一条）、**enum 不进 row struct**（否则读取历史非法值会失败）
- （#79，**2026-08-11 新发现，安全项**）**a2a 路由未挂 `auth_middleware`**。`routers/mod.rs:452` 是 `api_router.merge(a2a_router(...))`，没有任何 auth 层——而 `auth_middleware` 正是注入 `Extension<RequestTenantContext>` 的地方。`a2a/streaming.rs:31` 要提取它，没注入就 500。实测 `cargo test --features a2a --test a2a_integration` 得 **3 passed / 4 failed**（4 个全是 500 != 200）。这 7 个测试从写下来到 2026-08-11 一次都没跑过（文件级 `#![cfg(feature = "a2a")]` + CI 无 a2a 通道 = 编译为空、静默消失）。⚠️ **不要只补 extension 注入而不挂 auth** —— 那会留下一组无鉴权的 a2a 端点，比现在的 500 危险得多。这也是 ADR-0007「统一鉴权核心」的又一实现缺口
- （#80，**2026-08-11 新发现**）**前端 jest 基建整体不可用**：`jest.config.ts` import `@umijs/max/test` 报 `ERR_MODULE_NOT_FOUND`（既有状态，非新引入）。**所以仓库当前没有任何可运行的前端测试。** 另有孤儿快照 `src/pages/user/login/__snapshots__/login.test.tsx.snap` 无对应测试文件。这是「门禁存在但不起作用」的同类：有配置、有快照、有 test script，但一条都跑不了。lint 不能验证行为——E-8 的 rel、E-9 的 loading 都无法用测试固定
- （#71，**2026-08-11 新发现**）D-e3 补摘要后台任务。查询入口已就绪（`list_entries_pending_summary` + 部分索引），但**必须同时重算向量**：Complete 的向量是 `embed(摘要)`、Pending 的是 `embed(content)`，只填摘要文本会让该条目向量永久停在 content-derived、与语料库其余部分不一致。完整流程需在同一事务内更新 hash 与入队 outbox，否则对账会检出假的 `content_hash_mismatch`

---

## F. 首次 CI 运行结果（2026-08-12）

`fix/truthfulness-and-security-remediation` 此前**从未推送过**，所以 **CI 一次都
没跑过**——这一点更早曾被误述为「CI 显示通过」，见 §E 的 #34。

2026-08-12 已推送并开 PR **#79 → `dev`**，**六个 check 全绿**：

| job | 结果 | 它回答了什么 |
|---|---|---|
| `backend` | ✅ 10m1s | 1032 passed / 0 failed / **0 ignored**，且全日志 0 次 `SKIP` |
| `backend-a2a` | ✅ 4m1s | a2a-rs 的 pinned rev 在 runner 上拉取成功，7 个测试首次在 CI 上跑 |
| `sdk-rust` | ✅ 52s | **新增 job 的首次执行**；该 crate 此前从未被任何 job 编译过 |
| `frontend` | ✅ 2m8s | 阻塞式 lint 在 CI 的 `npm ci --legacy-peer-deps` 依赖树下同样 exit 0 |
| `build` | ✅ 2m1s | |
| `changes` | ✅ 7s | |

⚠️ **注意 CI 的触发条件**：`ci.yml` 只在 `push` 到 `main/master/dev` 或 `pull_request`
指向它们时运行，**没有通配分支触发**。所以推 feature 分支跑不了任何 job——
「已推送」不蕴含「已验证」，必须开 PR。

三个「只有 CI 能回答」的问题都已答：`sdk-rust` job 可用、两个 job 的 cache path
改为 `backend/target` 后未破坏构建、a2a-rs 的 git 依赖可拉取。

分支现有 **30 个 commit**（`origin/dev..HEAD`）：更早会话 10 个 + 2026-08-11
四批共 20 个。**尚未合并到 `dev`。**

第四批曾一度全部滞留在工作区（28 文件 / +1636 −184 未提交），已于 2026-08-12
拆成 5 个 commit 落地：指标接线（B-5b）、query 契约（D-i+D-g）、a2a governance
（A-4c）、a2a SSE 修复（E-12）、SDK 入 CI（E-7b）。**拆分后的中间提交经
`cargo check --tests` 逐个验证可独立编译**，不是只保证最终态可用的假历史。

**首次 push 前的注意事项**（1/2/5 已由 PR #79 的实跑结果回答，保留作记录）：

1. ~~`backend-a2a` job 会首次拉取 a2a-rs 的 pinned rev（需网络）~~ → ✅ 拉取成功，
   4m1s 通过。它刻意没有 `continue-on-error`，是独立 job，不牵连 `backend`。
2. ~~前端 lint 已成阻塞门禁，CI 用 `npm ci --legacy-peer-deps`，版本解析可能与
   本地不同~~ → ✅ CI 下同样 exit 0（2m8s）。
3. `cargo fmt` 门禁依赖 `rust-toolchain.toml` 的 `1.96.1` 与 `ci.yml` 的显式版本
   一致。改任一处都要同步另一处。**`sdk-rust` job 也 pin 了同一版本**，
   三处必须一起改。**（仍然有效）**
4. 存量库的运维前提不变：必须先
   `ALTER ROLE aetheris_app WITH LOGIN PASSWORD '<managed-secret>'`，
   否则连接报 `password authentication failed`（见 closeout-summary §6）。
   **（仍然有效；CI 用的是一次性容器，不覆盖这条）**
5. ~~`sdk-rust` job 首次覆盖该 crate，在 GitHub runner 上从未编译过~~ → ✅ 52s 通过。

**明确不宣称生产就绪。** 累计关闭 4 个生产阻塞项与 2 个 P0 跨租户泄漏，
但仍有 10 项待处理，其中 C-3（org 层租户模型）阻塞着 A-1/C-1/P0-6/P0-7 四项
已完成工作的实际可观测效果，且大量行为**未经真机验证**（本机无
PG/Qdrant/Neo4j/Ollama/promtool；docker 可用但未起这些服务）。

**第四批未实测的边界**：a2a governance 的 quota 门（gate 2）依赖 enterprise
hooks，默认构建下未初始化即 no-op，其真实拒绝路径未跑；两个库存 gauge 的
`COUNT` 在**加固后的 NOSUPERUSER/NOBYPASSRLS 角色**下、无 `aetheris.tenant_id`
GUC 时会被 RLS 过滤成 **0**（静默 0，比对账的「全 missing→告警」更隐蔽，见
`aetheris-rls-app-role-not-owner`）；`record_request` 的真实基数规模未在活负载下
观测（已用 `MatchedPath` 路由模板 + `unmatched` 哨兵在结构上排除无界 label）。

---

**最后更新**：2026-08-12（第四批 5 个 commit 落地；PR #79 六个 check 全绿，分支首次接受 CI 验证；38 项完成 / 新发现 17 项 / 本批只做后端，前端推后）
**更新角色**：tech-lead
