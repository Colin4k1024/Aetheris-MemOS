# Decisions Log

> 跨任务决策事实源（**追加，不覆盖**）。每条记录最小字段：日期 / 决策标题 / 背景 / 决策 / 影响。
> 详细技术论证以对应 ADR 为准；本文只沉淀「为什么这么定 + 影响面 + 已知中间状态」，
> 防止决策与半成品状态只活在对话里，被下一个人误读为「已做完 / 漏做」。
>
> - 建立日期：2026-08-11
> - 更新角色：`backend-engineer` / `architect`

---

## 2026-08-11 — MCP 双平面沙箱：建通路、第一方不套 wasm、Plane B 生产 registry 留空

- **关联**：`docs/adr/ADR-0004-mcp-sandbox-execution-model.md`；backlog A-2 / D-4（任务 #6，依赖 #5「按主体授予集合」已完成）。
- **触点**：`backend/src/routers/mcp.rs`（`classify_plane` / `call_extension_tool` / `McpState.sandbox_proxy`）、`backend/src/mcp/sandbox_proxy.rs`、`backend/src/mcp/sandbox.rs`、`backend/src/mcp/mod.rs`。
- **验证**：`cargo check` 0 error；`cargo test --lib` 358 passed / 0 failed / 3 ignored；上述 4 文件 `rustfmt --check` 无偏差。

### 背景

ADR-0004 定义「Plane A 可信第一方工具 + Plane B 不可信扩展工具」双平面模型。落地前的真实状态：`sandbox.rs` 已是真 wasmtime 实现，但 `execute_wasm`（真沙箱入口）**零调用点、零测试**；`SandboxProxy::execute_tool` 更是**忽略传入的 capability policy、直接调原生 `tool.execute()`**——即真沙箱躺在那里，而唯一的代理入口绕过了它。这是典型的「存在 X ≠ X 在起作用」：若只是把这个 proxy 接进 live 路径，会得到一个**看起来接好了的假隔离**，比不接更危险。同时当前产品**没有任何**不可信 / 第三方 / 自带工具——`call_tool` 执行的只有 5 个第一方记忆工具。

### 决策

1. **第一方记忆工具（memory_write / search / recall / forget / list）故意不走 wasm，保持原生执行。** 这是**有意决策，不是漏做**：第一方工具本就需要特权 DB 访问，套 wasm 只能把 DB 重新开成宽 host function 暴露回去，隔离收益≈0，却给可信路径凭空加上 fuel 限制和失败模式。将来若有人看到「第一方没进沙箱」，应视为设计选择而非缺口。

2. **Plane B 通路已接线，但生产环境 registry 为空——「通路存在」不等于「Plane B 可用」。** `call_tool` 顶部新增 `classify_plane`（**必须先于** Plane A 的 `capability::authorize`，否则其 deny-by-default 会把一切非第一方工具当 `UnknownTool` 拒掉，Plane B 永远进不去）：FirstParty→原生；Extension→`SandboxProxy`（真正路由到 `WasmSandbox::execute_wasm` 并消费 policy）；Unknown→拒绝+审计。顺带修掉了上面那个「忽略 policy、调原生 execute」的假沙箱，并给 `execute_wasm` 补了首个真实执行测试。**但扩展工具注册来源为空**，所以今天任何非第一方工具都会被拒。下一个看到 `SandboxProxy` 有调用者的人，不要据此以为 Plane B 已经能跑扩展工具。

3. **`execute_wasm` 当前的 capability 语义是占位符，不是真正的最小权限。** 现逻辑要求「同时授予全部 4 个 capability（Network + FsRead + FsWrite + Env）才能运行任何模块」（`sandbox.rs` 的 capability 校验循环）。这是**已知错误语义**：纯计算的 wasm 工具本不需要任何 host capability（当前根本没注册 host functions），此检查方向反了。重设计属 Plane B 设计工作，在真实扩展工具落地时一并做。此处显式登记，避免它变成隐性技术债。

### 影响

- Plane A 行为**零变化**（逐字保留 signing → 按主体 capability 授权 → governance → 原生 dispatch），无回归风险。
- `SandboxProxy` 从零引用死代码变为 live Plane B 执行器；删除了误导性的 `SandboxedTool` trait（其模型是「原生执行」，与 wasm 沙箱矛盾）。
- CLAUDE.md 的 MCP Sandbox 段已由 tech-lead 按双平面模型重写（含 registry 生产为空、`execute_wasm` capability 语义为占位符）。
- **仍 open 的 tech-lead 裁决项**：是否把「不可信 / 自带扩展工具」作为承诺售卖点。**在该裁决落定前，不应构建扩展工具的注册来源、Plane B 验签与 host-function 授予模型**——否则是造一个没有消费者的假能力，正是本轮真实性整改要杜绝的模式。裁决为「是」则按 ADR-0004 Plane B 作为工程项落地；为「否」则维持当前「通路在、registry 空、对外不宣称」的诚实中间态。
