# ADR-0007: 多协议传输真实化与跨协议统一鉴权

## 决策信息

- 编号：ADR-0007（Proposed；由 tech-lead 于 Design Review Board 收口后转 Accepted）
- 决策标题：gRPC / WebSocket / A2A 三协议真实化的传输选型（tonic / axum WS upgrade / a2a-rs），并把「token → JwtClaims → RequestTenantContext」抽取为**传输无关的鉴权核心**，四协议（含现有 REST/MCP）通过各自适配器统一收敛到同一 JWT + 租户语义
- 状态：Proposed
- 日期：2026-07-16
- Owner：architect
- 关联需求 / 命令入口：`docs/artifacts/2026-07-16-enterprise-productionization/delivery-plan.md`（P2「多协议接真实」：A2A 接真实记忆服务 + gRPC 真 server + WebSocket 真 server + 鉴权 + agent-card 一致性）
- 关联 ADR：
  - `ADR-0001-memory-storage-tenant-isolation.md`（租户隔离 / RLS——四协议隔离的最终兜底）
  - `ADR-0004-mcp-sandbox-execution-model.md`（MCP 工具执行 / 验签 / 沙箱——与本 ADR 的鉴权平面边界）
  - `ADR-0005-ha-infrastructure-selection.md`（不自建基础设施——约束「不引网关 / 网格作 P2 前置」）
  - `ADR-0006-enterprise-cluster-coordination.md`（许可分级 / 集群路由——跨协议许可门控挂载点）

## 结论先行

1. **现状是「一真三壳、且三壳无鉴权无租户」**：REST 是唯一真实面；MCP-over-HTTP 真实且**已复用** `hoops::jwt::auth_middleware` + `RequestTenantContext`（`routers/mcp.rs:136-151` 把 `/mcp/*` 挂在 auth 之后，`call_tool` 读 `Extension<RequestTenantContext>`）。而 **gRPC**（`protocol/grpc.rs:1-7` 自述 "NOT IMPLEMENTED (pending P2) … no tonic server … zero consumers"）、**WebSocket**（`protocol/websocket.rs:315-333` 的 `send_to_session` 计算了订阅却忽略、注释 "In real implementation, would send via WebSocket"）是纯类型壳 / 占位，**A2A**（`a2a/handler.rs` 所有 handler 返回假数据、feature-gated 关闭）是假数据壳。三者**均无握手鉴权、无租户上下文**。
2. **传输选型**：gRPC → **tonic**（`Cargo.toml:73` 已在依赖树，otel 传递引入，离线可用）；WebSocket → **axum `WebSocketUpgrade`**（框架内置，零新依赖，同端口 / 同 TLS / 同中间件栈）；A2A → **a2a-rs**（`Cargo.toml:22` `a2a` feature + `:79-84` 注释的 git 依赖，需联网 pin rev 后 `--features a2a`）。
3. **跨协议鉴权（核心决策）**：把 `hoops/jwt.rs::auth_middleware` 里「提取 token → HS256 解码 `JwtClaims{uid,exp}` → 构造 `RequestTenantContext{tenant_id,user_id}`」抽出为**传输无关的 `Authenticator` 核心**。REST/MCP（现状）、gRPC（tonic interceptor 取 metadata token）、WebSocket（握手期鉴权 + 连接期绑定租户）、A2A（HTTP middleware + agent 身份映射）**四个适配器全部调用同一核心**——保证四协议的租户隔离（ADR-0001 RLS）与许可门控（ADR-0006）**口径完全一致**。
4. **顺带收口双 JWT 实现漂移**：现有 `hoops/jwt.rs`（权威：拒绝 query-string token、注入租户上下文）与 `web/jwt.rs`（弱实现：`JwtClaims{sub,exp,iat}`、**接受 query-string token**、**不注入租户上下文**，`web/jwt.rs:14-18,57-63`）必须合并到 Authenticator 核心，消除鉴权漂移。
5. **边界清晰**：ADR-0007 只定义「传输 + 鉴权平面」。工具执行的验签 / 沙箱 / capability 归 ADR-0004；许可分级 / 集群分片算法归 ADR-0006；数据存储 HA 归 ADR-0005。遵循「不自建基础设施」：gRPC 走**同进程独立端口**，WebSocket / A2A **复用 axum HTTP 端口**，不引入 API 网关 / 服务网格作为 P2 前置。
6. **多周工作流、需真环境**：真正端到端（真 PG + 租户隔离 / 多实例 / a2a-rs 联网 pin）需落码与集成验证，**离线会话内只能出设计（本 ADR）**。

## 背景与约束

### 当前状态（已读代码核实）

- **权威鉴权 = `hoops/jwt.rs::auth_middleware`（`:58-94`）**：从 `jwt_token` httpOnly cookie 或 `Authorization: Bearer` 提取 token（**显式拒绝 query-string token**，防日志 / referrer 泄漏）；HS256 + `config::get().jwt.secret` 解码 `JwtClaims{uid,exp}`；注入 `JwtClaims` **和** `RequestTenantContext` 到 request extensions；`jwt.disabled` 时降级为 `anonymous`（仅 Docker/dev，生产 fail-fast 已由 P0.3 保证）。
- **`RequestTenantContext{tenant_id,user_id}`（`tenant/context.rs:110-147`）**：MVP 下 `tenant_id == uid`；同时实现 `FromRequestParts`（从 extensions 取 `hoops::jwt::JwtClaims`）。全仓 **39 个 caller**，是所有 handler 做数据隔离的统一入口。
- **REST（`routers/mod.rs:55`）**：`auth_layer = middleware::from_fn(hoops::jwt::auth_middleware)`，通过 `.route_layer(auth_layer)` 对受保护路由统一加鉴权。
- **MCP-over-HTTP（`routers/mcp.rs:136-151`）**：`/initialize` 公开；`/mcp/tools|tools/call|resources|resources/read` 在 `auth_middleware` 之后；`call_tool`（`:225-243`）读 `Extension<RequestTenantContext>` 后按工具名分派——**已是「HTTP 鉴权平面」的参考实现**（其执行 / 验签模型见 ADR-0004）。
- **gRPC（`protocol/grpc.rs:1-7`）**：仅手写镜像 protobuf 的 Rust struct，头注释自述未实现、无 tonic server、零消费者，`#![allow(dead_code)]`。`tonic 0.12`（`Cargo.toml:73`）+ `prost 0.13`（`:74`）已在依赖（当前仅 `opentelemetry-otlp` 的 `grpc-tonic` 传递使用）——**采用 tonic 不引入需联网解析的新顶层依赖**。
- **WebSocket（`protocol/websocket.rs`）**：`WsConnectionManager` 用内存 `HashMap` + `broadcast::Sender<EventResponse>` + session 计数；`send_to_session`（`:315-333`）**占位**（算出 `is_subscribed` 却丢弃、恒 `true`）。**无 axum WS 升级 handler、无路由、无握手鉴权**；`WsConnection.user_id: Option<String>`（`:203-208`）无租户概念。
- **A2A（`a2a/`，feature-gated OFF）**：`handler.rs` 按关键词探测 skill，但所有 handler 返回**假数据**（`handle_memory_search` 只 `format!` 字符串不查记忆；`handle_memory_status` 硬编码 `stm_count:0 … overall_healthy:true`）；`streaming.rs` 有真 SSE 管线（`/a2a/rest/messages/stream`）但调假 handler；**无租户、无鉴权**。`Cargo.toml:22` `a2a=[]` 裸 flag，git 依赖（`a2a-lf` / `a2a-server-lf` @ `a2aproject/a2a-rs`，`:79-84`）因「optional 也会在依赖解析阶段被拉取、破坏离线 / CI」被注释；`routers/mod.rs:434-440` 里 `#[cfg(feature="a2a")]` 才 merge `a2a_router`。
- **双 JWT 实现漂移**：`web/jwt.rs` 另有一套 `JwtClaims{sub,exp,iat}`、**接受 query-string token**、`auth_middleware` 只校验存在性、**不注入租户上下文**——与 `hoops/jwt.rs` 冲突且更弱，是安全隐患。

### 约束

- **企业级可售卖**：四协议必须**一致受鉴权保护、一致租户隔离**（delivery-plan P2 放行标准：「三协议端到端产生真实记忆操作（非假数据）」「均受鉴权保护」「agent-card 能力声明与实测一致」）。
- **离线 / 跨平台**：gRPC 不得引入需联网解析的新顶层依赖（tonic 已在）；a2a-rs 需**联网 pin 一次** + 提交 lock。
- **不推倒现有资产**：复用 `hoops/jwt.rs`、`RequestTenantContext`、ADR-0001 租户查询、ADR-0006 governance hooks。
- **不自建基础设施（ADR-0005）**：不引入 API 网关 / 服务网格作为 P2 前置；协议服务同进程内起。

### 非目标

- 不定义 MCP 工具的验签 / 沙箱 / capability（ADR-0004）。
- 不定义许可分级规则与集群分片算法（ADR-0006），仅定义其**跨协议挂载点**。
- 不定义数据存储 HA（ADR-0005）。
- 不引入 mTLS / SPIFFE 服务身份、不做 OAuth2 / OIDC 联邦（现为对称 HS256 自签，二者均列**演进位**）。

## 备选方案

### A. 传输选型

**gRPC**

| 方案 | 优点 | 风险 / 成本 | 取舍 |
|------|------|-------------|------|
| ① tonic + tonic-build（`.proto` codegen）| 已在依赖树、离线可用；生态标准；与 prost 一致；interceptor 天然支持鉴权；stream 支持 | 引入 `build.rs` codegen + `.proto` 维护；需单独 HTTP/2 端口 | **采用** |
| ② 延续手写 struct + 手搓编解码 | 无 codegen 步骤 | 无 streaming、非标准、极易与真 proto 漂移；仍要手接 tonic server | 不选 |
| ③ grpc-web / grpc-gateway | 浏览器 / REST 直连 | 额外代理层；当前无编排底座 | 不选（留演进）|

**WebSocket**

| 方案 | 优点 | 风险 / 成本 | 取舍 |
|------|------|-------------|------|
| ① axum `WebSocketUpgrade` | 零新依赖；同端口 / 同 TLS / 同中间件；握手是标准 HTTP → **可直接复用现有鉴权** | 需管理连接生命周期 / 心跳 / 背压 | **采用** |
| ② tokio-tungstenite 独立 server | 更底层灵活 | 独立端口 + 独立鉴权栈 + 重复 TLS，违背「统一鉴权」目标 | 不选 |
| ③ 仅 SSE（复用 a2a streaming 模式）| 最简单、已有先例 | 单向、无订阅 / 双向推送 | 部分只读推送场景可用，非 WS 替代 |

**A2A**

| 方案 | 优点 | 风险 / 成本 | 取舍 |
|------|------|-------------|------|
| ① a2a-rs（`a2a-lf` / `a2a-server-lf`）feature 开启 + pin rev | 官方 Rust A2A 类型 / server；handler 骨架已在树 | 需联网 pin 一次 + 许可证核对 + vendor 评估 | **采用** |
| ② 自实现 A2A JSON-RPC | 无外部 git 依赖 | 重复造轮子、协议漂移、维护成本 | 不选 |

### B. 跨协议鉴权（核心）

| 方案 | 适用条件 | 优点 | 风险 / 成本 | 取舍 |
|------|----------|------|-------------|------|
| ① 每传输各自鉴权（现状 WS/A2A = 无）| — | 局部改动 | 鉴权漂移、租户隔离不一致、安全空洞（**正是现状**）| 不选 |
| ② **传输无关 Authenticator 核心 + 各传输适配器** | 多协议共享同一 JWT + 租户语义 | 单一事实源；四协议隔离 / 许可口径一致；可单测；消除双 JWT 漂移 | 需重构现有中间件；需定义 `AuthError → 各传输错误码` 映射 | **采用** |
| ③ 外部 API 网关 / 服务网格（ext_authz）| 规模化、多服务统一入口 | 鉴权卸载到基础设施 | 需编排底座（当前无，违反 ADR-0005 的 P2 不自建前置）| 留演进位；token 语义保持网关兼容 |
| ④ 每传输 mTLS / SPIFFE 服务身份 | 服务间零信任 | 强身份、双向认证 | 证书体系 + 运维重 | 留演进位（尤其 gRPC / A2A agent 身份）|

## 决策结果

### 采用：传输无关 Authenticator 核心 + 四传输适配器；gRPC = tonic、WebSocket = axum upgrade、A2A = a2a-rs

**0. 传输无关鉴权核心（所有协议的单一事实源）**

- 抽取 `authenticate(raw_token: &str) -> Result<RequestTenantContext, AuthError>`：HS256 校验（`config.jwt.secret`）+ 解码 `JwtClaims{uid,exp}` + `exp` 校验 + 构造 `RequestTenantContext{tenant_id,user_id}`；`jwt.disabled` → `anonymous`（仅 dev/Docker，生产 fail-fast 已由 P0.3 保证）。
- 该核心是**跨协议许可门控挂载点**：在此解析 `LicenseTier` 并调用 ADR-0006 governance hooks（`check_license` / `check_quota` / `record_audit`），把许可 / 配额 / 审计从「仅 REST/HTTP」提升为**全协议一致生效**。
- `hoops/jwt.rs` 中间件与 `web/jwt.rs` 合并为**薄适配器**，均委托此核心；**retire** `web/jwt.rs` 的 query-string token 与无租户路径。

**1. REST / MCP-over-HTTP（现状，重构为委托核心）**

- 保持 axum `route_layer(auth_middleware)`；中间件内部改调 `authenticate` 核心；`call_tool` 继续读 `Extension<RequestTenantContext>`。**行为不回归**，MCP 继续作为 HTTP 鉴权平面参考实现。

**2. gRPC（tonic 真 server）**

- `proto/*.proto` + `tonic-build`（`build.rs`）生成 server；service impl **委托 REST 同款 memory 服务 / repository**（非另写业务逻辑），把 `protocol/grpc.rs` 的手写 struct 壳替换为生成类型。
- **鉴权 = tonic Interceptor / tower layer**：从 request metadata 取 `authorization: Bearer <jwt>` → `authenticate` 核心 → 把 `RequestTenantContext` 注入 tonic `Request` extensions；各 RPC handler 从 extensions 读取，**租户隔离与 REST 完全一致**。metadata 缺失 / 校验失败 → `Status::unauthenticated`。
- **部署**：gRPC 走**同进程独立 HTTP/2 端口**（不与 axum 在单端口做 h2c content-type 多路复用，降复杂度与出错面）；TLS 复用现有 rustls 配置（`axum-server` 已用 `tls-rustls`）。

**3. WebSocket（axum `WebSocketUpgrade`）**

- 路由 = axum GET 升级端点，**握手请求先过同一 `auth_middleware`**（cookie / Bearer，拒 query-string token）→ 得 `RequestTenantContext`。
- **连接期绑定**：把 `RequestTenantContext` 存入 `WsConnection`（替换现 `user_id: Option<String>`）；连接生命周期内所有帧都以该租户为准；订阅 / 广播 / 推送**按 tenant 过滤**，`send_to_session` **落真**（按订阅真正投递，终结 `:327-329` 的恒 `true` 占位），杜绝跨租户串号（参照 F3 signaling 修复精神）。
- **长连接 token 过期**：握手 `exp` 校验之外，加**连接最大 TTL + 到期要求重连**（或周期性 re-validate）；token 撤销 / 过期不得因连接常驻而绕过鉴权。

**4. A2A（a2a-rs，握手层复用 HTTP 鉴权 + agent 身份）**

- 启用 `--features a2a`（联网 pin rev + 提交 `Cargo.lock`）；handler 从假数据改为**调真 memory 服务**（search / store / fusion / status / knowledge_graph 各自落真 repository / service），task store 落真，streaming 走真事件（`streaming.rs` 管线保留，接真 handler）。
- **鉴权**：A2A 是 JSON-RPC / REST over HTTP → message-send / stream 端点复用**同一 axum `auth_middleware`** → `RequestTenantContext`。
- **agent 身份**：把 A2A 调用方 agent 映射到 tenant / user 模型（caller agent → `user_id` / `tenant_id`），与 `RequestTenantContext` 对齐；**agent-card 能力声明须与实测一致**（delivery-plan P2 放行标准），不再广告未实现 skill / streaming。

### 理由

- **单一事实源**：一个 `authenticate` 核心 → 四协议隔离 / 许可口径一致，直接闭合 WS / A2A 当前「无鉴权无租户」空洞与双 JWT 漂移。
- **复用而非重写**：gRPC / WS / A2A 的 service 逻辑全部委托 REST 同款 memory 服务与 ADR-0001 租户查询；tonic 已在依赖、axum WS 零新依赖。
- **符合「不自建基础设施」**：不引网关 / 网格作前置；gRPC 单端口同进程、WS / A2A 复用 axum。
- **诚实补真**：三协议从「壳 / 假数据」变为「端到端真实记忆操作且受鉴权」，对齐 delivery-plan P2 放行标准，而非以壳 / 假数据冒充能力。

### 影响范围

- 鉴权核心：`hoops/jwt.rs`（抽 `authenticate`）、`web/jwt.rs`（retire / 合并）、`tenant/context.rs`（`RequestTenantContext` 复用，不改结构）。
- gRPC：新增 `proto/*.proto` + `build.rs`（tonic-build）；`protocol/grpc.rs`（壳 → 真 service + interceptor）；`main.rs`（起 gRPC 端口 + 优雅停机）。
- WebSocket：`protocol/websocket.rs`（`WsConnection` 绑 `RequestTenantContext`、`send_to_session` 落真）；新增 axum WS 升级路由 + 握手鉴权；`routers/mod.rs`（挂载）。
- A2A：`Cargo.toml`（uncomment + pin a2a 依赖、`a2a` feature 改 `["dep:a2a","dep:a2a-server"]`）；`a2a/handler.rs`（假 → 真）；`a2a/router.rs` / `streaming.rs`（接真服务 + 鉴权 + agent 身份）；`routers/mod.rs`（`#[cfg]` 合入）。
- 治理：四协议接 ADR-0006 governance hooks（license / quota / audit）。
- 文档：`CLAUDE.md` / API 文档去除「gRPC/WS/A2A 已就绪」未兑现表述；agent-card 对齐实测。
- 测试：跨协议契约测试 + 四协议鉴权负向（缺 token / 过期 / 跨租户）测试。

### 兼容性 / 迁移影响

- REST / MCP **行为不变**（仅内部委托核心）。
- gRPC / WS / A2A 是**新增面**，无既有客户端；A2A feature 默认关，开启需联网 pin，**不影响离线默认构建**。
- `web/jwt.rs` 的 query-string token 支持将移除（`hoops` 侧此前已拒 query token）；若有 web 客户端依赖，需迁到 Bearer / cookie——属**行为收紧**，迁移期可先「告警不拒绝」灰度一版。
- 单端口 → 多端口（gRPC）：部署需开放新端口 + TLS 证书覆盖；`deployment-context` / 文档同步。

### 失败或回退思路

- **tonic codegen / 端口受阻**：回退到仅 REST + MCP + WS + A2A（HTTP 系），gRPC 延后；**不得**以手写 struct 壳冒充「gRPC 已就绪」。
- **a2a-rs 离线 pin 不可得**：A2A feature 保持关闭 + 文档如实标注「未启用」；**不得**以假数据冒充（现状根因）。
- **WS 长连接鉴权复杂度超预期**：回退到「短 TTL + 频繁重连」保守策略；**不得**放宽为「握手后永不校验」。
- **统一核心重构引入回归**：**分两步**——先抽核心让 REST/MCP 委托（不改行为）并加测试，再逐协议接入；任一步失败可停在上一步，不阻塞其他协议。

## 企业内控补充

- **应用等级**：多协议对外暴露 + 多租户数据，按 T2/T3 高风险内控口径；四协议的鉴权失败 / 跨租户拒绝 / 许可越限须纳入审计与告警准入。
- **技术架构等级**：gRPC / WS / A2A 的端点、端口、TLS、鉴权与租户注入点须纳入资产可视可控（TA 图 / 接口文档 / 部署架构图）；gRPC 新端口须登记。
- **关键组件 / 平台偏离**：`tonic 0.12`（已在依赖）、axum WS（框架内置）、`a2a-rs`（外部 git 依赖，需 pin + 许可证核对 + vendor 评估，偏离集团统一网关 / 协议栈须在此登记）；对称 HS256 自签 JWT 若与集团统一 IAM / OIDC 不一致，须评估对接或登记偏离。
- **资产文档入口**：本 ADR、`delivery-plan.md`、后续多协议契约测试计划、`deployment-context.md`（端口 / TLS / 证书）。

## 后续动作

| 动作 | Owner | 完成条件 |
|------|-------|----------|
| Design Review Board 收口本 ADR（含与 0004 / 0005 / 0006 边界确认）| tech-lead / architect | Proposed → Accepted |
| 抽 `authenticate` 核心 + REST/MCP 委托 + 合并 `web/jwt.rs` | backend | REST/MCP 行为不回归；鉴权核心单测（有效 / 过期 / 无效 / disabled）通过 |
| gRPC：`.proto` + tonic-build + service（委托 memory 服务）+ interceptor 鉴权 | backend | 端到端真实记忆操作；缺 / 错 token 返 `unauthenticated`；跨租户被拒 |
| WebSocket：axum 升级路由 + 握手鉴权 + `WsConnection` 绑租户 + `send_to_session` 落真 + 长连接 TTL | backend | 订阅 / 推送按租户隔离；过期连接被回收；无跨租户串号 |
| A2A：pin deps + feature 开 + handler 接真服务 + 鉴权 + agent 身份映射 + agent-card 对齐 | backend | `--features a2a` 端到端真实且受鉴权；card 与实测一致 |
| 四协议接 governance hooks（license / quota / audit）| backend | 越限 / 越权负向测试通过；审计字段非空 |
| 跨协议契约测试 + 四协议鉴权负向测试 | qa | test-plan 有执行证据，阻塞项清零 |
| 评估 JWT 与集团统一 IAM / OIDC 对接、a2a-rs 许可证 / vendor | architect / devops | 对接或偏离登记结论 |
