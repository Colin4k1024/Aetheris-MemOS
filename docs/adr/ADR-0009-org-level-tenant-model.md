# ADR-0009: Org 层租户模型（`tenant_id` 与 `user_id` 解耦）

## 决策信息

- 编号：ADR-0009
- 决策标题：把 `tenant_id` 从 `user_id` 解耦，引入 org 层租户与租户内多用户多角色
- 状态：**Proposed**
- 实现状态：**未落地**
- 日期：2026-08-11（提出）
- Owner：architect
- 收口责任人：待 `tech-lead` 主持 Design Review Board 收口（仍 open）
- 关联 backlog：C-3（本项）；被它阻塞的：A-1、C-1、P0-6、P0-7、C-5
- 关联 ADR：ADR-0001（租户隔离基线，本 ADR 是其 org 层续作）

## 背景与约束

### 当前模型：一个用户就是一个租户

`backend/src/tenant/context.rs:127-132`：

```rust
pub fn new(user_id: impl Into<String>) -> Self {
    let user_id = user_id.into();
    Self {
        tenant_id: TenantId::from_string(&user_id),  // ← tenant_id == user_id
        user_id,
    }
}
```

配套地，`services/rbac.rs` 对首次出现的主体惰性授予其自身租户的 `Owner`。

结果：**每个用户都是自己单用户租户的唯一 Owner**。这不是一个 bug，是 MVP 的有意简化（`context.rs` 原注释写明「For MVP: each user is their own tenant」）。但它使一整批已完成的授权工作**在今天没有可观测效果**。

### 它阻塞了四项已完成的工作

| backlog 项 | 已完成的内容 | 为什么今天没有效果 |
|---|---|---|
| A-1 | MCP capability 按主体从 RBAC 角色派生 | 所有主体都是 Owner，派生结果恒为 `[Read, Write, Delete]` |
| C-1 | 5 组管理面路由加强类型 `Permission` 门禁 | 所有主体都是 Owner，`has_permission` 恒 true |
| P0-6 / P0-7 | 17 个 handler 比对请求里的 `tenant_id` 与调用方身份 | 跨租户已被挡住，但**同租户内的角色区分**无从体现 |
| C-5 | `routers/tenant.rs` 的角色分配 handler 已就绪并已加固 | 无处可分配——一个租户只有一个用户 |

也就是说：**授权机制已经建成，但它作用于一个只有一种角色的世界。**

### 不做会怎样

不是「功能缺失」，而是**对外表述与实现持续失真**：文档与 ADR 描述的是一个有 Owner/Admin/Member/Reader 四级角色的多租户系统，而运行时只有 Owner。这正是 2026-08 这轮整改要消除的模式。

### 约束

1. **不能破坏 ADR-0001 的隔离基线。** RLS 依赖 `db::tenant_scope::begin_tenant_tx` 设置的事务局部 GUC `aetheris.tenant_id`；任何新模型都必须继续经过这个 keystone，否则 fail-close 失效。
2. **不能让现有部署的数据变得不可读。** 现存行的 `tenant_id` 等于某个 `user_id`，迁移必须为它们构造对应的 org 租户。
3. **JWT 契约变更影响所有客户端。** 当前 token 的主体即租户；引入 org 后 token 需同时承载 org 与 user。
4. 应用等级与数据风险按 ADR-0001 的判定继承（涉及跨租户数据，按更高风险等级处理）。

### 非目标

- 不引入组织层级树（org 下再分 team/project）。只做一层。
- 不实现 SSO / SCIM / 邀请流程。只做模型与授权。
- 不改变 STM/LTM/KG/MM 四层的数据模型——`tenant_id` 列语义不变，变的是它取自哪里。

## 备选方案

### 方案 A：新增 `tenants` 与 `tenant_members` 表，JWT 承载 `org_id` + `sub`

- **做法**：建 `tenants(tenant_id, name, created_at, ...)` 与
  `tenant_members(tenant_id, user_id, role, assigned_at)`（复合主键）。JWT 增加
  `org` claim；`RequestTenantContext` 从 `org` 取 `tenant_id`、从 `sub` 取
  `user_id`。RBAC 从 `tenant_members` 读角色而非惰性授予。
- **适用条件**：需要真正的多用户租户，且愿意承担一次 JWT 契约变更。
- **优点**：模型直白；`tenant_members` 天然是角色的单一真相源；RLS 与
  `begin_tenant_tx` 完全不用改（`tenant_id` 的来源变了，语义没变）。
- **风险 / 成本**：JWT 契约破坏性变更（需要过渡期同时接受带/不带 `org` 的
  token）；需要数据迁移为每个既有 `user_id` 建一个同名单成员 org；
  `services/rbac.rs` 的惰性授予要整段替换。
- **不选原因**：无（这是推荐方案）。

### 方案 B：保留 `tenant_id == user_id`，用「委派」表达多用户

- **做法**：不改租户模型，增加 `delegations(grantor_user, grantee_user, role)`，
  让 A 把自己租户的部分权限授予 B。
- **适用条件**：只需要「共享我的记忆给某人」，不需要企业组织概念。
- **优点**：零 JWT 变更、零数据迁移，RLS 完全不动。
- **风险 / 成本**：**授权语义会变成图而非树**——判断「B 能否读某条记忆」要遍历
  委派关系，RLS 的单 GUC 模型表达不了这个，会被迫把跨租户判断挪回应用层。
  那正是 P0-6/P0-7 刚修掉的失效模式（应用层判断被绕过）。
- **不选原因**：把已经收敛到 DB 层的隔离决策重新推回应用层，与 ADR-0001 的
  方向相反。

### 方案 C：不做，明确对外表述为「单用户租户」

- **做法**：不改代码。把 README / CLAUDE.md / ADR 里的多角色描述全部改成
  「每用户一租户，角色系统为未来预留」。删除或标注 `routers/tenant.rs` 的角色
  分配端点。
- **适用条件**：产品短期不面向企业客户。
- **优点**：零实现成本，且**立即消除失真**——这是它真正的价值。
- **风险 / 成本**：A-1、C-1、C-5 的已完成工作变成纯粹的未来预留（不是浪费，
  但收益推迟）；若企业需求出现需回头做方案 A。
- **不选原因**：待 tech-lead 判断产品方向后定。**如果短期没有企业客户，
  方案 C 比方案 A 更诚实**——本 ADR 不预设方案 A 必胜。

## 决策结果

**待 Design Review Board 收口。** 本 ADR 提出问题与备选，不单方面拍板，因为
方案 A 与方案 C 的取舍取决于产品方向（是否面向企业多用户场景），那不是架构
可以独立决定的。

倾向性意见（供 DRB 参考）：

- 若产品方向包含企业多用户 → **方案 A**。它是唯一能让已建成的授权机制真正
  生效、且不违反 ADR-0001 隔离基线的路径。
- 若产品方向短期不含企业多用户 → **方案 C**，并把 A-1/C-1/C-5 标注为
  「机制就位、待 org 模型激活」而非「已生效」。
- **方案 B 不建议**，理由见上（把隔离决策推回应用层）。

## 企业内控补充（按需填写）

- 应用等级：继承 ADR-0001 的判定。涉及跨租户数据与授权，按更高风险等级处理。
- 技术架构等级：无新增中间件或平台组件；方案 A 只增两张表与 JWT claim。
- 关键组件偏离：无。
- 资产文档入口：待方案确定后补部署与迁移说明。

## 后续动作

若采纳方案 A，实施顺序（每步可独立验证，不要合并）：

1. **迁移**：建 `tenants` / `tenant_members` 两表，并为每个既有 `tenant_id`
   建一条同名 org + 一条 `Owner` 成员记录。此步不改任何读写路径，可单独上线。
2. **JWT 过渡期**：`hoops/jwt.rs::authenticate` 同时接受带 `org` 与不带 `org`
   的 token；不带时回落到当前行为（`tenant_id = user_id`）。加指标统计两种
   token 的占比，用它决定何时可以移除回落。**不要**先移除回落再观察。
3. **RBAC 换源**：`services/rbac.rs` 从 `tenant_members` 读角色，删除惰性
   授予。此步之后 A-1 与 C-1 的门禁开始产生差异——**必须在此步补集成测试**，
   验证 Reader 确实无法调 `memory_write`、Admin 确实无法访问 billing。
4. **激活 `routers/tenant.rs`**：按 C-5 的模块 doc 处理两件事——与
   `multi_tenant_router::register_tenant` 的职责重叠必须二选一；
   `reset_tenant_memory` 需要超出租户检查的额外门禁。
5. **收口**：把 A-1、C-1、P0-6、P0-7 的 backlog 条目从「机制就位、待 C-3」
   更新为「已生效」，并补上第 3 步的集成测试作为证据。

需要同步的文档：`CLAUDE.md` 的 Multi-Tenant Isolation 段、ADR-0001 的实现状态
字段、`docs/memory/backlog.md` 的 A-1/C-1/P0-6/P0-7/C-5 条目。

需要通知的角色：`tech-lead`（DRB 收口）、`product-manager`（产品方向输入）、
`backend-engineer`（实施）、`qa-engineer`（第 3 步的角色区分集成测试）。

完成条件：第 3 步的集成测试能证明**至少两种角色的行为不同**。在那之前，本
ADR 的实现状态不得标为已落地——「模型建好了」不等于「角色能区分了」。
