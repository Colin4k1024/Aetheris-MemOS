# ADR-0004: MCP 工具沙箱执行模型

## 决策信息

- 编号：ADR-0004（Proposed；由 tech-lead 于 Design Review Board 收口后转 Accepted）
- 决策标题：MCP 工具执行采用「可信第一方平面 + 不可信扩展平面」双平面模型；第一方平面强制 call_tool 验签 + capability 授权 + 审计，不可信平面采用 wasmtime 真沙箱
- 状态：Proposed
- 日期：2026-07-16
- Owner：architect
- 关联需求 / 命令入口：`docs/artifacts/2026-07-16-enterprise-productionization/delivery-plan.md`（P1「MCP wasmtime 真沙箱执行 + capability 强制 + call_tool 验签」）
- 关联 ADR：`docs/adr/ADR-0001-memory-storage-tenant-isolation.md`（租户隔离，call_tool 已复用其 tenant context）

## 结论先行

1. **当前「沙箱」名实不符，且不是单一问题，而是两个被混淆的关注点**：live 执行路径（`routers/mcp.rs::call_tool`）执行的是 5 个第一方记忆工具（需特权 DB 访问），而 `WasmSandbox`/`SandboxProxy` 沙箱机器在全仓**零调用点**、且 `execute_wasm` 是原样返回输入的 mock。CLAUDE.md「MCP tool calls run in isolated sandboxes」当前为**虚假声明**。
2. **推荐采用双平面模型**：
   - **Plane A 可信第一方工具平面（今天在跑的路径）**：以 authn + 租户隔离（已具备）+ **call_tool 强制验签** + **capability 授权** + **结构化审计** 加固。第一方记忆工具**不套 WASM**——它们本就需要特权 DB 访问，套 WASM 只会逼迫开出宽 host function，隔离收益趋近于零。
   - **Plane B 不可信 / 第三方扩展工具平面（未来）**：**必须**用 wasmtime 真 WASM 沙箱（capability-gated host functions、deny-by-default、fuel/epoch CPU 限制、StoreLimits 内存限制、无 ambient authority）。当产品真正开放"外部 / 用户自带 MCP 工具"时按本 ADR 落地。
3. **P1 的 abort 阻塞项 = Plane A 加固**（call_tool 验签 + capability 授权 + 审计 + 移除虚假声明）。它直接闭合 live 路径的越权与名实不符问题，可在 P1 的 2–3 周窗口内完成，并满足放行标准「越权被拒」——针对**今天真实执行的工具**。
4. **需 tech-lead 裁决的范围点（升级门禁）**：delivery-plan 的 P1 放行标准「MCP 工具在沙箱内执行且越权被拒」是否**强制要求完整 Plane B（wasmtime 真执行）进入 P1**？鉴于当前无任何不可信工具，建议：Plane B 的构建取决于"支持不可信 / 自带工具"是否为已承诺的售卖点——是则按本 ADR 作为 P1/P2 工程项落地；否则 P1 只交付 Plane A，并把 mock 沙箱**要么补成 feature-gated 真实现（不对外宣称启用）、要么删除**，杜绝以 mock 冒充能力。

## 背景与约束

### 当前问题（已读代码核实）

- **live 路径未验签、未沙箱**：`backend/src/routers/mcp.rs:225-255` 的 `call_tool` 仅取 `Extension<RequestTenantContext>` 后，按工具名直接分派到原生 `handle_memory_write/search/recall/forget/list`，直连 `MemoryStorageService`/`KGRepository`/`MMRepository`/`STMRepository`/`LTMRepository`。执行前**不验签、不过 capability、不进沙箱**。
- **验签只 gate「广告」不 gate「执行」**：`list_tools`（`routers/mcp.rs:166-214`）与 `list_resources`（`:711+`）会用 `verify_component` 对从环境变量 `MCP_TOOL_SIGNATURES` / `MCP_RESOURCE_SIGNATURES` 载入的签名做校验，未签 / 验签失败的组件不列出。但**列举被验签、调用不被验签**，二者不一致。
- **沙箱是 mock 且无人调用**：`backend/src/mcp/sandbox.rs:135-164` 的 `execute_wasm` 校验完 capability policy 后 `Ok(input)` **原样返回输入**（注释自述 "mock implementation"）；`WasmSandbox` 持有的 `_engine`/`_store`/`_linker`（`:109-113`）全部 `_` 前缀未使用。`sandbox_proxy.rs:89-143` 的 `execute_tool` **忽略传入的 `policy` 参数**、直接调原生 `tool.execute`，且审计里 `capabilities_used` 恒为空（`:101`）。`backend/src/mcp/mod.rs:9-10` 只 `pub use` 再导出这些类型，全仓无任何实际调用点——**整个沙箱子系统在请求链路上是死代码**。
- **签名基元本身是好的**：`backend/src/mcp/signing.rs` 是真实的 HMAC-SHA256 + 常量时间比较 + trusted key bundle（从 env 载入），实现无问题——**只是用错了位置**（gate 了 list，没 gate call）。
- **对外声明失真**：CLAUDE.md 宣称 "MCP tool calls run in isolated sandboxes"，与上述现状不符。
- **wasmtime 已是依赖**：`backend/Cargo.toml:68` `wasmtime = "20"`，采用真 WASM 沙箱不引入新顶层依赖。

### 威胁模型（决策的关键前提）

当前 `call_tool` 执行的工具（`protocol/mcp.rs:239-250`）是 5 个**硬编码第一方记忆工具**，非用户提供代码。必须区分两类威胁，否则会对错误的对象施加错误的防护：

| 平面 | 执行对象 | 威胁 | 正确防护 |
|------|----------|------|----------|
| Plane A | 第一方记忆工具（今天） | 未授权调用、越租户、契约被篡改 / 冒名、无审计 | authn + 租户隔离 + **验签（契约完整性）** + **capability 授权** + 审计 |
| Plane B | 不可信 / 第三方工具（未来） | 任意代码执行、逃逸、资源耗尽、访问宿主 IO / 网络 / 内存 | **真 WASM 沙箱** + capability-gated host fn + 资源限制 + 无 ambient authority |

对 Plane A 套 WASM 是错配：第一方工具需要特权 DB 访问，只能把 DB 变成受控 host function 再暴露给 wasm，等于把特权重新开回去，隔离收益≈0，却付出全部改造成本。

### 约束

- 企业级可售卖定位要求：**若执行不可信工具，必须有真实隔离**（delivery-plan「四块全补真」）。
- 本仓为跨平台开发（darwin / Linux），方案不应绑定单一 OS 内核特性。
- 需兼容现有 signing.rs（HMAC-SHA256 + trusted key bundle）与 ADR-0001 租户上下文，不推倒重来。
- P1 是可靠性 + 安全准入底线，改动需可在 2–3 周窗口内交付可验证结果。

### 非目标

- 不在本 ADR 定义 MCP 传输协议本身（gRPC/WS/A2A 归 P2 协议 ADR）。
- 不定义 runtime 内的 planner sandbox / subagent pool（与 MCP 工具沙箱是不同子系统）。
- 不要求为 Plane A 第一方工具引入 WASM 编译链路。

## 备选方案

| 方案 | 适用条件 | 优点 | 风险 / 成本 | 不选原因 |
|------|----------|------|-------------|----------|
| ① wasmtime WASM 沙箱（真隔离） | 执行不可信 / 第三方 / 用户自带工具；需 capability least-privilege + 资源限制 + 跨平台 | 进程内轻量（无需额外进程 / 容器）；deny-by-default host fn，无 WASI 即无 IO；fuel/epoch 限 CPU、StoreLimits 限内存；跨平台；wasmtime 已是依赖，与 capability 模型天然契合 | 工具须编译为 wasm（第一方 Rust 工具要重构）；宿主-访客数据编解码开销；每个 capability 需实现受控 host fn；需持续跟进 wasmtime 安全更新 | 作为**第一方工具**方案：过度且隔离收益≈0（见威胁模型）。作为**不可信工具**方案：**采用**（Plane B） |
| ② OS 进程隔离 / seccomp | Linux 生产执行原生二进制工具，需内核级 syscall 过滤 | 可运行任意原生代码（现有 Rust 工具无需改 wasm）；seccomp-bpf 内核级强隔离；成熟 | 绑定 Linux（darwin 开发环境无 seccomp，与跨平台开发冲突）；进程 spawn/IPC 开销与生命周期管理复杂；seccomp profile 维护易错；仍需自建 capability→syscall 映射 | 平台绑定 + 运维复杂度高于收益；wasmtime 提供跨平台等价隔离且是现成依赖 |
| ③ 容器隔离 | 强隔离、多租户执行不可信**长时**任务，需 fs/网络 namespace 隔离 | 隔离边界最清晰（namespace/cgroup）；生态成熟；天然资源配额；适合每工具 / 每租户一容器 | 冷启动延迟不适合 per-call；需容器 runtime + 编排底座（本仓当前无）；镜像供应链与漏洞管理成本；与"进程内低延迟工具调用"目标不符 | 对高频低延迟 per-call 执行过重；保留为未来"重量级 / 长时不可信工作负载"的补充隔离层，非默认模型 |
| ④ 仅验签不隔离 | 只执行完全可信第一方工具；威胁限于契约篡改 / 冒名，不含代码作恶 | 改动最小（复用现成 HMAC-SHA256）；零运行时开销；直接闭合 call_tool 未验签 vs list_tools 验签的不一致 | 不提供任何代码级隔离，一旦引入不可信工具即失效；"沙箱"一词名不副实，须从对外表述移除 | 作为**唯一**方案：不满足"执行不可信工具需隔离"的企业定位。作为**Plane A 基线**（与 capability 授权 + 审计组合）：**采用** |

## 决策结果

### 采用方案：双平面 MCP 工具执行模型

**Plane A — 可信第一方工具平面（现 call_tool 路径，P1 落地）**

1. **call_tool 强制验签**：调用前用现有 `verify_component`（signing.rs）校验被调工具的签名契约，确保执行的是"已签名、可信 issuer、未被篡改"的工具定义；未签 / 验签失败一律拒绝。与 list_tools 使用同一 trusted key bundle，消除"列举验签、调用不验签"的不一致。
2. **capability 授权**：为 5 个记忆工具各自声明**最小 capability / scope 集合**（如 memory_write→写，memory_search/list/recall→读，memory_forget→删），在分派前按 RBAC + 租户配额强制校验，越权即拒（对齐 delivery-plan「capability 强制」「越权被拒」）。
3. **结构化审计**：每次调用记录 `execution_id / tenant_id / tool / 授权决策 / 结果`，落审计存储（修复现状 `capabilities_used` 恒空、审计仅打 log 的问题）。
4. **不套 WASM**：第一方工具继续以原生 async Rust 执行，隔离依赖 authn + 租户隔离 + 上述授权，不进 wasmtime。

**Plane B — 不可信 / 第三方扩展工具平面（未来，需 tech-lead 确认是否进 P1）**

1. **wasmtime 真沙箱执行**：把 `execute_wasm` 从 mock 换为真实例化——用 `Linker` 注册**capability-gated host functions**（deny-by-default，未授予的 capability 对应的 host fn 不注册 / 直接拒绝），实例化模块并调用入口，返回真实输出（终结"原样返回输入"）。
2. **资源限制**：fuel metering 或 epoch interruption 限 CPU；`StoreLimits` 限线性内存 / 表 / 实例数；执行超时。防资源耗尽型 DoS。
3. **无 ambient authority**：默认不启用 WASI；仅在授予对应 capability 时，通过受控 host fn 提供**被虚拟化、被审计**的最小能力（如受限 fs 视图 / 白名单网络）。
4. **同样验签 + 审计**：Plane B 工具同样先验签再执行，审计口径与 Plane A 一致。
5. **交付形态**：以 `#[cfg(feature = "wasm-tools")]` 门控；在未真正开放不可信工具前**不对外宣称启用**。

### 理由

- 把防护对齐到真实威胁：对第一方工具用授权 + 验签 + 审计（够且轻），对不可信工具用真沙箱（必要且到位），避免"对第一方套 WASM"的错配和"对不可信只验签"的失守。
- 复用现有资产：signing.rs（真 HMAC）、ADR-0001 租户上下文、wasmtime 已在 Cargo，均不推倒重来。
- 诚实收口"补真"：Plane A 用真授权闭合 live 路径漏洞并移除虚假声明；Plane B 用真沙箱替换 mock——两条路都让安全姿态"变真"，而非以 mock 冒充。

### 影响范围

- `backend/src/routers/mcp.rs`（call_tool 增加验签 + capability 授权 + 审计接线；list 复用同一 key bundle）
- `backend/src/mcp/sandbox.rs`（execute_wasm：mock→真实例化或 feature-gate/删除；补 fuel/epoch/StoreLimits）
- `backend/src/mcp/sandbox_proxy.rs`（execute_tool 真正消费 policy；审计填充真实 capabilities）
- `backend/src/mcp/signing.rs`（复用，可能新增"调用侧验签"入口）
- `backend/src/protocol/mcp.rs`（为工具声明 capability / scope 元数据）
- `CLAUDE.md`（移除 / 修正 "MCP tool calls run in isolated sandboxes"，改为如实描述双平面现状）
- 相关测试：call_tool 验签 / 越权负向测试、（Plane B）沙箱逃逸 / 资源耗尽 / capability 拒绝测试

### 兼容性 / 迁移影响

- 现有 `MCP_TOOL_SIGNATURES` / `MCP_TRUSTED_ISSUERS` env 契约不变；call_tool 验签复用同一签名与 key bundle。
- call_tool 从"任意已认证租户可调任意工具名"收紧为"须验签 + 授权"，属**行为收紧**：迁移期可先"验签失败仅告警不拒绝"灰度一版，再切"拒绝"，避免一次性打断现有 SDK / E2E。
- capability 授权需先补齐工具→scope 映射与租户 RBAC 数据，backfill 完成前不 enforce。
- Plane B 若 feature-gate，默认关闭，不影响现有构建 / 部署。

### 失败或回退思路

- call_tool 验签导致误拒：回退到"验签仅告警 + 授权仍强制"的中间态，但**不得回退到完全不验签**。
- capability 映射不完整误拒合法调用：临时放宽到"tool 存在性 + 租户隔离"最小校验，同时补映射；不回退到无授权。
- Plane B wasmtime 真实现受阻（如工具 wasm 化工作量超预期）：**不得**以 mock 冒充——要么 feature-gate 关闭并从对外表述移除"沙箱"，要么容器隔离（方案③）作为重量级兜底；由 tech-lead 依升级策略裁决。
- 若 tech-lead 判定 P1 不需要 Plane B：删除 mock 沙箱死代码，P1 仅交付 Plane A，`delivery-plan` 相应更新放行标准口径。

## 企业内控补充

- 应用等级：待 tech-lead 按业务 / 底线评定确认；MCP 执行外部工具触及代码执行与多租户数据，无法确认前按 T2/T3 高风险内控口径处理。
- 技术架构等级：工具执行、验签、capability 授权、（Plane B）沙箱运行时均须纳入资产可视可控、审计与告警准入（越权拒绝、验签失败、沙箱逃逸 / 资源耗尽告警）。
- 关键组件 / 平台偏离：wasmtime 20（进程内 WASM runtime，已在依赖）；HMAC-SHA256 signing（自实现，非集团统一签名平台——若集团有统一制品签名 / 密钥管理能力，Plane A/B 的 key bundle 应评估对接，偏离须在此登记）；capability / RBAC 若未复用集团统一权限平台，需 ADR 说明。
- 资产文档入口：`docs/artifacts/2026-07-16-enterprise-productionization/delivery-plan.md`、本 ADR、后续 MCP 安全测试计划与 deployment-context。

## 后续动作

| 动作 | Owner | 完成条件 |
|------|-------|----------|
| 裁决 P1 范围：Plane B（wasmtime 真执行）是否进 P1，取决于"支持不可信 / 自带工具"是否为承诺售卖点 | tech-lead | 范围结论写回 delivery-plan，放行标准口径明确 |
| 分配本 ADR 正式编号并主持 Design Review Board 收口 | tech-lead / architect | ADR 状态 Proposed→Accepted |
| Plane A：call_tool 接入验签 + capability 授权 + 审计（含灰度 / enforce 两阶段） | backend-engineer | 越权 / 验签失败负向测试通过，审计字段非空 |
| 定义 5 个记忆工具的 capability / scope 元数据与租户 RBAC 映射 | architect / backend-engineer | 映射评审通过，backfill 完成 |
| 移除 / 修正 CLAUDE.md "isolated sandboxes" 表述，与现状一致 | backend-engineer | 无"声明>实现"残留 |
| Plane B（若入 P1）：execute_wasm 真实例化 + fuel/epoch/StoreLimits + capability host fn，feature-gate | backend-engineer | 沙箱逃逸 / 资源耗尽 / capability 拒绝测试通过；未启用前不对外宣称 |
| MCP 安全测试计划（越权拒绝 + 验签 + 沙箱）纳入 P1 渗透 / 故障注入 | qa-engineer | test-plan 有执行证据，阻塞项清零 |
| 评估 signing key bundle 与集团统一密钥 / 制品签名平台对接 | architect / devops-engineer | 对接或偏离登记结论 |
