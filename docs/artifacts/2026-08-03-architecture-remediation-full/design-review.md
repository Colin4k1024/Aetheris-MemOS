# Design Review — Aetheris-MemOS 架构修复全线推进

- **日期**：2026-08-03
- **主责角色**：tech-lead（收口仲裁）
- **关联入口**：`docs/artifacts/2026-08-03-architecture-remediation-full/prd.md`
- **关联设计**：`docs/artifacts/2026-08-03-architecture-remediation-full/arch-design.md`
- **当前阶段**：`design-review`
- **目标阶段**：`handoff-ready`

---

## 结论先行

1. **需求挑战会已完成**，核心假设已验证，关键决策已确认。
2. **Arch Design 已产出**，技术方案可行，无根本性阻塞。
3. **Implementation-Readiness 结论**：✅ **Ready**（满足进入 `/team-execute` 的条件）。
4. **阻断条件**：C1/C2 已确认，P2/P3 范围已锁定，人力已确认。
5. **下一步**：产出 Handoff，进入 `/team-execute`。

---

## 设计收口会议记录

### 参与角色

| 角色 | 职责 | 贡献 |
|------|------|------|
| tech-lead | 仲裁、放行决策 | 确认 C1/C2/人力/范围 |
| product-manager | 需求挑战、假设验证 | 完成需求挑战会，质疑 4 条核心假设 |
| architect | 技术方案设计 | 产出 Arch Design，确认技术可行性 |
| backend-engineer | 技术方案细化 | 待完成（子 Agent 进行中） |
| project-manager | 排期、风险监控 | 确认 P0-P3 时间估算 |

### 核心假设验证（来自需求挑战会）

| 假设 | 质疑 | 结论 | 影响 |
|------|------|------|------|
| **企业买家会检查所有协议实现** | 大多数技术尽调聚焦 REST/MCP，不会逐协议检查 | **已验证**：P2 优先级可下调，但用户选择全线推进 | 保留 P2 |
| **P3 eval 能证明自适应优于静态** | ADR-0008 承认样本冷启动 + 选择偏差，成功概率低 | **已验证**：接受"诚实降级"可能性 | 保留 P3，接受降级风险 |
| **1-2 名后端可完成全线推进** | 人力紧张，每阶段无缓冲 | **已确认**：2 名全职后端，可支持并行 | 人力充足 |
| **应用层 tenant_id 过滤足够安全** | 代码审查发现 9 个假端点，说明遗漏存在 | **已验证**：RLS 必要 | P1 必做 |

### 替代路径分析

| 路径 | 优点 | 缺点 | 决策 |
|------|------|------|------|
| **当前路径：P0-P3 全线推进** | 完整修复，企业尽调全覆盖 | 时间长（20-29 周），人力紧张 | ✅ 采用 |
| **替代路径：仅 P0+P1** | 时间短（3-4 个月），风险低 | 失去 P2/P3 差异化卖点 | ❌ 不采用 |
| **替代路径：P3 拆分独立产品线** | 降低 P3 风险，核心系统更快交付 | 市场承诺可能受影响 | ❌ 不采用 |

**决策理由**：
1. 用户已确认 P2/P3 保留在 v1 范围内（NEW-1）。
2. 用户已确认 2 名全职后端（NEW-2），人力充足。
3. 企业买家的真实需求是"数据安全 + 工具安全 + 协议一致性 + 声明诚实"，P0-P3 全覆盖。

---

## Implementation-Readiness 验证

### 准入条件检查

| 条件 | 状态 | 证据 |
|------|------|------|
| PRD 完成 | ✅ | `prd.md` 已产出 |
| Delivery Plan 完成 | ✅ | `delivery-plan.md` 已产出 |
| Requirement Challenge 完成 | ✅ | `requirement-challenge.md` 已产出 |
| Arch Design 完成 | ✅ | `arch-design.md` 已产出 |
| C1 应用等级确认 | ✅ | T2（RPO ≤ 15min, RTO ≤ 30min） |
| C2 部署目标确认 | ✅ | 私有云/K8s |
| NEW-1 P2/P3 范围确认 | ✅ | P0-P3 全线推进 |
| NEW-2 人力确认 | ✅ | 2 名全职后端 |
| NEW-3 文档统一 | ⚠️ | 需在 Handoff 中统一两个 delivery-plan 的 enterprise.rs 决策 |

### 阻断条件检查

| 阻断条件 | 状态 | 说明 |
|----------|------|------|
| P1 地基未通过 QA 放行，P2/P3 不可启动 | ✅ | 已在 delivery-plan 中明确 |
| C1/C2 未确认，P1 不可启动 | ✅ | 已确认 |
| enterprise.rs 决策文档未统一 | ⚠️ | 需在 Handoff 中统一 |

### 风险评估

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| RLS 迁移遗漏查询路径 | 中 | 高 | DB层兜底 + 渗透测试 + 负向测试 |
| P3 eval 无法证明增益 | 高 | 中 | 诚实降级 + D7 替代差异化 |
| 人力不足导致延期 | 低 | 高 | 严格串行 + 并行项分工 |
| Git status 代码改动"跳步" | 中 | 中 | 需在 Handoff 中澄清代码改动来源 |

---

## Story Slice 列表

### P0 收口（1-2 天）

| Story | 目标 | 验收标准 | Owner | Handoff 终点 |
|-------|------|----------|-------|--------------|
| P0-1 | 摘除 enterprise.rs 路由 | 代码与文档声明一致；`cargo check` 0 error | backend-engineer | PR merged |
| P0-2 | 检查 agent_card 声明 | streaming/skills 声明与实际能力一致 | backend-engineer | PR merged |

### P1 地基（关键路径 7-10 周）

| Story | 目标 | 验收标准 | Owner | Handoff 终点 |
|-------|------|----------|-------|--------------|
| PR-1 | tenant_scope Step 1 | 纯逻辑+单测通过 | backend-engineer | PR merged |
| PR-1b | tenant_scope Step 2 | 逐 repository 接入，负向测试通过 | backend-engineer | PR merged |
| PR-2 | 审计基建 | memory_audit_events 表 + audit_writer | backend-engineer | PR merged |
| PR-3 | RLS migration | expand→backfill→enforce，跨租户负向测试通过 | backend-engineer + architect | PR merged |
| PR-5 | outbox worker | claim→Qdrant→mark，对账验证通过 | backend-engineer | PR merged |
| PR-6 | 治理 hooks | RBAC/配额/审计 hooks 接线验证通过 | backend-engineer | PR merged |
| PR-7 | MCP Plane A | call_tool 验签+授权，负向测试通过 | backend-engineer | PR merged |
| HA | HA 基建 | 备份/恢复演练通过（PG + Qdrant） | devops-engineer | 演练通过 |

### P2 多协议（6-8 周）

| Story | 目标 | 验收标准 | Owner | Handoff 终点 |
|-------|------|----------|-------|--------------|
| P2-1 | 统一 Authenticator | 传输无关核心 + 合并 web/jwt.rs | backend-engineer | PR merged |
| P2-2 | A2A 接真实 | 真实记忆服务 + 鉴权，契约测试通过 | backend-engineer | PR merged |
| P2-3 | gRPC 真 server | tonic server + interceptor 鉴权 | backend-engineer | PR merged |
| P2-4 | WebSocket 真 server | 握手鉴权 + 租户绑定 | backend-engineer | PR merged |
| P2-5 | 跨协议契约测试 | agent-card 一致性验证 | qa-engineer | 测试通过 |

### P3 自适应（7-11 周，P3a 可与 P1 并行）

| Story | 目标 | 验收标准 | Owner | Handoff 终点 |
|-------|------|----------|-------|--------------|
| P3a | 特征管线 | 从 OTel traces 提取任务特征，training_samples 表 | backend-engineer + architect | PR merged |
| P3b | 离线训练 | 离线批训练 + 模型注册表 + model card | backend-engineer + architect | 模型产出 |
| P3c | eval harness | scheduler 候选选优 + eval 报告 | backend-engineer + qa-engineer | eval 报告产出 |
| 放行/降级 | 统计显著或诚实降级 | 统计显著→放行；否则→诚实降级 | tech-lead + qa-engineer | 决策产出 |

---

## 角色分工

| 角色 | P0 | P1 | P2 | P3 |
|------|----|----|----|----|
| **tech-lead** | 仲裁、放行 | 仲裁、放行 | 仲裁、放行 | 仲裁、放行、诚实降级决策 |
| **architect** | 方案审核 | RLS migration 方案、tenant_scope 设计 | 统一 Authenticator 设计 | 特征管线设计、eval 方法论 |
| **backend-engineer** | P0 收口实现 | PR-1~PR-7 实现 | P2 实现 | P3 实现 |
| **devops-engineer** | — | HA 基建 | — | — |
| **qa-engineer** | — | P1 负向测试 | P2 契约测试 | P3 eval 验证 |
| **project-manager** | 进度跟踪 | 进度跟踪 | 进度跟踪 | 进度跟踪 |

---

## 检查节点

| 节点 | 时间 | 检查内容 | 放行条件 |
|------|------|----------|----------|
| P0 完成 | Day 2 | enterprise.rs 摘路由、agent_card 检查 | `cargo check` 0 error |
| PR-1 完成 | Week 2 | tenant_scope Step 1 | 单测通过 |
| PR-1b 完成 | Week 4 | tenant_scope Step 2 | 负向测试通过 |
| PR-3 完成 | Week 7 | RLS migration | 跨租户负向测试通过 |
| PR-5 完成 | Week 9 | outbox worker | 对账验证通过 |
| P1 放行 | Week 10 | P1 全部完成 | QA 放行报告签署 |
| P2 完成 | Week 18 | 多协议全部完成 | 契约测试通过 |
| P3 完成 | Week 29 | 自适应全部完成 | eval 报告产出或诚实降级 |

---

## 升级点

| 升级点 | 触发条件 | 升级路径 |
|--------|----------|----------|
| P1 延期超过 2 周 | 关键路径受阻 | tech-lead 评估是否裁剪 P2/P3 |
| RLS 迁移导致线上故障 | 迁移脚本问题 | 立即回滚，tech-lead 仲裁 |
| P3 eval 无法证明增益 | 统计不显著 | tech-lead 决策：诚实降级或继续投入 |
| 人力不足 | 2 名后端无法支撑 | tech-lead 评估是否增加人力或裁剪范围 |

---

## 关联文档

- PRD: `docs/artifacts/2026-08-03-architecture-remediation-full/prd.md`
- Delivery Plan: `docs/artifacts/2026-08-03-architecture-remediation-full/delivery-plan.md`
- Arch Design: `docs/artifacts/2026-08-03-architecture-remediation-full/arch-design.md`
- Requirement Challenge: `docs/artifacts/2026-08-03-architecture-remediation-full/requirement-challenge.md`

---

## 下一步

1. 等待子 Agent（architect、backend-engineer）完成设计细化
2. 产出 Handoff
3. 进入 `/team-execute`

---

**状态**：✅ **Ready for Handoff**
**当前阶段**：`design-review`
**目标阶段**：`handoff-ready`
