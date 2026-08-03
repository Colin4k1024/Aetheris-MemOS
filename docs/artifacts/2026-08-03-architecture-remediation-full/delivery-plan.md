# Delivery Plan — Aetheris-MemOS 架构修复全线推进（P0–P3）

- **日期**：2026-08-03
- **主责角色**：tech-lead（本计划收口与仲裁）/ architect（方案）/ project-manager（排期）
- **关联入口**：架构审查报告（2026-08-03）→ 需求挑战会（2026-08-03）
- **前置文档**：
  - `docs/artifacts/2026-08-03-architecture-remediation-full/prd.md`（intake PRD）
  - `docs/artifacts/2026-07-16-enterprise-productionization/delivery-plan.md`（原始 P0-P3 计划）
  - `docs/artifacts/2026-07-16-enterprise-productionization/p1-execution-runbook.md`（P1 PR 依赖拓扑）

---

## 结论先行

1. 技术选型无根本性错误（Axum + PG + Qdrant + OTel），ADR 体系完善（8 篇）。核心问题是**"声明 > 实现"差距**阻塞企业级售卖。
2. 已有 P0-P3 路线图覆盖审查发现的所有主要问题。本次 intake 在其基础上做了**优先级校准和范围调整**。
3. 关键路径 7-10 周（tenant_scope → RLS → outbox），HA 基建全程并行。
4. **MCP Plane B 从 P1 阻塞项降级为非阻塞项**（需产品 Owner 确认）。
5. 新增缺口（手写 OpenAPI、langchain-rust）降级为 tech-debt，不阻塞任何阶段。

---

## 版本目标

| 里程碑 | 范围 | 放行标准 | 粗估时间 |
|--------|------|----------|----------|
| **P0 收口**（~1 周） | enterprise.rs 假集群从路由摘除；agent-card 检查 | 代码与文档声明一致；`cargo check` 0 error | **1-2 天** |
| **P1 地基**（~2-3 月） | tenant_scope + RLS + outbox + 治理 hooks + MCP Plane A + HA 基建 | 跨租户隔离 DB 层强制；MCP 验签+授权；备份恢复演练通过 | **关键路径 7-10 周** |
| **P2 多协议**（~1.5-2 月） | 统一 Authenticator + A2A/gRPC/WS 接真实 + 鉴权 | 三协议端到端真实+受鉴权；契约测试通过 | **6-8 周** |
| **P3 自适应**（~2-4 月） | 特征管线 + predictor 拟合 + scheduler 选优 + eval harness | eval 证明优于静态最优或诚实降级 | **7-11 周**（P3a 可并行） |

---

## 需求挑战会结论（2026-08-03）

### 关键决策

| # | 决策 | 仲裁依据 |
|---|------|----------|
| D1 | enterprise.rs 假集群**从路由摘除但保留文件**（P2 做真复用） | 假控制面比"没实现"更损害信任；摘除 ≈ 10 分钟 |
| D2 | P1 关键路径 = PR-1→PR-3→PR-5；HA 基建 + 审计基建**可并行** | 依赖拓扑分析 |
| D3 | tenant_scope 执行器**分两步**：Step 1 纯逻辑+单测 → Step 2 逐 repository 接入 | 降低单次 PR 改动面 |
| D4 | **MCP Plane A = P1 阻塞项；Plane B = P1 非阻塞项**（⚠️ 待产品 Owner 确认） | 当前无不可信工具；ADR-0004 论证第一方不需要 WASM |
| D5 | P3a（基准集+特征管线）**可与 P1 并行起步** | 不依赖 P1 产出 |
| D6 | 手写 OpenAPI + langchain-rust **降级为 tech-debt** | 非安全阻塞项 |
| D7 | P3 降级后差异化 = 多层记忆架构 + MCP 生态 + 多协议互操作 | 不依赖"自适应"标签 |

### 挑战会质疑记录

| 分组 | # | 质疑内容 | 目标 | 结论 |
|------|---|----------|------|------|
| A: P0 | 1 | enterprise 假集群是否阻塞售卖？ | P0 R4 | **是**——技术尽调发现"假装实现"比"没实现"更严重 |
| A | 2 | 摘除路由是否误伤 hoops::enterprise？ | R4 安全性 | **不会**——模块路径完全独立 |
| A | 3 | P0 摘除路由与 P2 做真是否冲突？ | P0/P2 衔接 | **不冲突**——保留文件，P2 重新挂载 |
| B: P1 | 1 | P1 是否有并行空间？ | 串行假设 | **部分接受**——HA+审计可并行 |
| B | 2 | tenant_scope 是否有更轻量替代？ | PR-1 复杂度 | **无**——手动 SET LOCAL 易遗漏；但可分两步降低风险 |
| B | 3 | MCP Plane B 投入产出比？ | Plane B 进 P1 | **降级为非阻塞项** |
| C: P3 | 1 | P3 降级后差异化？ | 产品定位 | 多层记忆+MCP+多协议 |
| C | 2 | "静态最优"基线如何定义？ | eval 方法论 | 冻结测试集上训练集以外的最优配置 |
| C | 3 | P3 时间是否含数据等待？ | P3 估算 | **不含**——用离线基准；P3a 可并行 |
| D: 缺口 | 1 | 手写 OpenAPI 是否需立即替换？ | 新增缺口 | **tech-debt** |
| D | 2 | langchain-rust 实际风险？ | 新增缺口 | **tech-debt**——退场路径 = reqwest |

---

## 工作拆解

### P0 收口（1-2 天）

| 工作项 | 主责 | 计划 |
|--------|------|------|
| 从 `routers/mod.rs` 摘除 `/enterprise` 路由块（保留文件和 mod 声明） | backend | 0.5d |
| 检查 a2a/agent_card.rs streaming/skills 声明 | backend | 0.5d |

### P1 地基

#### 关键路径（串行，7-10 周）

| PR | 工作项 | 主责 | 依赖 | 计划 |
|----|--------|------|------|------|
| PR-1 | tenant_scope 执行器 Step 1（纯逻辑+单测） | backend | — | 1-1.5wk |
| PR-1b | tenant_scope 执行器 Step 2（逐 repository 接入） | backend | PR-1 | 1.5-2wk |
| PR-3 | RLS migration（expand→backfill→enforce）+ 查询接线 | backend+architect | PR-1b | 3-4wk |
| PR-5 | outbox worker + 对账 | backend | PR-3 | 2-3wk |

#### 可并行项

| PR | 工作项 | 主责 | 依赖 | 计划 |
|----|--------|------|------|------|
| PR-2 | 审计基建（memory_audit_events + audit_writer） | backend | —（与 PR-1 并行） | 1wk |
| PR-6 | 治理 hooks 接入请求中间件 | backend | PR-1b | 1-2wk |
| PR-7 | MCP Plane A（call_tool 验签 + capability 授权） | backend | PR-2 | 2-3wk |
| PR-8 | MCP Plane B（wasmtime 真沙箱）— **非阻塞** | backend | — | 2-3wk |
| HA | DB 备份/PITR/HA/告警/演练 | devops | —（全程并行） | 2-4wk |

#### P1 放行标准

- [ ] 跨租户负向测试通过（所有 memory-owned tables）
- [ ] MCP call_tool 验签 + 越权拒绝负向测试通过
- [ ] 备份/恢复演练通过（PG + Qdrant）
- [ ] 治理 hooks 接线验证通过
- [ ] QA 放行

### P2 多协议（6-8 周）

| 工作项 | 主责 | 依赖 | 计划 |
|--------|------|------|------|
| 传输无关 Authenticator 核心 + 合并 web/jwt.rs | backend | P1 核心稳定 | 1-1.5wk |
| A2A handler 接真实记忆服务 + 鉴权 | backend | 网络 pin 依赖 | 2-3wk |
| gRPC（tonic）真 server + interceptor 鉴权 | backend | — | 2-3wk |
| WebSocket 真 server + 握手鉴权 + 租户绑定 | backend | — | 2wk |
| 跨协议契约测试 + agent-card 一致性 | qa | 以上 | 贯穿 |

### P3 自适应核心（7-11 周，P3a 可与 P1 并行）

| 阶段 | 工作项 | 主责 | 依赖 | 计划 |
|------|--------|------|------|------|
| P3a | 基准集设计 + 特征管线 + training_samples 表 | backend+architect | OTel 已就绪 | 2-3wk |
| P3b | 离线批训练 + 模型注册表 + model card | backend+architect | P3a | 3-5wk |
| P3c | scheduler 候选选优 + predictor + eval harness | backend+qa | P3b | 2-3wk |
| 放行/降级 | 统计显著→放行；否则→诚实降级 | tech-lead+qa | P3c | 1wk |

---

## 风险与缓解

| 风险 | 影响 | 缓解 | Owner |
|------|------|------|-------|
| RLS 迁移遗漏查询路径 | 跨租户泄露 | RLS 为 DB 层兜底；渗透测试 | architect |
| P3 eval 无法证明增益 | 差异化卖点丢失 | 诚实降级 + D7 替代差异化 | architect |
| a2a-rs git 依赖离线不可拉 | P2 A2A 受阻 | 联网一次 pin rev | backend |
| 人力不足 | 全线拖期 | 严格串行 P1→P2→P3；P3a 可并行 | tech-lead |
| 托管 HA 选型/成本未定 | P1 HA 无法落地 | architect+devops 出选型结论 | devops |

---

## 升级门禁（待产品 Owner / tech-lead 确认）

| # | 事项 | 建议 | 阻塞阶段 |
|---|------|------|----------|
| G1 | MCP Plane B 是否从 P1 降级为非阻塞项 | **建议降级** | P1 |
| G2 | 应用等级（T1/T2/T3）定档 | 按 T2/T3 处理 | P1 |
| G3 | 部署目标确认 | 需确认以启动 HA 选型 | P1 HA |

---

## 关联文档

- PRD：`docs/artifacts/2026-08-03-architecture-remediation-full/prd.md`
- 原始 delivery-plan：`docs/artifacts/2026-07-16-enterprise-productionization/delivery-plan.md`
- P1 execution runbook：`docs/artifacts/2026-07-16-enterprise-productionization/p1-execution-runbook.md`
- ADR-0001~0008：`docs/adr/`
