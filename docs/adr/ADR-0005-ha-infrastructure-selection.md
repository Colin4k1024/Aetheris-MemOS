# ADR-0005: HA 基建选型（托管 vs 自管）

## 决策信息

- 编号：ADR-0005
- 决策标题：三大数据存储（PostgreSQL / Qdrant / Neo4j）的高可用能力**优先采用托管/成熟基建交付，禁止自研分布式底座**
- 状态：Accepted
- 实现状态：已完整落地 —— 本 ADR 交付的是选型 / 红线决策，代码红线已被遵守（`distributed/` 是进程内协调原语、无自研共识/复制/分片）；托管 provisioning、RPO/RTO 定档、恢复演练本就是部署期动作（见「非目标」），不构成代码实现缺口。
- 日期：2026-07-16（提出）；2026-08-11（按实现核实收口状态）
- Owner：devops-engineer / architect
- 收口责任人：tech-lead（红线与选型方向已于 2026-07-16 仲裁，2026-08-11 按代码核实收口决策轴）
- 关联需求：`docs/artifacts/2026-07-16-enterprise-productionization/delivery-plan.md`（P1 地基 · 可靠性）
- 关联 ADR：ADR-0001（租户隔离）、ADR-0002（向量 outbox 与对账）、ADR-0003（存储运维就绪 gate）、ADR-0004（MCP 沙箱执行模型）
- 关联部署上下文：`docs/artifacts/2026-07-06-memory-storage-reliability/deployment-context.md`、`release-plan.md`
- 评审记录：tech-lead 于 2026-07-16 仲裁通过两点——(a)"托管优先、operator 自管为合规回落"的表述；(b) operator 管理的集群不违反"不自建 Raft"红线（红线为"不手写共识/复制/分片"）。

---

## 实现核实与缺口（2026-08-11 收口）

本 ADR 是**基建选型 / 红线决策**（「HA 走托管、禁止自研分布式底座」），核实的是「代码是否遵守红线」而非「托管集群是否已开通」（后者属部署期动作，不在代码仓内）。

**已核实与决策一致（故 `Accepted`）：**

- **红线被遵守**：`backend/src/distributed/` 是**进程内**协调原语（epoch 取消、中断传播、lease、workflow signaling），**没有**自研 Raft / Paxos / 复制 / 分片。这与本 ADR「不手写共识」及 CLAUDE.md「NOT a distributed cluster — HA delegated to managed infrastructure」一致。
- **应用连接角色已为托管/隔离铺垫**：`migrations/20260810000000_create_app_role.sql` 建 `aetheris_app`（NOSUPERUSER），迁移文件明确要求生产密码来自 secrets manager、禁默认口令——与本 ADR「密钥入 secret manager、禁默认口令」一致。

**部署期 follow-up（缺口，属正常——本 ADR 明确「不在本 ADR 内实现集群或执行演练」）：**

1. 托管平台确认与 provisioning、RPO/RTO 定档、选型闸门（pgvector / APOC / Qdrant Cloud 区域）、成本对比——均为「后续动作」表列项，尚未闭合，取决于合规与采购。
2. HA 告警项（replica lag / failover / backup age / snapshot age / cluster health / PITR window）尚未接入 `monitoring/`（当前 alert 规则覆盖 backend down / reconciliation / outbox，尚无 HA 专项）。
3. 恢复演练证据（restore / failover / rebuild）未归档——与 ADR-0003 的 blocked 门禁互为依赖。
4. 需同步修订 2026-07-06 `deployment-context.md` / `release-plan.md` 与 delivery-plan 的「PG 自建 HA」→「托管优先」（本 ADR 结论先行第 5 点自陈的 follow-up）。

> 说明：这些 follow-up 不阻碍本 ADR 收口为 `Accepted`——本 ADR 交付的是**选型决策**，决策方向已定且代码红线被遵守；provisioning 与演练本就被划为部署期动作（见「非目标」与「后续动作」）。

## 结论先行

1. **方向落地**：延续 delivery-plan 的"分布式不自建"决策——不写自研 Raft/复制/分片。三大存储的 HA、备份、恢复能力**一律由托管服务或成熟基建（数据库 operator / 官方集群）承接**，我们只做接线、配置、演练与告警。
2. **推荐组合（倾向托管）**：
   - **PostgreSQL（事实源，最高优先级）→ 托管 DBaaS**（RDS/Aurora / Cloud SQL/AlloyDB / Azure Flexible Server / 阿里云 RDS 之一），Multi-AZ 自动 failover + 自动备份 + PITR。
   - **Qdrant（可重建索引）→ 托管 Qdrant Cloud**，replication factor ≥ 2；因 ADR-0002 已保证"PostgreSQL fact + outbox/对账重建"兜底，Qdrant durability 可接受较低档，成本敏感时允许降级。
   - **Neo4j（当前定为生产必选）→ 托管 Neo4j Aura**（Business Critical/Enterprise 档），托管备份 + HA；**但附两个准入闸门**（APOC 兼容性核验、关键路径等级复核）。
3. **"托管优先"是分层偏好，不是唯一解**：Tier-1 托管 DBaaS > Tier-2 K8s operator 自管高可用（CloudNativePG / Qdrant 集群 / Neo4j 因果集群）> Tier-3 主从+手工 failover（生产不接受，仅非关键路径临时态）。**"自管高可用集群"仍属"用成熟基建"，不违反"不自建"**——我们不碰的红线是自己写 `distributed/` 里的共识/复制/分片。
4. **合规前置**：以上"托管优先"**假设存在获批的托管平台**（公有云或集团内部 DBaaS）。若企业强制私有云/on-prem 且无托管 DB 平台，则回落到 Tier-2 operator 自管；该回落需在部署上下文显式记录，不改变"禁止自研底座"的红线。
5. **本 ADR 变更既有假设**：2026-07-06 的 `deployment-context.md` / `release-plan.md` 及 delivery-plan 写的是"PostgreSQL 自建 HA"。本 ADR 将其**修订为"托管优先"**；这些文档的同步更新由 tech-lead 作为 follow-up 处理（见后续动作）。

---

## 背景与约束

### 当前问题（无证据）

ADR-0003 已确立"企业高可靠准入必须含备份恢复 + HA + 告警 + 发布回滚 + 演练证据"，但审查结论是三大存储**均无任何 HA/备份/恢复证据**：

- 生产拓扑仅有 `docker-compose.yml` 的单节点：`pgvector/pgvector:pg16`、`qdrant/qdrant:v1.9.4`、`neo4j:5`（社区版默认口令 `neo4j/password`），仅供开发，**不能代表生产**。
- PostgreSQL：无 primary/replica、无 PITR/WAL archive、无 restore drill。
- Qdrant：无 cluster/replication、无 snapshot schedule、无 rebuild 演练。
- Neo4j：无 cluster、无 backup/dump/restore runbook；社区版本身不支持在线备份与集群。
- 监控栈（Prometheus/Grafana/OTel/Jaeger）只有采集，无 HA 相关告警（replica lag / failover / backup age / cluster health）。

### 业务目标

企业级可售卖产品的可靠性准入底线（delivery-plan P1）：跨故障可用、数据可恢复、恢复可演练、异常可告警——且需**可追溯的证据**而非声明。

### 约束条件

- **不自研分布式底座**（delivery-plan 已定）：`distributed/` 假集群 P0 删除，HA 依赖外部成熟能力。
- **PostgreSQL 是 memory-owned relational fact 的唯一事实源**（ADR-0001/0002）；Qdrant 是可从 DB 重建的索引；Redis 只承接缓存/短期协调，不做不可恢复事实源。
- 企业内控（`enterprise-architecture-governance.md` / `enterprise-component-baseline.md`）：优先复用集团获批平台与组件；偏离集团标准需 ADR 记录原因、责任人、退场路径。
- 多租户敏感数据（ADR-0001）：需实例级隔离、静态/传输加密、备份加密、私网接入。
- 应用等级、RPO/RTO 尚未由 tech-lead 定档（ADR-0001/0003 遗留项）；本 ADR 给出**待确认的默认目标**。
- pgvector / APOC 依赖：生产 PG 若实际使用 pgvector、Neo4j 若依赖 APOC，托管候选必须支持——列为选型闸门。

### 非目标

- 不在本 ADR 内实现集群或执行演练（落到 release-plan 与 drill runbook）。
- 不锁定唯一云厂商；给出选型框架与推荐序，最终报价对比由 devops+tech-lead 定稿。
- 不定义全公司统一 on-call 流程（沿用 ADR-0003 告警闭环要求）。
- 不重新设计 Qdrant 重建链路（沿用 ADR-0002）。

---

## 备选方案

三大存储的能力需求一致（备份/PITR、failover/HA、监控、成本、运维负担），但**关键性与可重建性不同**，故分别评估。

### 通用取舍轴

| 档位 | 形态 | 是否违反"不自建" | 运维负担 | 生产可用性 |
|------|------|------------------|----------|------------|
| Tier-1 托管 DBaaS | RDS/Aura/Qdrant Cloud 等 | 否 | 最低 | ✅ 推荐 |
| Tier-2 operator 自管高可用 | CloudNativePG / Qdrant 集群 / Neo4j 因果集群 + pgBackRest/WAL-G/snapshot | 否（用成熟基建） | 中-高 | ✅ 合规回落 |
| Tier-3 主从+手工 failover | 流复制 + repmgr/手动 promote | 否但脆弱 | 中 | ❌ 生产不接受 |
| （红线）自研共识/复制/分片 | 自写 `distributed/` | **是** | — | 🛑 禁止 |

> 说明："不自建 Raft"指**我们**不写 Raft。Qdrant/Neo4j 集群内部各自使用 Raft 属于其成熟实现，选用它们的集群模式**不违反**红线（tech-lead 2026-07-16 已确认）。

### A. PostgreSQL（事实源 · 最高优先级）

| 方案 | 备份 / PITR | Failover / HA | 监控 | 成本 | 运维负担 | 不选原因 / 取舍 |
|------|-------------|---------------|------|------|----------|-----------------|
| **A1 托管 DBaaS**（RDS/Aurora、Cloud SQL/AlloyDB、Azure Flexible、阿里云 RDS） | 自动备份 + PITR（保留窗口典型 7–35 天），WAL 归档托管 | Multi-AZ 备节点，自动 failover（RDS 典型 60–120s；Aurora 通常 < 30s） | 厂商指标 + Performance Insights，可接 Prometheus | 单价高，**零运维人力** | 最低 | **推荐**。需闸门：确认 pgvector 扩展支持与版本 |
| A2 operator 自管高可用（CloudNativePG / Patroni+etcd） | pgBackRest / WAL-G 到对象存储，PITR 自建 | operator/Patroni 自动 failover（典型 < 60s） | 自建 postgres_exporter + 告警 | 基建费低，**运维人力高**（需 DBA、打补丁、值守） | 高 | 合规回落项（无托管平台时）；运维摊薄风险 |
| A3 主从 + 手工/半自动 failover（repmgr） | 仍需 pgBackRest/WAL-G | repmgr 或手动 promote，RTO 高、有脑裂风险 | 自建 | 基建费最低 | 中，出错率高 | **不选**：手工 failover 不满足企业 HA，RTO 不可控 |

### B. Qdrant（向量索引 · 可从 DB 重建）

> ADR-0002 已保证 PostgreSQL fact + durable outbox + 对账可重建 Qdrant，因此 Qdrant 的自身 durability 不是最高优先级——这放宽了其选型档位。

| 方案 | 备份 / 快照 | Failover / HA | 监控 | 成本 | 运维负担 | 不选原因 / 取舍 |
|------|-------------|---------------|------|------|----------|-----------------|
| **B1 托管 Qdrant Cloud**（RF ≥ 2） | 自动快照/备份托管 | 集群多副本 + 节点自愈 | 内置面板 + Prometheus 指标 | 订阅费，零运维 | 最低 | **推荐**。可能偏离集团标准组件 → 需退场路径记录 |
| B2 operator 自管集群（K8s Helm，RF ≥ 2 + shards） | 快照 API 定时到对象存储 + DB 重建 | 集群副本容忍单节点丢失（内部 Raft 管元数据） | 自建 | 基建 + 运维 | 中（比 PG HA 简单） | 数据驻留/成本要求时的合规回落项 |
| B3 单节点/热备 + 快照 + 从 PostgreSQL 重建 | 定时快照 + ADR-0002 重建链路 | 无原生 HA，靠重建；RTO 较高但**数据不丢**（DB 为事实源） | 自建 | 最低 | 低 | 可作为成本敏感兜底态；关键在于重建演练必须真实通过 |

### C. Neo4j（图 / KG · 当前定为生产必选）

> **成本/许可关键**：Neo4j 社区版**不支持集群与在线备份**（仅离线 dump/load）；集群 + 在线备份需 **Enterprise 商业许可**。这使自管 Neo4j 的 TCO 最高。

| 方案 | 备份 / 恢复 | Failover / HA | 监控 | 成本 | 运维负担 | 不选原因 / 取舍 |
|------|-------------|---------------|------|------|----------|-----------------|
| **C1 托管 Neo4j Aura**（Business Critical/Enterprise 档） | 自动每日备份 + 按需快照（非严格 PITR） | 托管多实例 HA + 自动 failover | Aura 控制台指标，Prometheus 有限 | 较贵，零运维、**免自管 Enterprise 许可** | 最低 | **推荐**。闸门：APOC 兼容性（Aura 仅 APOC Core 子集）、Cypher/插件核验 |
| C2 自管 Enterprise 因果集群（core + read replica） | `neo4j-admin backup` 在线备份（Enterprise） | 因果集群自动 failover（内部 Raft） | 自建 + JMX | **Enterprise 许可 + 基建 + 运维**，总成本最高 | 高 | 若需 APOC Extended 或数据必须 on-prem 时的回落项 |
| C3 社区单实例 + 离线 dump/load | 仅离线 dump（备份需停写/停机） | **无** HA | 自建 | 最低 | 中 | **不选**：无 HA、备份需停机，不满足"生产必选" |

---

## 决策结果

### 采用方案

**三大存储 HA 一律托管/成熟基建承接，禁止自研底座；默认托管优先，按下表定档：**

| 存储 | 首选（Tier-1） | 合规回落（Tier-2） | 关键约束 / 闸门 |
|------|----------------|--------------------|-----------------|
| PostgreSQL | 托管 DBaaS + Multi-AZ + PITR | CloudNativePG operator + pgBackRest/WAL-G | 确认 pgvector 支持；实例级租户隔离 + 加密 + 私网 |
| Qdrant | 托管 Qdrant Cloud，RF ≥ 2 | K8s operator 集群 RF ≥ 2；成本敏感可降级到单节点+快照+DB 重建 | 快照 schedule；ADR-0002 重建链路必须真实演练通过 |
| Neo4j | 托管 Neo4j Aura（Business Critical/Enterprise） | 自管 Enterprise 因果集群 | **APOC 兼容性核验**；**关键路径等级复核**（见下）|

### RPO/RTO 目标（待 tech-lead 按应用等级定档，以下为 T2/T3 默认建议）

| 存储 | RPO（建议） | RTO（建议） | 依据 |
|------|-------------|-------------|------|
| PostgreSQL | ≤ 5 min（近同步备库/WAL）；可要求同步副本达 RPO≈0 | ≤ 15 min（托管自动 failover < 2min + 应用重连） | 事实源，最严 |
| Qdrant | ≤ 24h（快照）叠加 DB 实时重建能力 | 集群档 ≤ 15 min；重建档数十分钟–数小时 | 可重建，可放宽 |
| Neo4j | ≤ 1h（在线/每日备份） | ≤ 30–60 min | 视关键路径复核结果收紧或放宽 |

### 理由

- 托管以最小运维人力交付企业级 failover/备份/PITR，直接契合"不自建 + 1–2 人后端 + 兼职 devops"的现实人力（delivery-plan 风险项）。
- PG 作为事实源投入最高档；Qdrant 借 ADR-0002 重建能力省成本；Neo4j 借托管规避 Enterprise 许可自管负担。
- 分层（托管 > operator 自管 > 手工主从）在满足企业合规（私有云/数据驻留）时仍保持"不自建"红线。

### 影响范围

- `docs/artifacts/2026-07-06-memory-storage-reliability/deployment-context.md`：将"PostgreSQL 自建 HA"修订为"托管优先"；补三大存储托管/回落拓扑。
- `docs/artifacts/2026-07-06-memory-storage-reliability/release-plan.md`：Phase 7 演练项按托管形态改写（托管 failover 触发/备份恢复/快照恢复）。
- `docs/artifacts/2026-07-16-enterprise-productionization/delivery-plan.md`：将"PG 自建 HA"细化为"托管优先、operator 自管为合规回落"。
- `monitoring/`：新增 HA 告警（replica lag、failover event、backup age/success、snapshot age、cluster node health、PITR window）。
- 配置：`DATABASE_URL`/Qdrant endpoint/Neo4j URI 指向托管私网 endpoint；密钥入 secret manager；禁默认口令。
- CI：Testcontainers 集成测试仍用容器版 PG/Qdrant/Neo4j 作 gate（delivery-plan 风险缓解项），生产拓扑与之解耦。

### 兼容性 / 迁移影响

- `docker-compose.yml` 继续仅作 local/dev，不被提升为生产证据（延续 ADR-0003）。
- 迁移路径（single-node → 托管/HA）：
  1. 依合规确认部署目标（公有云 / 集团 DBaaS / 私有云）与厂商候选。
  2. 过选型闸门（pgvector、APOC、Qdrant Cloud 区域/驻留）。
  3. Provision 托管实例 → 跑 migrations → 数据迁移（PG：`pg_dump`/逻辑复制；Qdrant：快照恢复或从 DB 重建；Neo4j：dump/load）。
  4. 连接串切换灰度 → 验证 → 下线单节点。
- 本 ADR 修订 2026-07-06 文档与 delivery-plan 的"自建"假设，需同步更新以免文档与真相漂移（enterprise-architecture-governance 要求资产与部署一致）。

### 失败或回退思路

- 托管 provisioning 被合规/采购阻塞 → 回落 Tier-2 operator 自管高可用（获批 K8s 上），**不回退到自研底座**。
- 某存储未过选型闸门（如 PG 无 pgvector、Aura 缺所需 APOC Extended）→ 仅该存储切 Tier-2 自管，其余保持托管。
- Neo4j 托管成本过高或 APOC 不兼容 → 触发**关键路径等级复核**：若 KG 可降级为"故障期只读/旁路增强索引"，则可降档；若确为关键路径，接受 Enterprise 自管成本或托管高档。此复核回 tech-lead 决策（ADR-0003 已留此口子）。
- 任一演练（failover/restore/rebuild）失败 → 发布状态 blocked，按升级策略交 tech-lead 裁决，不以"稍后补"放行。

---

## 企业内控补充

- **应用等级**：待 tech-lead 按业务评定 + 底线评定定档；无法确认前按 T2/T3 高风险内控口径处理。RPO/RTO 随等级细化（T1 需关键资源独占/强隔离与独立容量）。
- **技术架构等级**：三大存储 + 备份归档 + 监控告警 + 发布回滚均需纳入资产可视可控（TA 图 / 部署架构图 / 接口文档 / 监控入口）。
- **资源隔离**：多租户敏感数据要求实例级隔离（至少独立 DB 实例）、静态加密 + 传输 TLS、备份加密、私网/VPC endpoint 接入；T1 场景要求独立集群与独立容量。
- **平台偏离**：优先复用集团获批 DBaaS / K8s DB operator 平台；采用外部托管（Qdrant Cloud / Neo4j Aura）若偏离集团标准组件基线，须在本 ADR 与后续 ADR 记录原因、责任人、升级窗口与退场路径。
- **资产文档入口**：`docs/artifacts/2026-07-06-memory-storage-reliability/deployment-context.md`、`release-plan.md`、后续 restore/failover drill runbook。

---

## 后续动作

| 动作 | Owner | 完成条件 |
|------|-------|----------|
| 确认应用等级 + RPO/RTO 目标定档 | tech-lead | 目标写入 deployment-context，替换本 ADR 的"建议值" |
| 确认部署目标（公有云 / 集团 DBaaS / 私有云 / on-prem）与合规约束 | tech-lead / devops / 架构治理 | 明确 Tier-1 是否可用，否则确认 Tier-2 回落 |
| 选型闸门评估：pgvector 支持、Qdrant Cloud 区域/驻留、Neo4j Aura APOC 兼容性 | devops + architect | 每存储给出通过/回落结论 |
| **Neo4j 关键路径等级复核**（生产必选 vs 可降级增强） | tech-lead + architect | 决定 Neo4j 目标档位与成本口径 |
| 成本对比与选型定稿（≥3 家托管报价 vs 自管 TCO 含许可） | devops + tech-lead | 选型定稿并记入 ADR/deployment-context |
| 制定备份策略（PG PITR/WAL 保留窗口、Qdrant 快照 schedule、Neo4j 备份周期） | devops | 策略写入 deployment-context，含加密与归档位置 |
| 定义并接入 HA 告警项（replica lag / failover / backup age / snapshot age / cluster health / PITR window） | devops | 告警可触发、有 owner 与升级路径（对齐 ADR-0003） |
| 设计并执行恢复演练 runbook（restore / failover / Qdrant rebuild / Neo4j restore） | devops + qa | 演练证据按 test-plan 格式归档（日期/环境/命令/结果/残余风险/owner） |
| 同步修订 2026-07-06 `deployment-context.md` / `release-plan.md` 与 delivery-plan（"自建"→"托管优先"） | tech-lead | 文档与本 ADR 方向一致，无"声明>实现"残留 |
