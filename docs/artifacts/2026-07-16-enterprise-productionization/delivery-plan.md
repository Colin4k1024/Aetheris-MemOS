# Delivery Plan — Aetheris-MemOS 企业级生产化

- **日期**：2026-07-16
- **主责角色**：project-manager（本计划）/ tech-lead（收口与仲裁）/ architect（各阶段 ADR）
- **关联入口**：后端服务化审查（见 `../2026-07-16-backend-serviceization-remediation/execute-log.md`）→ 产品定位确认为**企业级可售卖** → 立场**逐个补真实实现**
- **定位决策（已确认）**：企业级可售卖；四块能力（企业可靠性 / 自适应核心 / 多协议 / MCP 沙箱）全部补真；**分布式不自建 Raft，HA 走托管基建**。

---

## 结论先行

1. "四块全补到企业级"是一个 **6–12 个月、多人力的工程程序**，不是单个任务；本计划按依赖关系分 **P0→P3** 四阶段推进。
2. **核心原则**：企业级售卖 = 先有可信地基（隔离/持久/安全），再谈上层能力（协议/智能）。故 **P1 可靠性+安全为准入底线**，P2 协议、P3 自适应依次建于其上。
3. **已定架构方向**：分布式 HA 依赖托管基建（PG 主从/failover、Qdrant 集群、Neo4j 集群），自研 `distributed/` 假集群代码在 P0 清理/改名。
4. 时间为**粗估区间**，假设 1–2 名后端 + 兼职 devops/QA；需按实际人力校准。本计划为 living document。

---

## 版本目标（里程碑 / 范围 / 放行标准）

| 里程碑 | 范围 | 放行标准 |
|---|---|---|
| **P0 建前清理**（~1 周）| 停掉未兑现的能力广告（agent-card / CLAUDE.md）；删除/改名将被替换的死代码（`distributed/` 假集群、`strategy_mutator` 死产物、gRPC/WS 纯类型壳标注为待实现）| 代码与文档声明一致；`cargo check` 0 error；无"声明>实现"的对外表述残留 |
| **P1 地基：可靠性 + 安全**（~2–3 月）| schema RLS + `tenant_id NOT NULL` + 接线；outbox+对账接线；治理 hooks 接入请求链路；DB 备份/PITR/HA/告警/演练；MCP wasmtime 真隔离 + `call_tool` 验签 | 跨租户隔离在 DB 层强制（RLS 通过渗透测试）；崩溃无部分写/无孤儿向量（对账可自愈）；备份可恢复演练通过；MCP 工具在沙箱内执行且越权被拒；QA 放行 |
| **P2 多协议接真实**（~1.5–2 月）| A2A handler 接真实记忆服务 + 真 task store + 真流式；gRPC（tonic）真 server；WebSocket 真 server + 鉴权 | 三协议端到端产生真实记忆操作（非假数据）；均受鉴权保护；契约测试通过；agent-card 能力声明与实测一致 |
| **P3 自适应核心**（~2–4 月，含研究）| 遥测→特征管线；predictor 从实测性能拟合；scheduler 用真预测；在线更新系数替换假 mutator；eval harness | eval 证明自适应策略在离线基准上**显著优于**静态最优配置；predictor 置信度有真实依据；无写死常量喂决策 |

---

## 工作拆解

### P0 建前清理（tech-lead + backend-engineer）
| 工作项 | 主责 | 依赖 | 计划 |
|---|---|---|---|
| agent-card 去除 streaming/未实现 skill 声明；CLAUDE.md 移除"沙箱隔离/自进化"等未兑现表述 | backend | — | 1–2d |
| 删除 `distributed/` 假集群（consensus/replication/sharding/node），保留并改名实际在用的单进程原语（epoch/interrupt/lease/signaling）| architect+backend | 方向已定 | 2–3d |
| 删除 `strategy_mutator` 死产物路径；gRPC/WS 纯类型壳加 `#[cfg]`/文档标注"待 P2 实现" | backend | — | 1–2d |

### P1 地基（architect + backend + devops + qa）
| 工作项 | 主责 | 依赖 | 计划 |
|---|---|---|---|
| schema RLS + `tenant_id NOT NULL` 迁移 + 全仓库查询接线（替换前缀+应用层过滤）| architect+backend | ADR-0001 | 3–4wk |
| outbox 表接线 + 幂等重放 + 后台对账 worker（替换同步双写补偿）| backend | ADR-0002 | 2–3wk |
| 治理 hooks（RBAC/配额/审计）接入请求中间件链路 | backend | — | 1–2wk |
| DB 备份/PITR/HA/告警/回滚演练（PG/Qdrant/Neo4j 托管或自管高可用）| devops | ADR-0003 | 2–4wk |
| MCP wasmtime 真沙箱执行 + capability 强制 + `call_tool` 验签 | backend | 新增 ADR | 2–3wk |
| P1 渗透/故障注入/恢复演练测试 | qa | 以上 | 贯穿 |

### P2 多协议（architect + backend + qa）
| 工作项 | 主责 | 依赖 | 计划 |
|---|---|---|---|
| A2A handler 接真实记忆服务 + task store + 真流式（重新启用 `--features a2a`，pin 依赖）| backend | P1 核心稳定 | 2–3wk |
| gRPC（tonic）真 server 实现 + 鉴权 | backend | 协议 ADR | 2–3wk |
| WebSocket 真 server（连接管理/订阅/推送）+ 鉴权 | backend | 协议 ADR | 2wk |
| 跨协议契约测试 + agent-card 一致性校验 | qa | 以上 | 贯穿 |

### P3 自适应核心（architect + backend + qa）
| 工作项 | 主责 | 依赖 | 计划 |
|---|---|---|---|
| 遥测→特征管线（复用 OTel）+ 性能样本存储 | backend | OTel 已在发 | 2–3wk |
| predictor 从实测性能拟合（替换写死常量）| backend+architect | 样本积累 | 3–5wk |
| scheduler 用真预测 + 在线系数更新（替换假 mutator）| backend | predictor | 2–3wk |
| eval harness：离线基准证明优于静态配置 | qa+backend | 以上 | 2–3wk |

---

## 风险与缓解

| 风险 | 影响 | 缓解 | Owner |
|---|---|---|---|
| RLS 接线改动面大，易漏查询导致隔离绕过 | 跨租户泄漏（企业红线）| 全量清点数据访问点（已有 tenant-production-path-inventory）；RLS 为 DB 层兜底；渗透测试 | architect |
| 离线环境无法连 PG/Qdrant 验证迁移与 RLS 行为 | 迁移/查询回归只能静态验证 | 建一套 Testcontainers 集成测试环境（连真 PG/Qdrant）作为 CI gate | qa+devops |
| 自适应"从实测拟合"缺训练数据、效果不确定 | P3 差异化卖点可能不达预期 | 先做 eval harness 与基准，用数据决定是否值得继续投入；失败则诚实降级为"启发式配置选择" | architect |
| a2a-rs 未 pin git 依赖 + 离线不可拉 | P2 启用 A2A 受阻 | 联网一次 pin rev + 提交 lock（见 execute-log 收尾步骤）；评估 vendor | backend |
| "四块并进"人力摊薄 | 全线拖期、地基未稳先上协议 | 严格按 P1→P2→P3 串行；P3 遥测地基可在 P1 并行起步，其余不抢跑 | tech-lead |
| 托管 HA 基建选型/成本未定 | P1 的 DB 高可用无法落地 | P1 前由 architect+devops 出基建选型 ADR（托管 vs 自管）| devops |

---

## 节点检查

- **方案评审（Design Review Board）**：每阶段进入前，architect 出 ADR + tech-lead 主持评审通过方可开工；P1 前必须完成 MCP 沙箱 ADR、HA 基建选型 ADR。
- **开发完成**：该阶段工作项代码合并 + `cargo check`/`clippy` 0 error + 单测/契约测试通过。
- **测试完成**：qa 按阶段放行标准验证（P1 渗透+恢复演练、P2 契约、P3 eval）；阻塞项清零。
- **发布准备**：deployment-context + release-plan + launch-acceptance 齐备（P1 起每阶段上线均需）。

---

## ADR 清单

| ADR | 状态 | 阶段 |
|---|---|---|
| ADR-0001 租户隔离基线 | 已有 | P1 |
| ADR-0002 向量 outbox 与对账 | 已有 | P1 |
| ADR-0003 存储运维就绪 gate | 已有 | P1 |
| ADR-0004 **MCP 沙箱执行模型（双平面；Plane B 已确认入 P1）** | 已写 Proposed | P1 |
| ADR-0005 **HA 基建选型（托管优先）** | 已写 Proposed | P1 |
| ADR-0006 **企业集群协调（成熟原语，非自建共识）** | 已写 Proposed | P1/P2 |
| ADR-00xx 多协议传输与跨协议鉴权 | 待写 | P2 |
| ADR-00xx 自适应学习方法与 eval 方法论 | 待写 | P3 |

---

## 已定关键决策

- **分布式不自建**：不实现自研 Raft/复制/分片；HA 依赖托管/成熟基建。`distributed/` 假集群代码 P0 删除，实际在用的单进程 cancellation/pool/signaling 原语保留并**改名去除"distributed"误导**。
- **顺序不可抢跑**：P1 可靠性+安全是企业售卖准入底线，先于 P2 协议、P3 自适应；仅 P3 的遥测地基允许在 P1 并行起步。

---

## 决策更新（2026-07-16 追加）

用户在 P0/P1 设计评审中确认两项，据此调整范围：

1. **R4 反转：`services/enterprise.rs` 假集群不删、改为做真**（见 `docs/adr/ADR-0006-enterprise-cluster-coordination.md`）。用**成熟协调原语**实现（PG advisory lock 选主 + 一致性哈希分片路由 + `cluster_nodes`/`memory_shards`/`tenant_licenses` 表 + 许可分级门控），**不自建 Raft**——与 ADR-0005「不自建」一致（应用层协调 ≠ 数据存储 HA）。新增工作流：
   - **P1**：许可/套餐分级（`tenant_licenses` + 治理 hooks 门控）——与治理 hooks 接入同批。
   - **P2**：多实例集群协调（选主/成员/分片路由 + 心跳/re-balance 守护），需真 PG + 多实例环境。
   - 端点路径不变、语义改真；同步更新对外 API 文档（去"假集群"表述）。
2. **MCP Plane B 确认入 P1**（见 `docs/adr/ADR-0004-mcp-sandbox-execution-model.md`）：产品会跑不可信/自带工具，故 P1 除 Plane A（第一方验签+授权+审计）外，**必须交付 Plane B（wasmtime 真沙箱执行）**——`execute_wasm` 从 mock 换真实例化 + capability host fn + fuel/epoch/StoreLimits 资源限制。

> 说明：上述均为多周工作流，需真 PG/Qdrant/多实例/（Plane B）wasm 工具链，**离线会话内只能出设计（ADR）+ 计划**；落码与验证待基建/网络就绪。P0 阶段对 `services/enterprise.rs` **不再执行删除**（原 P0 R4 删除建议作废），保留至 P1/P2 原地做真。
