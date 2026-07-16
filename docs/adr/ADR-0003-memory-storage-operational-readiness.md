# ADR-0003: Memory Storage Operational Readiness Gate

## 决策信息

- 编号：ADR-0003
- 决策标题：企业高可靠准入必须包含备份恢复、HA、告警、发布回滚 runbook 与演练证据
- 状态：Proposed
- 日期：2026-07-06
- Owner：tech-lead / devops-engineer / qa-engineer
- 关联需求：`docs/artifacts/2026-07-06-memory-storage-reliability/prd.md`
- 关联测试计划：`docs/artifacts/2026-07-06-memory-storage-reliability/test-plan.md`

## 背景与约束

当前仓库已有本地开发拓扑、healthcheck、Prometheus scrape、OpenTelemetry、Jaeger 和 Grafana datasource provisioning。但这些证据不足以说明系统已经满足企业内部高可靠生产要求。

审查发现的主要运维缺口：

- PostgreSQL 缺少 HA、PITR、WAL archive、restore drill 证据。
- Qdrant 缺少 cluster、snapshot schedule、restore 或 rebuild 演练。
- Neo4j 缺少 cluster、backup、dump/load、restore runbook。
- Prometheus/Grafana 只有采集基础，缺少告警规则、SLO、pager routing。
- CI 和文档缺少 migration rollback / forward-fix、release checklist 和 launch acceptance。
- `SelfHealingService` 中部分健康检查和恢复动作是模拟实现，不能作为真实恢复证据。

约束：

- 生产拓扑可能由托管服务或外部平台承接，代码仓不一定包含完整部署配置。
- 即使使用托管服务，也必须在 deployment context 中记录 RPO/RTO、备份、恢复、告警和责任人。
- 本 ADR 定义准入要求，不强制指定唯一供应商或部署工具。

非目标：

- 不在本 ADR 中实现 HA 集群。
- 不定义全公司统一 on-call 流程。
- 不要求 local docker-compose 承担生产职责。

## 备选方案

| 方案 | 适用条件 | 优点 | 风险或成本 | 不选原因 |
|------|----------|------|------------|----------|
| 仅依赖应用测试通过 | PoC、本地演示 | 成本低 | 无法证明恢复、HA、告警和回滚能力 | 不满足企业生产要求 |
| 文档声明使用托管服务即可 | 生产由外部平台托管 | 简化代码仓配置 | 没有实际 RPO/RTO、告警和演练证据 | 证据不足 |
| 每次发布前强制完整灾备演练 | 高监管关键系统 | 证据最强 | 成本高，不适合每次小版本 | 可用于重大版本或 T1 场景 |
| 建立 operational readiness gate，按等级要求提交 runbook 和演练证据 | 企业内部生产 | 成本可控，证据可追溯 | 需要跨角色协作 | 采用 |

## 决策结果

企业高可靠准入必须通过 Memory Storage Operational Readiness Gate。未通过前，不得声明 memory storage 已满足企业内部生产级高可靠。

准入要求：

1. **备份恢复**
   - PostgreSQL：PITR 或等价备份恢复方案、RPO/RTO、restore drill 记录。
   - Qdrant：snapshot restore 或从 PostgreSQL + outbox/reconciliation rebuild 的演练记录。
   - Neo4j：如果生产依赖 KG backend，则必须有 backup / dump / restore 记录。

2. **HA / 故障切换**
   - PostgreSQL：托管 HA 或自建 primary/replica + failover 方案。
   - Qdrant：cluster/replication 或明确的降级和恢复策略。
   - Neo4j：cluster/read replica 或明确非关键路径说明。
   - backend：多实例部署、健康检查和负载均衡说明。

3. **告警闭环**
   - DB/Qdrant/Neo4j unavailable。
   - backup age / restore failure。
   - outbox backlog / dead-letter。
   - reconciliation drift。
   - tenant isolation violation。
   - latency / error rate / saturation。
   - 告警接收 owner 和升级路径。

4. **发布回滚**
   - migration dry-run。
   - expand-contract 策略。
   - rollback 或 forward-fix 路径。
   - 发布前 smoke 和发布后 observation window。
   - launch acceptance 和 closeout summary。

5. **测试和证据**
   - `test-plan.md` P0 项全部有执行结果。
   - operational drills 记录日期、环境、命令、结果、失败项、残余风险和 owner。

影响范围：

- `docs/artifacts/2026-07-06-memory-storage-reliability/test-plan.md`
- 后续 `deployment-context.md`、`release-plan.md`、`launch-acceptance.md`、`closeout-summary.md`
- `monitoring/` alert rules 和 dashboards
- CI / release workflow
- DevOps runbooks

兼容性 / 迁移影响：

- 当前 `docker-compose.yml` 继续作为 local/dev 入口，不被提升为生产 HA 证据。
- 现有测试报告继续保留，但需要标注其证明的是功能可用性，不等价于企业高可靠。
- 发布流程需要新增运维证据门禁，可能延长上线准备时间。

失败或回退思路：

- 如果某项运维证据无法在代码仓内完成，必须在 deployment context 中链接外部平台证据并说明 owner。
- 如果告警或恢复演练失败，发布状态为 blocked，需由 tech-lead 根据升级策略裁决。
- 如果生产拓扑尚未确定，不能以“稍后补充”方式放行企业高可靠声明。

## 企业内控补充

- 应用等级：待确认；若涉及主营业务主链路、敏感数据或 7x24 要求，应按更高等级处理。
- 技术架构等级：存储、向量索引、图数据库、缓存、观测和发布回滚均需进入资产可视可控。
- 关键组件：PostgreSQL、Qdrant、Neo4j、Redis、backend service、monitoring stack。
- 平台偏离：local docker-compose 只能代表开发环境，不代表生产架构。
- 资产文档入口：后续应补 `deployment-context.md`、`release-plan.md`、`launch-acceptance.md` 和 `closeout-summary.md`。

## 后续动作

| 动作 | Owner | 完成条件 |
|------|-------|----------|
| 确认目标应用等级、RPO、RTO | tech-lead | PRD 待确认项关闭 |
| 编写 deployment context | devops-engineer | 环境、部署入口、配置、备份、恢复和监控入口明确 |
| 编写 release plan | devops-engineer | migration、rollback、smoke、观察窗口明确 |
| 增加 alert rules 和 dashboard 链接 | devops-engineer | 告警可触发并有 owner |
| 执行 restore / rebuild / rollback drills | devops-engineer / qa-engineer | test-plan operational drills 有证据 |
| 形成 launch acceptance | qa-engineer / tech-lead | 放行结论、阻塞项和残余风险明确 |
| 发布后收口并更新 backlog | tech-lead | closeout summary 和 backlog 同步完成 |
