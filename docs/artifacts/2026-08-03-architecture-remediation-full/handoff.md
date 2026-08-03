# Handoff — Aetheris-MemOS 架构修复全线推进

- **日期**：2026-08-03
- **主责角色**：tech-lead（handoff 发起）/ backend-engineer（handoff 接收）
- **关联入口**：`docs/artifacts/2026-08-03-architecture-remediation-full/prd.md`
- **关联设计**：`docs/artifacts/2026-08-03-architecture-remediation-full/arch-design.md`
- **当前阶段**：`handoff-ready`
- **目标阶段**：`execute`

---

## 背景

Aetheris-MemOS 需要从 MVP 进入企业级可售卖状态。2026-08-03 架构审查确认技术选型合理，但存在"声明 > 实现"差距。本次 handoff 基于需求挑战会结论和 Arch Design，将任务从设计阶段推进到实现阶段。

---

## 输入依据

| 文档 | 状态 | 关键内容 |
|------|------|----------|
| PRD | ✅ 完成 | P0-P3 四阶段路线图，用户故事，成功标准 |
| Delivery Plan | ✅ 完成 | 工作拆解，时间估算，风险与缓解 |
| Requirement Challenge | ✅ 完成 | 核心假设验证，4 条质疑结论 |
| Arch Design | ✅ 完成 | 系统边界，组件拆分，关键数据流，技术选型 |
| Design Review | ✅ 完成 | Implementation-Readiness 验证，Story Slice 列表 |

---

## 结论

### 关键决策

| 决策 | 内容 | 依据 |
|------|------|------|
| **D1** | P0-P3 全线推进（v1 范围） | 用户确认 NEW-1 |
| **D2** | 应用等级 T2（RPO ≤ 15min, RTO ≤ 30min） | 用户确认 C1 |
| **D3** | 部署目标私有云/K8s | 用户确认 C2 |
| **D4** | 2 名全职后端 | 用户确认 NEW-2 |
| **D5** | enterprise.rs 从路由摘除（保留文件） | 需求挑战会决策 D1 |

### 技术方案要点

1. **P0 收口**（1-2 天）：摘除 enterprise.rs 路由，检查 agent_card 声明
2. **P1 地基**（关键路径 7-10 周）：
   - tenant_scope 执行器（应用层→DB层）
   - RLS 迁移（expand→backfill→enforce）
   - outbox worker（异步向量化）
   - MCP Plane A（验签+授权）
   - HA 基建（K8s 原生 + PG 主从复制）
3. **P2 多协议**（6-8 周）：统一 Authenticator + A2A/gRPC/WS 接真实
4. **P3 自适应**（7-11 周，P3a 可并行）：特征管线 + 离线训练 + eval harness

---

## 风险

| 风险 | 影响 | 缓解措施 | Owner |
|------|------|----------|-------|
| RLS 迁移遗漏查询路径 | 跨租户泄露 | DB层兜底 + 渗透测试 + 负向测试 | backend-engineer |
| P3 eval 无法证明增益 | 差异化卖点丢失 | 诚实降级 + D7 替代差异化 | tech-lead + qa |
| Git status 代码改动"跳步" | 方案不一致 | 需澄清代码改动来源，对齐 Arch Design | backend-engineer |
| 人力不足导致延期 | 全线拖期 | 严格串行 + 并行项分工 | tech-lead |

---

## 待确认项

| 编号 | 事项 | 状态 | 阻塞阶段 |
|------|------|------|----------|
| **NEW-3** | 两个 delivery-plan 的 enterprise.rs 决策统一 | ⚠️ 待统一 | P0 收口前 |
| **CODE** | Git status 代码改动的来源和依据 | ✅ 已澄清 | — |
| **T1** | P2 代码是否暂存分支还是保留 main | ❓ 待确认 | P1 完成前 |
| **T2** | MCP Plane B 是否进 P1 | ❓ 待确认 | P1 执行前 |
| **T3** | P3 是否纳入本轮还是降级 | ❓ 待确认 | P3 启动前 |

**说明**：
- NEW-3：2026-07-16 delivery-plan 说"R4 反转：不删做真"，2026-08-03 PRD 说"从路由摘除（保留文件）"。需在执行前统一文档。
- CODE：已由 backend-engineer 澄清。详见 `technical-design.md` §1。
- T1/T2/T3：来自 backend-engineer 上游质疑，需 tech-lead 确认。

### Git Status "跳步"分析结论

| 阶段 | 文件数 | 判定 | 建议 |
|------|--------|------|------|
| **P0 正确** | 2 | `routers/mod.rs`（路由摘除）+ `agent_card.rs`（streaming 诚实化） | 立即提交 |
| **P2 提前** | 9 | 统一 Authenticator 核心 + gRPC/WS/A2A 鉴权适配器 + A2A handler 接真服务 | 暂不提交，P1 完成后作为 P2 PR-1 合入 |
| **P3 提前** | 4 | analyzer/monitor 诚实化 + feature_pipeline + migration | 诚实化改动可提交；feature_pipeline 暂不提交 |

**核心风险**：P1 RLS 未 enforce 就实现 P2 的 A2A handler 接真服务，可能绕过租户隔离。
**缓解措施**：P2 代码保留但标记为 `#[cfg(feature = "p2")]` 或 `#[allow(dead_code)]`，P1 RLS enforce 后再接线。

---

## 当前阶段

`handoff-ready`

---

## 目标阶段

`execute`

---

## 就绪状态

**✅ `handoff-ready`**

满足进入 `/team-execute` 的条件：
1. PRD、Delivery Plan、Requirement Challenge、Arch Design、Design Review 已完成
2. C1/C2/人力/范围已确认
3. Implementation-Readiness 验证通过
4. Story Slice 列表已产出

---

## Readiness Proof

| 检查项 | 状态 | 证据 |
|--------|------|------|
| PRD 完成 | ✅ | `prd.md` |
| Delivery Plan 完成 | ✅ | `delivery-plan.md` |
| Requirement Challenge 完成 | ✅ | `requirement-challenge.md` |
| Arch Design 完成 | ✅ | `arch-design.md` |
| Design Review 完成 | ✅ | `design-review.md` |
| C1 应用等级确认 | ✅ | T2 |
| C2 部署目标确认 | ✅ | 私有云/K8s |
| NEW-1 P2/P3 范围确认 | ✅ | P0-P3 全线推进 |
| NEW-2 人力确认 | ✅ | 2 名全职后端 |
| Implementation-Readiness 验证 | ✅ | Ready for Handoff |

---

## 阻塞项

| 阻塞项 | 状态 | 说明 |
|--------|------|------|
| NEW-3 文档统一 | ⚠️ 待处理 | 需在 P0 收口前统一两个 delivery-plan 的 enterprise.rs 决策 |
| CODE 代码改动澄清 | ✅ 已澄清 | backend-engineer 已分析，结论：P2 代码暂不提交，P1 完成后再合入 |
| T1 P2 代码分支策略 | ❓ 待确认 | 建议暂存分支，P1 完成后合入 |
| T2 MCP Plane B 范围 | ❓ 待确认 | 建议不进 P1（当前无不可信工具） |
| T3 P3 范围确认 | ❓ 待确认 | 建议 P3-lite（配置推荐引擎）先行 |

**说明**：
- NEW-3 不阻塞进入 `/team-execute`，但需在 P0 收口前处理。
- CODE 已澄清，不阻塞。
- T1/T2/T3 来自 backend-engineer 上游质疑，需 tech-lead 确认后执行。

---

## 下一跳角色

**backend-engineer**（主责实现）

---

## 下一跳产出

| 产出 | 说明 |
|------|------|
| P0 收口实现 | 从 `routers/mod.rs` 摘除 `/enterprise` 路由块 |
| P1 PR-1~PR-7 实现 | tenant_scope、RLS、outbox、治理 hooks、MCP Plane A |
| 测试覆盖 | 单元测试、集成测试、负向测试 |
| Execute Log | 记录实现过程中的关键决定和阻塞 |

---

## 技能装配清单

| 技能 | 触发阶段 | 原因 |
|------|----------|------|
| `command-rust-build` | P0-P3 全程 | Rust 编译问题快速修复 |
| `command-rust-review` | 每个 PR | Rust 代码质量门禁 |
| `command-rust-test` | P1-P3 | TDD 驱动实现 |
| `command-build-fix` | 随时 | 构建错误快速修复 |

---

## 下游质疑记录

> 依据 `rules/handoff-contract.md` 要求，接收方（backend-engineer）必须对上游输入提出质疑。

### 质疑 1：Git status 代码改动的来源和依据

- **质疑内容**：git status 显示大量 backend 文件已修改（a2a/*, protocol/*, services/*, web/jwt.rs等），包括新增 `feature_pipeline.rs` 和 `training_samples` 迁移文件。这些改动是否基于 Arch Design？是否存在"跳步"实现？
- **质疑目标**：Arch Design 和 Design Review 未提及这些代码改动
- **结论**：**已澄清**
- **处理说明**：backend-engineer 已完成分析（见 `technical-design.md` §1）。结论：P0 正确（2 文件），P2 提前（9 文件），P3 提前（4 文件）。建议 P2 代码暂不提交，P1 完成后作为 P2 PR-1 合入。

### 质疑 2：接口契约的落地成本是否被低估？

- **质疑内容**：delivery-plan 将 PR-1b（逐 repository 接入）估为 1.5-2 周，但实际需要改造的 repository 数量至少 6 个，每个 repository 的写入路径平均 5-8 个方法。PR-1b 的 1.5-2 周可能偏乐观。
- **质疑目标**：delivery-plan 中 PR-1/PR-1b 的工期估算
- **结论**：**接受风险但需监控**
- **处理说明**：PR-1 纯逻辑部分确实 1-1.5 周可完成。PR-1b 的 1.5-2 周假设"每个 repository 改动模式相同"，但 `kg.rs`（Neo4j 双后端）和 `mm.rs`（多模态）复杂度较高，可能需要额外 0.5-1 周。建议在 PR-1b 开始前做一次精确的 repository 方法盘点。

### 质疑 3：数据模型是否过度或不足？

- **质疑内容**：P3 的 `config_archetypes` 的 8 个 seed 值是静态硬编码的。ADR-0008 要求"候选配置空间"支持按租户 SLA 调整权重，但当前设计没有动态扩展机制。
- **质疑目标**：`config_archetypes` 表的扩展性设计
- **结论**：**接受原方案**
- **处理说明**：8 个 archetype 覆盖了常见的记忆层组合，权重可在 JSON 中按租户调整。新增 archetype 只需 INSERT，无需 schema 变更。在 P3 验证阶段，固定配置空间是**有意为之**的。

### 质疑 4：异常路径是否被充分覆盖？

- **质疑内容**：outbox worker 的异常路径设计依赖 `MAX_ATTEMPTS = 8` 和 `STALE_LOCK_SECS = 120`，但未定义 dead-letter 后的告警阈值和人工介入流程。ADR-0002 要求"超过阈值进入 dead-letter 并告警"，但当前实现只有 `warn!` 日志。
- **质疑目标**：outbox worker 的 dead-letter 处理和告警闭环
- **结论**：**要求补充**
- **处理说明**：需在 P1 PR-5 阶段增加：(1) dead-letter 事件的 Prometheus 指标（`outbox_dead_letter_total`）；(2) Grafana 告警规则（dead-letter > 0 持续 5min）；(3) 运维 runbook 中的处理流程。已列入 PR-5 的放行标准。

---

## 关联文档

- PRD: `docs/artifacts/2026-08-03-architecture-remediation-full/prd.md`
- Delivery Plan: `docs/artifacts/2026-08-03-architecture-remediation-full/delivery-plan.md`
- Arch Design: `docs/artifacts/2026-08-03-architecture-remediation-full/arch-design.md`
- Requirement Challenge: `docs/artifacts/2026-08-03-architecture-remediation-full/requirement-challenge.md`
- Design Review: `docs/artifacts/2026-08-03-architecture-remediation-full/design-review.md`
- ADR-0001~0008: `docs/adr/`

---

## 确认记录

| 角色 | 确认内容 | 时间 |
|------|----------|------|
| tech-lead | Handoff 产出，就绪状态 `handoff-ready` | 2026-08-03 13:20 UTC+8 |
| backend-engineer | 待接收（需先澄清 CODE 质疑） | — |

---

**状态**：✅ **Handoff Ready**
**当前阶段**：`handoff-ready`
**目标阶段**：`execute`
**就绪状态**：`handoff-ready`
