# Aetheris MemOS 技术讲解文档（内部）

> **用途**：给新加入的同事讲清楚三件事 —— 这个项目的**核心设计**、**对外能力范畴**、以及**哪些代码可以信赖 / 哪些还需要迭代**。
>
> **成文方式**：本文所有结论都是**读源码逐条核对**出来的，不是照抄仓库里的状态文档。仓库里的
> `IMPLEMENTATION_STATUS.md`、`docs/ROADMAP.md`、`docs/artifacts/2026-08-03-*/project-summary.md`
> 与 `CLAUDE.md` 存在**明显高估和互相矛盾**的地方，本文会逐条指出。**以本文为准，不以那些文档为准。**
>
> - 核对基线：`dev` 分支 `a9a3e5a`，默认 `cargo build`（`default = []`）
> - 成文日期：2026-08-10
> - 规模：backend Rust 约 **55.8k 行** / 190 个 `.rs` 文件；默认构建注册 **157 个 HTTP 路由**

---

## 0. 结论先行（先记住这 6 句）

1. **真正工程质量过硬的有两块**：LTM 写入的 **transactional outbox**（跨 Postgres/Qdrant 最终一致）和
   **Evidence Graph**（决策哈希链留痕、可验签、可导出审计）。这两块可以放心讲、放心用。
2. **代码库里有两套并行现实**：真实跑的是 `routers → services → db/*`；
   旁边还有一整套 `kernel/ + layers/ + providers/ + agent/` 的漂亮抽象，**是内存 stub，线上主链路完全不走它**。
   不认清这点，读代码会严重跑偏 —— 这是新同事**第一个必须建立的认知**。
3. **"自适应/自学习"目前是静态规则**。analyzer / predictor / scheduler / weight 全是写死系数，
   **没有任何从遥测数据学习的闭环**。对外表述请用"启发式配置选择"，不要用"自学习"。
4. **对外真正可达的协议只有 2 个**：REST 和 MCP-over-HTTP。
   gRPC / WebSocket / A2A **都不可达**（无 server / 未挂载 / feature 默认关闭），
   尽管 `project-summary.md` 声称"四传输适配器全部实现"。
5. **多租户隔离目前仍不是数据库强制的**。RLS 策略写得是对的（已实测有效），但**默认仍以 Postgres
   超级用户连库，RLS 是 no-op**，实际依赖应用层纪律。这是上生产前的头号问题。
   地基（受限角色、迁移解耦、代码绕过口、admin 池）已全部就绪并实测可用，**只差把连接串切过去**——见 §0.5。
6. ~~**治理（RBAC/配额）处于"要么形同虚设、要么直接卡死"的状态**~~ **已修（2026-08-10）**：
   RBAC 可通过（403 → 200 实测）、配额真正计数、默认 fail-closed、MCP 写路径已纳入治理。见 §0.5。

> ⚠️ **阅读顺序**：§0 描述的是本文最初成文时（2026-08-10 整改前）的基线判断，用于建立认知；
> **当前状态一律以 §0.5 为准**。§5 的 P0 清单同理——其中 P0-2/3/4/5 已修，P0-1 待 ops 决策。

---

## 0.5 整改进展（2026-08-10，附证据）

> 本节记录已落地的修复，避免本文在**反方向**上过期（把已修问题继续描述为未修）。
> 每条都有可复现证据；**没有实测证据的不写在这里**。

### ✅ 已修并验证

| 项 | 证据 |
|---|---|
| **测试假绿**（6 个 DB 门控测试静默报 ok） | 现如实报 `ignored`；有 DB 时 `--include-ignored` 真跑并通过。本地 `cargo test` 从「286 passed / **3 failed**（硬 panic）」变为「**0 failed**」 |
| **`db/mm.rs` 租户 fail-open**（原 §5 P0-2） | 11 处裸连接池分支、`begin_optional_tenant_tx`、无租户过滤的 `count` 全部删除；租户改必填；空/空白租户 fail-closed 报 400。**8/8 public 方法**均校验租户且走 `begin_tenant_tx` |
| **`aetheris_app` 角色在存量库不存在** | 角色创建从 `docker/initdb/`（仅空数据目录执行）搬进 sqlx migration；实测 `super=false bypassrls=false` |
| **迁移与应用启动耦合** | `auto_migrate` 默认 `false`，改为**校验 schema 并 fail-fast**（实测 `pending=1 versions=[...]` → 拒绝启动）。受限角色现可完成 DB 初始化 |
| **管理员跨租户回填会被 RLS 挡住** | 独立 admin 池（方案 A），`admin_url` 未配置时端点返回明确错误 = 默认关闭；启动时 `warn!` 提示 BYPASSRLS 池已激活 |
| CI 强化 | 新增 Qdrant service；clippy 与 cargo audit 改为**阻塞**；删除重复的 `backend-ci.yml` |
| 文档 / SDK / 前端漂移 | CLAUDE.md 端点校正、ROADMAP 分布式表述校正、SDK 三个 bug、前端 auth 路径 4 处 |
| **P0-4 治理形同虚设 / 卡死** | RBAC 惰性自动授予自身租户 Owner（`tenant_id == uid`，每用户即自己的租户）。**实测前后对照**：`/memory/search/ltm`、`/kg/entities`、`/mm/list` 从 **403 → 200**，且响应均为空且租户作用域正确（`total:0`）——治理放行的同时隔离仍生效。顺带修 `try_read()` 锁竞争导致的随机 403 |
| **配额永不计数** | 中间件补调 `post_store`（仅 `Store` 且 2xx）。**实测** `new_used=1 → 2` 与成功写入次数精确对应；四种失败模式（422/404/500×2）均未计数 |
| **治理默认 fail-open** | 默认翻转为 fail-closed；`GOVERNANCE_FAIL_CLOSED=false` 仍可显式退出。**关键防护**：无法识别的取值（`TRUE`/`1`/`yes`/空串）保持 fail-closed，不会静默降级安全等级 |
| **auth 关闭时静默无治理** | `jwt.disabled=true` 时启动打无法忽略的告警框（auth 与 RBAC 均被跳过、单一 anonymous 租户、禁止用于生产）。**短路本身保留**——移除它会让 `local.toml` 下每个请求 403，打挂本地开发 |
| **P0-5 MCP 写路径绕过治理** | 在 `call_tool` 内按工具名执行治理（**不能改 `classify()`**：MCP 单路径复用所有操作，统一归类会让 search 被当写入计费）。`write→Store`／`forget→Delete`／`search·recall·list→Search`，未知工具 `None`；Deny 返回 403 与 REST 一致；仅 `Store` 且成功才 `post_store`。含防漂移守卫测试 |
| **P0-3 限流可被 `X-Forwarded-For` 绕过** | 两处 serve 注入 `ConnectInfo`；新增 `trusted_proxies`（默认空 = 永不信任 XFF）；可信 peer 才查 XFF 且**从右往左**跳过代理取首个非可信项。**实测攻击验证**：14 次登录请求各带不同伪造 XFF（限流 10/60s）→ `401×10` 后 `429×4`，伪造失效。`/register` 补限流 |

**RLS 策略本身已实测有效**（关键：有对照组才有意义）：

| 连接 | GUC | 可见行 | 结论 |
|---|---|---|---|
| `aetheris_app` | 未设 | **0** | fail-close |
| `aetheris_app` | `probe_a` | **1** | 精确按租户过滤 |
| `memory`（超级用户） | — | **2** | 绕过 RLS ← 这就是下面那条未修项 |

测试基线：`cargo test` 稳定 **752 passed / 0 failed / 14 ignored**（整改前为 286 passed / 3 failed）。

### ⬜ 仍未修（不要当成已解决）

| 项 | 现状 |
|---|---|
| **P0-1 应用仍以超级用户连库** | 🟡 **技术就绪但未生效**。角色、迁移解耦、代码绕过口、admin 池都已就绪，并**已实测**应用能在 `aetheris_app` 下完整启动并服务请求。但 `backend/config.toml` 与 `docker.toml`（**两者均已被 git 跟踪**）仍指向 `memory`，所以**默认部署的隔离仍未生效**。剩余阻塞是一个 ops 决策：`aetheris_app` 的密码来源（`ALTER ROLE` / compose secret / secrets manager）。容器网络连接必须有密码——实测 `fe_sendauth: no password supplied` |
| W3-6 治理层扩面 | `/billing`、`/tenants`、`/snapshot`、`/memory-pool`、`/v1/agents` 有鉴权但**无治理层**，计费与租户管理无角色门禁 |
| W3-7 审计可靠性 | fire-and-forget、静默丢弃、无重试背压；SQLite 下审计全丢 |
| W4 可靠性 | outbox 重放/幂等**零行为测试**；对账扫描器已实现但零调用者；liveness/readiness 探针不存在；无告警规则与 dashboard |
| W5 剩余 / W6 决策项 | 手写 OpenAPI → utoipa；ADR 全为 `Proposed` 待收口；7 项做-or-删决策未拍板 |

**本轮新增的 W4 候选**（实测撞出，非代码审查所得）：

- **DB check 约束以 500 暴露给调用方**：`session_type` 只接受 `conversation`/`task`/`query`、`source_type` 只接受 `document`/`api`/`database`/`web`/`user_input`，传非法枚举值返回 **500 而非 400**，集成方无从排查
- **`config.toml` 与 `local.toml` 的 embedding 模型不一致**（768 vs 1024），切换配置必然触发 `vector_guard` 并拒绝启动
- **`vector_guard` 的错误提示不完整**：它建议 "Drop the Qdrant collection and re-index"，但签名实际存于 `~/Library/Application Support/adaptive-memory/vector_signatures.json`，只删 collection 无法恢复
- **Neo4j "可选" 在运行上不成立**：`init_neo4j_indexes` 确是 best-effort，但 neo4rs 的重试退避**同步发生在启动主线程**，一个错密码能让服务器几十秒起不来；且 `config.toml` 的占位符密码不像 JWT 那样 fail-fast，只是静默重试
- **Ollama 不可达时 LTM 写入完全不可用**（依赖 LLM 摘要，返回 500）

**本地环境已知问题**（非代码缺陷）：Qdrant collection 与当前 embedding 配置维度不一致时
`vector_guard` 会正确 fail-fast 并拒绝启动。这与 DB 角色无关，两个角色都会命中。


---


## 1. 项目定位

给 AI Agent 做**记忆基础设施**：把"短期记忆 / 长期记忆 / 知识图谱 / 多模态记忆"四层统一到一套 API 之后，
再叠一层"按任务特征选择记忆配置"的决策管线，并把决策过程留痕成可审计的证据链。

技术栈：**Rust (Axum 0.8) + PostgreSQL + Qdrant**（+ 可选 Neo4j / Redis）+ **React (Umi 4 + Ant Design Pro)**。
Embedding / LLM 走 Ollama 或 DashScope（OpenAI 兼容格式）。

| 层次 | 内容 |
|---|---|
| 应用层 | AI Apps / Agents / Workflows |
| 运行时层 | LangChain / 自研编排器 |
| **记忆层** | **← Aetheris MemOS 在这里** |
| 模型层 | Ollama / DashScope / OpenAI |
| 基础设施 | PostgreSQL / Qdrant / Neo4j |

---

## 2. 核心设计

### 2.1 请求主链路

```
Client
  └─> Router (Axum)            仅协议、校验、错误映射
       └─> Middleware          auth(JWT) → rate_limit → governance
            └─> Service        业务编排（scheduler / memory_search / memory_storage …）
                 └─> db/*      Repository（Postgres / Qdrant / Neo4j）
                      └─> Decision Trace → Evidence Graph（决策留痕）
```

启动时拉起的后台任务（`src/main.rs`）：

| 后台任务 | 条件 | 作用 |
|---|---|---|
| `outbox_worker` | 仅 Postgres | 消费 vector_outbox → 投递 Qdrant |
| `audit_writer` | 仅 Postgres | 异步批量写审计事件 |
| `memory_transfer` | 总是 | STM → LTM 自动晋升 |
| `memory_ingestion` 反思守护 | 总是 | 分层摄入 / 反思 |
| `information_guard` | 总是 | 写journal + 完整性扫描 |
| `write_queue` | 仅 SQLite | 合并并发写 |
| `strategy_mutator` | **已注释停用** | 见 §2.6 |

### 2.2 ⚠️ 最重要的认知：两套并行现实

这是本文**最关键**的一节。代码库里同时存在：

| | **A. 线上真实路径** | **B. 抽象层（未接线）** |
|---|---|---|
| 位置 | `routers/` → `services/` → `db/*.rs` | `kernel/` + `layers/` + `providers/` + `agent/` |
| 存储 | 真 Postgres / Qdrant | `RwLock<HashMap>` 内存 stub |
| 谁在用 | 所有 memory / search / storage / kg / mm 接口 | 只有 `routers/procedural.rs` 一个入口 |
| 状态 | 真实工作 | **`layers/ltm_layer.rs` 的 search 是 `contains()` 子串匹配、score 恒为 1.0** |

`layers/ltm_layer.rs:4` 自己写着注释："Currently uses in-memory storage; will integrate Qdrant + PostgreSQL in production."

而唯一用到 kernel 的线上端点 `POST /api/v1/memory/search/graphrag`，
内部构造的 `InMemoryVectorSearch::search_by_vector` **直接 `return Ok(vec![])`**
（`routers/procedural.rs:168-176`）—— 也就是说它在**空输入上跑真实的融合算法**。

**结论**：`kernel/` 是一套设计良好但线上完全绕开的抽象。
未来要么把它接上真实 repository（有价值），要么明确标注为实验区。
**新同事排查线上问题时不要读 `layers/`，要读 `db/`。**

同类"存在但未接线"的还有：
- **Neo4j**：启动时连接 + 建索引，之后**没有任何 router/service 调用它**。KG 实际是纯 Postgres。
- `services/hybrid_search.rs`：实现了**正确的 RRF**（Reciprocal Rank Fusion, k=60），但只接在上面那个 stub 端点上。
- `services/vector_reconciliation.rs`：对账逻辑**完整实现**（missing / orphan / tenant_mismatch / content_hash_mismatch），
  但**零调用者** —— 没有守护进程、没有路由。

### 2.3 四层记忆的真实存储

| 层 | 真实后端 | 关键表 / 说明 |
|---|---|---|
| **STM** 短期 | Postgres（Redis 适配器可选，feature `redis-stm` 默认关） | `context_sessions` + `session_messages`，会话级 |
| **LTM** 长期 | Postgres **+ Qdrant** | `knowledge_entries`（事实源）+ Qdrant（向量索引，异步） |
| **KG** 图谱 | **Postgres**（Neo4j 未使用） | `entities` / `relations`，带 bitemporal 列与 `confidence_score` |
| **MM** 多模态 | Postgres + Qdrant | `multimodal_entries`，含 text/image/audio_embedding 列 |
| Procedural | **无持久化** | 只有内存 stub，无 `db/procedural.rs` |

**双时间（bi-temporal）是真的**，而且对外可用：LTM/KG 都支持
`.../{id}/at`、`.../{id}/history`、`ltm/time-travel` 这些时间旅行查询。

### 2.4 写路径：transactional outbox ✅ 工程亮点

这是全项目最扎实的一段，按 `docs/adr/ADR-0002` 落地：

```
POST /api/v1/memory/storage/ltm
  └─ store_ltm_for_tenant()                      services/memory_storage.rs:141
      └─ begin_tenant_tx()                       开启事务 + 设置租户 GUC
          ├─ INSERT knowledge_entries            ← 关系事实
          └─ vector_outbox::insert_event_tx()    ← 待投递事件（同一个事务！）
      └─ COMMIT                                  返回 indexStatus: "pending"

           ⋮  异步解耦  ⋮

  outbox_worker（后台，仅 PG）                     services/outbox_worker.rs
      └─ reclaim_stale()  →  claim_batch()  FOR UPDATE SKIP LOCKED
          └─ Qdrant insert_vectors / delete_vectors
              └─ mark_applied() / mark_failed()（指数退避重试 → 死信）
```

设计要点，值得作为团队范式推广：

- **Qdrant 不在写请求的关键路径上**。Qdrant 挂了不影响写入成功，只是 `indexStatus` 停在 `pending`。
- **Postgres 是唯一事实源**，Qdrant 被定位成"可重建的索引"。
- **幂等**：`upsert_idempotency_key(entry_id, payload_hash)`。
- `FOR UPDATE SKIP LOCKED` 支持多 worker 并发消费不打架。

⚠️ 但注意两点：**对账扫描器实现了却没人调用**（§2.2）；**outbox 的重放/幂等逻辑没有任何行为测试**（§5）。

### 2.5 读路径：三路混合检索（真实算法是线性加权，不是 RRF）

```
POST /api/v1/memory/search/triple
  ├─ 向量路：Ollama embedding → Qdrant search（按租户过滤）→ 回 Postgres 补全行
  ├─ 关键词路：Postgres  content LIKE '%q%' → 1.0 分；title LIKE → 0.8 分
  └─ 图谱路：把整个 query 当 entity name 去 KG 查
        ↓
   融合 = vector*Wv + keyword_norm*Wk + graph*Wg     默认 0.5 / 0.3 / 0.2
        ↓
   cross-encoder rerank → 检索反馈调整 → 阈值过滤
```

需要对同事讲清的**真实边界**：

- **关键词路不是全文检索**，是 `LIKE '%…%'`。代码注释自己承认应该上 FTS。数据量大了会是性能问题。
- **融合是线性加权求和，不是 RRF**。项目里那份正确的 RRF 实现没接在这条线上（§2.2）。
- **图谱路很粗糙**：把整条 query 字符串当作实体名去匹配，没有做实体抽取。
- rerank 和检索反馈调整**是真的**，这两块有效。

### 2.6 决策链路（observe → decide → act）

```
TaskContext ─→ Analyzer  ─→ TaskCharacteristics / MemoryStrategy
Resource   ─→ Monitor   ─→ ResourceStatus
                ↓
             Predictor  ─→ PerformancePrediction（效率/连贯性/成本）
             WeightAdjuster ─→ MemoryWeights
                ↓
             Scheduler  ─→ MemorySelectionResult
                ↓
        Decision Trace ─→ Evidence Graph
```

**这条链路能跑通、对外可用，但内部全是写死的启发式**：

| 组件 | 真实实现 |
|---|---|
| `analyzer.rs` | 写死关键词表 `["分析","推理"…]` 命中 +0.3；`complexity = len*0.4 + hist*0.3 + kw`。注释自认"仍然是启发式" |
| `predictor.rs` | `new()` 里写死基线常量；`synergy = 1 + (layers-1)*0.1`；`confidence_score` 硬编码为 `None`；`TrainablePredictor::fit_from_samples` **只有声明没有实现** |
| `scheduler.rs` | 静态权重 `ltm=0.8, kg=0.7, mm=0.6`；资源约束时统一 `*= 0.85` |
| `weight_strategy.rs` | `ltm = (complexity*0.8).min(0.8)`；`performance_impact` 存了但**从不读回** |

**学习闭环整体断开**：`adaptive_telemetry` → `feature_pipeline` → `eval_harness` → `strategy_mutator`
四个组件**互不相连，也没有一个喂回 scheduler/predictor**。
`main.rs:106-116` 里把 `strategy_mutator` 守护进程注释停用了，注释写得很诚实：

> "its output is consumed by nothing — scheduler/predictor never read it — so running it only emits misleading 'self-optimizing' logs."

`eval_harness::run` 更直接：硬编码 `passed: true`、`coherence: 1.0`，从不真正调用 scheduler。

> **对外口径建议**：说"基于规则的可解释配置选择"。**不要说"自适应学习"或"自优化"** ——
> 技术尽调一翻代码就会发现，代价比收益大。

### 2.7 Evidence Graph：可审计决策留痕 ✅ 第二个亮点

每次持久化决策时构建一条**哈希链**：

- 每个 `WorkflowEvidenceNode` 计算 `node_hash = sha256(canonical payload)`，用 `prev_hash` 串联（区块链式）
- 锁定审计字段：`timestamp`、`attempt_id`、`llm_input_hash`、`llm_output_hash`、`tool_invocations`、`context_snapshot`
- `verify_chain()` 重算每个哈希 + 校验 prev_hash 链接 + 校验 `sequence_number` 单调 + 检测锁定字段被篡改
- 支持确定性导出（`exported_at` 故意排除在 canonical body 之外，保证同一记录可重复导出同样字节）
- 对外接口：`GET /api/v1/workflows/{id}/evidence`，返回 `run / nodes / edges / verification`

诚实的边界（`docs/ARCHITECTURE.md` 里写得不错，可直接引用）：
**tamper-evident（可发现篡改），不是 tamper-proof（不可篡改）**；
`chain_verified` 只证明存储数据自身完整，不证明外部工具输出为真。

### 2.8 多租户隔离设计（设计对，默认配置下未生效）

设计意图是三重防御：

```
1. 应用层：JWT 签名验证 → claims.uid → TenantId       （无法通过 header 伪造 ✅）
2. 事务层：begin_tenant_tx() 设置事务内 GUC aetheris.tenant_id
3. 数据库层：RLS 策略读 current_setting('aetheris.tenant_id') 并 fail-close（NULL → 0 行）
```

`db/tenant_scope.rs` 用 `set_config(..., is_local=true)`，事务结束自动清除，不会泄漏到连接池 —— 这个设计是对的。

⚠️ **一个容易被忽略的产品事实**：`tenant_id` 取的是 JWT 的 `claims.uid`（`tenant/context.rs:136-147`），
也就是**租户粒度 = 用户粒度，不是组织粒度**。要做"一个企业下多个用户共享记忆"，
当前模型撑不住，需要先引入 org 层级。讲解时要说清楚，避免下游按"企业租户"去设计。

另外 `jwt.disabled=true` 时，`auth_middleware:102-110` 会把所有请求塌缩成同一个 `anonymous` 租户
—— `local.toml` 就是这个配置，本地开发要有意识。

**但第 3 层默认失效**，详见 §5 P0-1。

---

## 3. 对外能力范畴

### 3.1 协议接入面 —— 只有两个真的能被客户端调用

| 协议 | 状态 | 依据 |
|---|---|---|
| **REST / HTTP** | ✅ **可用**（主力） | Axum + JWT，157 个路由 |
| **MCP over HTTP** | ✅ **可达**，但默认被验签卡死 | 见 §3.3 |
| gRPC | 🔴 **不可达** | `grpc_auth_interceptor` **零调用者**；无 tonic server、无 `.proto`、无 `build.rs`。`tonic` 依赖实际只用于 OTLP 上报 |
| WebSocket | 🔴 **不可达** | `ws_upgrade_handler` **从未挂载**；内层 handler 是 TODO，连上立刻回 Close 1008 |
| A2A | 🔴 **不发布** | `#[cfg(feature = "a2a")]`，`default = []`；git 依赖需联网解析。handler 本身是真代码，但默认构建里根本不编译 |

> ⚠️ `project-summary.md` 的"**四传输适配器（REST/gRPC/WS/A2A）全部实现**"这句话**不成立**。
> 实际是 2 个可达 + 2 个类型定义和一个未接线函数 + 1 个 feature 关闭。对外汇报请勿沿用这句话。

### 3.2 REST 能力清单（按域，默认构建全部已注册）

鉴权模型：`protected_api_router` 统一套 JWT；
`/v1/memory`、`/kg`、`/mm` 额外套 governance + 限流(100/60s)；登录路由 10/60s。

| 域 | 数量 | 主要能力 |
|---|---|---|
| 自适应调度 / 决策追踪 | 24 | `adaptive/select`、`adaptive/trace`、`traces`、`explain`、`feedback`、`forget`、analyzer/predictor/monitor/weights 各子接口、`importance/*`、`fusion/*` |
| Agent 身份与自我模型 | 18 | Agent CRUD、self-model、capabilities、episodes、behaviors、`reflect` |
| 记忆检索 | 13 | `search/{stm,ltm,hybrid,triple,entity,scored}`、KG/LTM 的 `at`/`history`/`time-travel` |
| 记忆存储 | 9 | `stm`、`ltm`、`batch-ltm`、`transfer`、`compress/{session,messages}`、Qdrant 租户回填 |
| 记忆池（多 Agent 协作） | 9 | register/share/revoke/visible、correlations、network |
| 快照（Oris 集成） | 7 | create/restore/checkpoint/rollback |
| 知识图谱 | 6 | entities CRUD、by-name、related、relations、search |
| 多模态 | 5 | store、entry/session/modality 查询、list |
| 配置 CRUD | 5 | memory config 增删改查 |
| 可视化 | 5 | timeline / graph / heatmap / dashboard / metrics |
| 租户 | 5 | 列表/注册、`{id}/search`、`{id}/sessions`、`access/check` |
| 分布式协调 | 5 | pool status/allocate/release、signals 发布订阅（**单机进程内**，非集群） |
| MCP | 5 | initialize、tools list/call、resources list/read |
| Billing | 5 | init、usage、quota、record（**注意：没有被 enterprise feature 隔开，默认就在**） |
| Evidence / Workflow | 4 | `workflows/{id}/evidence`、approve/reject、approval status |
| Procedural / GraphRAG | 4 | store/search/graphrag/provider health（**底层是 stub**） |
| 安全探针 | 3 | prompt-probe check / check-input / check-output |
| Planner 沙箱 | 2 | dry-run / reset（**模拟器，非真隔离**） |
| 时间旅行追踪 | 2 | workflow events / dag |
| 健康 | 2 | `/api/v1/memory/health`、`/api/v1/memory/v1/health` |
| 数据导入导出 | 2 | export 可用；⚠️ **import 是占位实现，只计数不写入** |
| 认证 / 用户 | 8 | login、currentUser、users CRUD |
| 根 / 文档 / 静态 | 9 | `/scalar`、`/api-doc/openapi.json`、`/metrics`（**公开**）等 |

**已被移除的能力（返回 404）**：原 `/api/v1/memory/enterprise/cluster/*` 等 **9 个"企业集群"端点**
（内存里假的 Raft + 假分片）在 P0 清理中摘除。`routers/enterprise.rs` 作为 dead code 保留，等 ADR-0006 真实化。
**这个清理动作是对的，值得肯定。**

**声明了但没挂载的 dead router**：`routers/dashboard.rs`、`routers/tenant.rs`、`routers/enterprise.rs`、
`axum_routers/{agent,auth,demo}.rs`、`axum_routers/distributed.rs::router()`。

### 3.3 MCP 工具面

| 项 | 内容 |
|---|---|
| 工具（5） | `memory_write`、`memory_search`、`memory_recall`、`memory_forget`、`memory_list` |
| 资源（4） | `memory://stm/sessions`、`memory://ltm/entries`、`memory://kg/entities`、`memory://mm/entries` |
| 验签 | ✅ **真实且 fail-closed**：HMAC-SHA256 + 常量时间比较，`call_tool` 派发前强制校验 |
| Capability 授权 | 🟡 真实调用、未知工具 deny-by-default，**但 `granted` 对所有调用方硬编码为 `[Read, Write, Delete]`** —— 没有按主体的最小权限 |
| Wasm 沙箱 | 🟠 `mcp/sandbox.rs` 是**真的 wasmtime 实现**（含 fuel 限制、capability 策略），**但零引用、未接入 `call_tool`**。工具实际以原生方式执行 |

⚠️ **默认配置下 MCP 是"卡死"状态**：环境变量 `MCP_TOOL_SIGNATURES` / `MCP_KEY_BUNDLE` 未配置时，
`list_tools` 返回**空列表**，`call_tool` **对每个工具返回 403**。要用必须先由运维签发签名。
安全上这是正确的 fail-closed，但对接同事一定要提前说明，否则会以为坏了。

> 补充：`CLAUDE.md` 说 sandbox 是"mock（原样返回输入）"——**这条已过时**，现在是真 wasmtime。
> 但 CLAUDE.md 的核心判断仍然成立：**没接进调用链**。

### 3.4 SDK（`sdks/`）

| SDK | 状态 | 问题 |
|---|---|---|
| Python `adaptive-memory` | ✅ 最完整、可用 | 同步+异步 client、models、契约测试。**唯一真实路径 bug：调 `api/mcp/initialize`，后端实际是 `/api/initialize`** |
| Python LangChain | 🟡 功能可用 | Tool / Retriever / ChatMessageHistory 都在。⚠️ **`pyproject.toml` 的 `build-backend` 写错（`setuptools.backends._legacy:_Backend`），`pip install` / 打 wheel 会直接失败** |
| Rust SDK | 🔴 **不自洽** | `lib.rs` 只编译 `client` + `models`；`memory.rs`/`knowledge_graph.rs`/`mcp.rs`/`agent.rs` 引用不存在的 `crate::{Config,Result,Error}`，是**孤儿文件**。**README 文档的是这套死 API，照 README 写的示例编译不过** |

SDK 均未覆盖：MM、agents、fusion、tenants、data-io。

### 3.5 前端

Umi 4 + Ant Design Pro，13 个页面（9 个功能页 + Home/Documentation/login/404），路由都注册了。
功能页覆盖：Dashboard、Performance、ResourceMonitor、MemoryManagement、MemoryDetails、
MemoryConfig、TaskAnalysis、WeightHistory、MemoryDecisionTrace。

⚠️ **一个必须知道的坑**：`src/services/memory/auth.ts` **4 个认证接口路径全错**：

| 前端调用 | 后端实际 |
|---|---|
| `POST /api/v1/auth/login` | `POST /api/login` |
| `GET /api/v1/user/me` | `GET /api/currentUser` |
| `POST /api/v1/auth/logout` | 不存在 |
| `GET /api/v1/auth/captcha` | 不存在 |

**`npm start` 下看不出来**，因为 Umi dev mock 拦截了。**一接真后端登录就 404。**

**后端有能力但前端无界面**：agents、workflows/approvals、tracing、distributed、planner、
prompt-probe、tenants、data-io、MCP、`search/triple|scored`、`storage/compress/*`、billing。

### 3.6 ⚠️ 文档漂移警告（讲解时务必提醒）

`CLAUDE.md` 里记录了**不存在的端点**，照着写会踩空：

| CLAUDE.md 声称 | 实际 |
|---|---|
| `GET /api/v1/health`、`/health/liveness`、`/health/readiness` | **都不存在**。真实健康检查只有 `/api/v1/memory/health`（`SELECT 1` 探针）、`/api/v1/memory/v1/health`（假数据）、`/metrics`。**根路径 `/health` 也不存在** —— `mod.rs:119` 的 `.route("/health")` 在 memory router 内部，nest 后是 `/api/v1/memory/health` |
| `GET /api/v1/tenant/context`、`/tenant/quota`、`POST /tenant/quota/reset` | **代码里零匹配**。真实租户面是 `/api/tenants/*` |
| `GET /api/v1/distributed/epoch/status` | 只存在于**未挂载**的 `axum_routers/distributed.rs`，**未对外服务** |

另外 `GET /api-doc/openapi.json` 是**手写**的，只覆盖 MVP 子集
（adaptive/storage/search/kg/mm/mcp），**不含** agents、workflows、distributed、planner、
tenants、security、billing、snapshot、memory-pool、tracing、data-io。
**Scalar 页面不是权威接口清单**，权威是 `src/routers/mod.rs`。

---

## 4. 已完成、可以信赖的代码

图例：✅ 生产可用 ｜ 🟡 能用但启发式 ｜ 🟠 有代码未接线 ｜ 🔴 不可达/mock

### ✅ 可以放心讲、放心用

| 能力 | 位置 |
|---|---|
| **LTM transactional outbox 写路径** | `services/memory_storage.rs:141` + `db/vector_outbox.rs` + `services/outbox_worker.rs` |
| **Evidence Graph 哈希链 + 验签 + 确定性导出** | `services/evidence_graph.rs:27,74` |
| **四层记忆 CRUD + 租户作用域事务** | `db/{stm,ltm,kg,mm}.rs`、`db/tenant_scope.rs` |
| **双时间（bi-temporal）查询 / time-travel** | `routers/memory_search.rs` |
| **STM→LTM 自动晋升守护** | `services/memory_transfer.rs`（真实守护，按消息数/时长/比例触发） |
| **上下文压缩（4 策略，真调 LLM）** | `services/context_compressor.rs`（含截断兜底） |
| **MCP 验签**（HMAC-SHA256、常量时间比较、fail-closed） | `mcp/signing.rs` + `routers/mcp.rs:254` |
| **JWT 安全基线** | HS256、显式过期校验、拒绝 query-string token、httpOnly cookie；**弱密钥启动即失败**（`config/mod.rs:172`） |
| **Prometheus 指标**（outbox pending / dead-letter / 配额比率 / 请求时延） | `services/prometheus_exporter.rs` |
| **docker-compose 完整编排** | PG(pgvector)/Qdrant/Neo4j/Redis/OTel→Jaeger/Prometheus/Grafana，全带 healthcheck |
| **P0 诚实化清理** | 摘掉 9 个假集群端点、停用误导性的 self-optimizing 日志 |

### 🟡 能用，但要知道是启发式

四层检索融合（线性加权 + `LIKE` 关键词）、analyzer / predictor / scheduler / weight_adjuster、
`importance_evaluator`（宣称 LLM-as-Judge，实际是算术加权）、`confidence_scorer`、`memory_fusion`
（relevance 是占位常量 1.0/0.5/0.8，**不是向量相似度**）、`conflict_detector`（作用于执行计划而非记忆）。

### 🟠 有完整代码但没接线（改造成本低、收益高，适合作为新同事的第一批任务）

`services/vector_reconciliation.rs`（对账）、`services/hybrid_search.rs`（RRF）、
`mcp/sandbox.rs`（wasmtime 沙箱）、`services/config_recommendation.rs`（配置推荐，零调用者）、
`services/{adaptive_telemetry,feature_pipeline,eval_harness,strategy_mutator}`、
`services/{consolidation,weight_decay,bitemporal_kg}`、整个 `kernel/`+`layers/`、Neo4j。

### 🔴 不可达 / mock（对外不要计入能力）

gRPC、WebSocket、A2A、`self_healing.rs`（`check_health` 返回硬编码 `healthy:true` 和 1–4ms 假时延，
**且这个假数据被 `/api/v1/memory/v1/health` 端点对外提供**）、
`runtime/planner_sandbox.rs`（`execute_step` 从不真正执行工具，返回 `generate_mock_response` 硬编码 JSON；
所谓网络/文件"隔离"只是一个 bool 标志，**不是 OS/WASM 级隔离**）、
`POST /api/data/import`（占位）、`routers/procedural.rs` 的 GraphRAG（跑在空 stub 上）。

---

## 5. 需要调整迭代的部分（按优先级）

### P0 —— 上生产前必须修（安全红线）

**P0-1. 多租户 RLS 默认完全失效 🔴 最高优先级**

- 现象：所有配置都以 `memory` 角色连库（`docker-compose.yml:13`、`backend/docker.toml:8`、`config.toml:5`、`local.toml:7`），
  而这是 pgvector/postgres 官方镜像创建的 **SUPERUSER 且是表 owner**。
  **Postgres 的 RLS（即使 FORCE）对超级用户一律绕过 → 策略写得再对也永不触发。**
- 迁移脚本自己就写明了这点：`migrations/20260716000100_rls_ltm.sql:26-30`。
- 讽刺的是**正确的受限角色已经存在**：`docker/initdb/01-create-app-role.sql` 创建了
  `aetheris_app`（NOSUPERUSER NOBYPASSRLS），**但没有任何配置把 `DATABASE_URL` 指向它**。
- **修法**：部署时把 `DATABASE_URL` 切到 `aetheris_app`，然后跑一遍已有的 RLS 渗透测试。
  🔴 **但先看这条（2026-08-10 实测）**：`aetheris_app` **在运行库里并不存在**。
  `docker/initdb/01-create-app-role.sql` 虽然被挂载进 `/docker-entrypoint-initdb.d`，但 Postgres
  **只在数据目录为空的首次初始化时执行 initdb 脚本**——本地卷创建于 07-16，脚本 07-17 才加入，所以从未运行。
  生产库同理永远不是全新的 → **必须把角色创建搬进 sqlx migration**，否则这个安全控制永远不会生效。
  实测证据：`memory` 角色是 `superuser=true, bypassrls=true`；`pg_roles` 里 `aetheris_app` 计数为 0。
  ⚠️ 另外这不是纯配置改动：有**一条**刻意的管理员跨租户扫描路径
  （`db/ltm.rs::list_qdrant_tenant_backfill_entries` —— 走裸池、无租户过滤、故意匹配全部租户），
  它在 RLS 生效后会 fail-close 返回 0 行，导致 Qdrant 回填端点静默变成 no-op。切角色前必须先处理。
  （早先版本称"两条"并把 `db/kg.rs::search_knowledge_by_entity_for_tenant` 也算进来，
  经 2026-08-10 复核**该函数已正确租户作用域**，无需改动。）
- 顺带修正两个数字口径：RLS 实际启用在 **8 张表**（不是文档说的 13 张）；
  `project-summary.md` 把"RLS 迁移遗漏查询路径"标为"已缓解"是**误导** —— 迁移没问题，是连接角色让它失效。

**P0-2. `db/mm.rs` 租户隔离 fail-open 🔴**

`begin_optional_tenant_tx`（`mm.rs:37`）只在 `tenant_id` 为 `Some` 时开租户事务，
**为 `None` 时直接走裸连接池**（11 处），且 `MMRepository::count`（`mm.rs:579`）是
**永远不带租户过滤的全局 `SELECT COUNT(*)`**。即使修完 P0-1，MM 层仍是漏的。

**P0-3. 限流可被 `X-Forwarded-For` 绕过 🔴**

`rate_limit.rs:127-140` 优先用 `ConnectInfo` 的 peer IP，XFF 只作兜底 —— 逻辑是对的，
**但 `ConnectInfo` 从未在 serve 时注入**（`main.rs:165/180` 用的是 `into_make_service()` 和
`axum::serve(listener, app)`，都没有 `into_make_service_with_connect_info::<SocketAddr>()`）。
**结果是永远走 XFF 分支，而 XFF 完全由客户端控制** → 轮换 header 即可每请求一个新桶。
**登录限流用的是同一个中间件 → 撞库保护同样失效。**
副作用：没有 XFF 时所有客户端共用 `"unknown"` 桶，会互相拖累。
另外 `POST /register` 挂在根路由，**既无鉴权也无任何限流**。

**修法**：serve 时改用 `into_make_service_with_connect_info::<SocketAddr>()`，并加可信代理白名单。

**P0-4. 治理（RBAC/配额）没有一种配置能既生效又可用 🔴**

- **配额永不生效**：唯一自增 `used` 的地方是 `enterprise_impl.rs:540` 的 `post_store`，
  而 governance 中间件**只调 pre-hook，从不调 post_hook**（`governance.rs:161-166`）→ `used` 永远是 0。
  且没有任何已挂载端点会给租户**创建**配额，未知租户默认 `allowed: true`。
- **RBAC 代码补上了但不可用**：`pre_store` 确实查了 `"write"` 权限，
  但角色**从未在启动时播种**，唯一的 `assign_role` handler 在**未挂载**的 `routers/tenant.rs:202`
  → `blocking_has_permission` 恒为 `false`。
- **净效果**：
  - 鉴权开 + 治理开 → 所有被治理的 store/search 一律 `403 Insufficient role permissions`，**记忆功能整体卡死**；
  - `jwt.disabled=true` → governance 中间件提前 return，**RBAC/配额/审计全部跳过**（`local.toml` 就是这个配置）。
- **fail-closed 开关确实存在**（`GOVERNANCE_FAIL_CLOSED`）但**默认 fail-open**；
  且 `jwt.disabled` 的短路发生在 fail-closed 判断**之前**，所以它保护不了关闭鉴权的部署。

**P0-5. MCP 写路径完全绕过治理 🔴**

`classify("POST", ".../mcp/tools/call")` 匹配不到任何模式 → 返回 `None` → 中间件跳过
（`governance.rs:128-131`）。于是 `handle_memory_write` / `handle_memory_forget`
**在没有配额、没有 RBAC 的情况下执行**。而 MCP 恰恰是主要的 Agent 写入通道。

### P1 —— 可靠性与可信度

1. **outbox 重放/幂等零行为测试**：`db/vector_outbox.rs` 只测了 key 字符串格式；
   `ON CONFLICT DO NOTHING`、`claim_batch`、重试退避、`reclaim_stale` **在任何层都没有测试**。
   这是"最扎实的一块代码"却没有回归保护，风险不对称。
2. **接上对账扫描器**：`vector_reconciliation` 已实现，加个守护进程或运维端点即可，收益立竿见影。
3. **审计不可靠**：fire-and-forget，队列满/通道关/未初始化都**静默丢弃 + 计数**，无重试无背压；
   且只在 Postgres 下启动 → **SQLite 下审计事件全丢**。当前不能作为合规日志，只能在 PG 上并监控 `dropped_count()`。
4. **补齐 liveness / readiness**：CLAUDE.md 和 README 都写了这两个探针，**实际不存在**；
   `/v1/health` 返回的还是 `self_healing` 的**假数据**。readiness 也不检查 Qdrant/Neo4j/Redis 连通性。
   compose 的 healthcheck 打的是 `/`（裸根）—— 而根路径上**并没有** `/health`，唯一的真实探针是 `/api/v1/memory/health`。
5. **告警与看板缺失**：指标质量其实不错，但 `monitoring/` 里**没有任何 alert rules**，
   Grafana 只配了 datasource、**没有 dashboard**。outbox 堆积、死信增长、鉴权失败都无人告警。
6. **CI 覆盖面**：CI 只起了 Postgres —— outbox→Qdrant、KG、Redis-STM、embedding/LLM 全部**未被自动化验证**。
   `clippy` 和 `cargo audit` 都是 `continue-on-error: true`（**不阻塞**）。
   `SQLX_OFFLINE=true` 依赖签入的 `.sqlx/` 缓存，缓存过期会削弱编译期 SQL 校验。
   ~~`ci.yml` 和 `backend-ci.yml` 两个 workflow 高度重复，会漂移。~~ **已修（2026-08-10）**：
   `backend-ci.yml` 已删除，`ci.yml` 成为唯一 workflow 并新增 Qdrant service，clippy 与 cargo audit 改为阻塞。
7. **测试"假绿"要治理**：6 个集成测试（`rls_isolation_pg`、`rls_kg_pg`、`rls_mm_pg`、`rls_stm_pg`、
   `tenant_scope_pg`、`memory_platform_e2e`）在 `DATABASE_URL`/`AMS_E2E` 未设置时**提前 return 并报 ok**
   —— 它们是仓库里最有价值的安全测试，本地却贡献 0 证据。建议改 `#[ignore]` 或显式 fail。
   另外 `tenant_isolation.rs` 那 8 个"永远通过"的测试**只验证 `TenantId::prefix()` 字符串格式，不是数据库隔离**，极易误读。
   附真实数字：本地无 DB 时 `cargo test` = **286 passed / 3 failed**（3 个是
   `tenant_scope.rs:71` 的 `.expect()` 硬 panic，不是优雅跳过）；集成测试 129 "passed"/24 binaries，其中 6 个假绿。
   `a2a_integration.rs`（7 个测试）**根本不编译**。

### P2 —— 协议与集成

1. **gRPC / WebSocket 二选一或都砍掉**：现在是"有类型定义、有一个未接线函数"的中间态，
   维护成本 > 价值。要做就补 `.proto` + tonic server / 挂载 WS 路由；不做就删掉并同步文档。
2. **统一 Authenticator 只有 REST 真正收敛**；`web/jwt.rs`（**双 JWT** 技术债）仍然存在且仍被编译
   （已标 DEPRECATED，接受 query-string token、不注入租户上下文，无调用点）。`project-summary.md` 把它当已解决是不准的。
3. **MCP capability 做成按主体最小权限**（现在对所有人硬编码全权限）。
4. **把 wasmtime 沙箱接进 `call_tool`**（不可信扩展平面），按 ADR-0004 落地。
5. **SDK 三个具体 bug**：Python/Rust 的 `api/mcp/initialize` → `/api/initialize`；
   LangChain 的 `build-backend` 写错；Rust SDK 的孤儿模块 + README 文档死 API。
6. **前端 `auth.ts` 4 个路径修正**，并考虑关掉 mock 跑一次真后端联调。
7. **SQLite 模式没有任何 DB 层隔离**，而 `db.url` 为空时会**静默降级到 SQLite**
   （`config/mod.rs:129-138`）。建议生产配置显式禁止降级。

### P3 —— 自适应智能（想清楚再做）

按 `ADR-0008` 的思路是对的：**先离线批量拟合可解释模型，用 eval harness 证明显著优于静态最优配置，
证不出来就诚实降级为"启发式配置选择"**。当前状态相当于"降级版"已经是事实，只是文档还没完全对齐。

要往前走，最小闭环是：
`adaptive_telemetry` 真实落库 → `feature_pipeline` 产出特征/标签 → 离线拟合 →
`predictor::fit_from_samples` **真正实现**（现在只有声明）→ `eval_harness` 真调 scheduler 做 A/B →
胜出才让 scheduler 消费学习到的参数。

另外 `config_recommendation.rs` 有个**必须纠正的虚假声明**：
文件头写着"基于规则 + 历史数据 / training_samples"，但代码**只查 `ConfigArchetypeRepository::list_active()`，
从未查询 `training_samples`**，而且整个服务**零调用者**。要么接上并真的用历史数据，要么改掉这句注释。

### 附：文档与治理债

- `IMPLEMENTATION_STATUS.md`（7-17）已被 8-03 的改动超越，但 8-03 的总结**反向高估**。两份都需要按本文校正。
- `docs/ROADMAP.md` 把 v0.8"分布式集群（node/replication/sharding/consensus）"标为 Implemented ——
  实际是**进程内单机协调原语**，HA 按 ADR-0005 委托给托管基建。这个表述必须改，否则是尽调雷点。
- 8 份 ADR **全部是 `Proposed` 状态**，没有一份转 `Accepted`，但代码已按其中若干份实现了。需要补收口。
- **open-core 边界只存在于文档**：`feature = "enterprise"` 在 `src/` 里**出现 0 次**，
  gates 不了任何代码；billing 等所谓 Enterprise 模块默认就编译进 MIT 二进制。
  商业化前必须真正做 feature gating。
- 弱默认口令：Neo4j `neo4j/password`、Grafana `admin/admin` —— 开发可接受，**不能带上生产**。

---

## 6. 给新同事的上手建议

**读代码顺序**（别一头扎进 `kernel/`）：

1. `src/main.rs` —— 看清启动时到底拉起了什么
2. `src/routers/mod.rs` —— **唯一权威的对外能力清单**
3. `services/memory_storage.rs:141` + `services/outbox_worker.rs` + `db/vector_outbox.rs` —— 学写路径范式
4. `services/memory_search.rs:300,761` —— 看检索融合的真实算法
5. `db/tenant_scope.rs` + 一个 `db/ltm.rs` —— 理解租户事务约定
6. `services/evidence_graph.rs:27,74` —— 审计哈希链
7. 最后再看 `kernel/`、`layers/` —— **知道它是未接线的抽象**

**三条铁律**：

1. 碰到任何 RLS 表的查询，**必须** 走 `begin_tenant_tx()`，不要拿裸 pool 查。参考 `db/ltm.rs`，**不要**参考 `db/mm.rs`。
2. 新增对外接口后，同步更新 `routers/mod.rs` 注册、手写 OpenAPI、以及本文的能力清单。
3. **不要相信仓库里的状态文档**。判断一个能力是否真的存在，标准是：
   **路由是否注册 + 函数是否有调用者 + feature 是否默认开启**。三者缺一即"不可达"。

## 7. 本文结论的复核方式

想自己验证任何一条，用这几个命令：

```bash
# 某个函数到底有没有调用者（判断死代码的关键）
codegraph explore "函数名"

# feature 是否默认开启
sed -n '/\[features\]/,/^\[/p' backend/Cargo.toml

# 某能力是否真的注册成路由
grep -n "路径片段" backend/src/routers/mod.rs

# 连库角色是不是超级用户（P0-1 的根因）
grep -rn "postgres://" backend/*.toml docker-compose.yml

# 本地真实测试数字
cd backend && cargo test 2>&1 | tail -30
```

---

**维护约定**：本文是"实现真相"文档。每次合入改变对外能力或成熟度的 PR，请同步更新对应小节；
若与 `IMPLEMENTATION_STATUS.md` 冲突，**以本文为准**并回头修正那份。
