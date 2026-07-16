# PRD: Memory Storage Reliability 企业高可靠整改

## 背景

Aetheris MemOS 当前已经具备 STM / LTM / KG / MM 记忆存储、Qdrant 向量检索、PostgreSQL migration、租户前缀隔离、基础 observability 和环境型 E2E 测试能力，适合本地开发、PoC 和受控演示。

企业内部高可靠审查发现，当前实现尚不能直接作为生产级高可靠记忆存储上线。关键问题包括：

- 多租户隔离主要依赖 `t:{tenant}` 字符串前缀和应用层约定，缺少数据库级 `tenant_id`、tenant-scoped constraints 和 PostgreSQL RLS。
- 部分 update / history / time-travel / relation 路径未强制绑定 request tenant。
- STM、KG 等多步写入缺少事务边界，存在半写和统计不一致风险。
- LTM 与 Qdrant 采用同步双写 + 局部补偿，缺少 durable outbox、幂等重放和 reconciliation。
- PostgreSQL / Qdrant / Neo4j 当前主要是单节点开发拓扑，缺少生产 HA、备份恢复、灾备演练和发布回滚证据。
- 监控采集基础存在，但告警规则、SLO、pager routing、恢复失败告警和租户隔离违规告警不足。

本 PRD 将审查结论转化为企业高可靠整改范围和验收标准，为后续架构设计、代码实现、测试验证和发布治理提供事实源。

## 目标与成功标准

### 业务目标

- 将记忆存储从“可用的 MVP substrate”提升到“可进入企业内部生产评审的可靠存储能力”。
- 为多租户企业场景提供可证明、可审计、可恢复的隔离与持久化能力。
- 建立存储故障、向量索引漂移、备份恢复、发布回滚的可执行闭环。

### 用户价值

- **平台管理员** 可以明确当前集群是否满足备份、恢复、HA、告警和回滚准入。
- **后端工程师** 可以按统一的 TenantId、事务和 outbox 模型实现存储变更，减少隐式耦合。
- **QA 工程师** 可以用负向租户测试、事务失败测试和恢复演练判断是否放行。
- **DevOps 工程师** 可以按 runbook 执行备份恢复、告警验证和发布回滚。
- **企业审计方** 可以追溯关键记忆写入、删除、transfer、隔离拒绝和恢复演练证据。

### 成功指标

1. STM / LTM / KG / MM 及 history / relation / outbox 相关表具备显式 `tenant_id` 或有文档化兼容桥接策略。
2. 所有 memory repository 的 update、delete、history、time-travel、relation mutation 路径显式绑定 `TenantId`。
3. PostgreSQL RLS 或等价数据库级策略默认拒绝缺失租户上下文的 memory 数据访问。
4. STM append/delete、KG relation mutation、LTM supersede/history mutation 等多步写入使用事务。
5. LTM 与 Qdrant 通过 durable outbox + async worker + reconciliation 保证可恢复的 eventual consistency。
6. Qdrant 搜索结果必须带 tenant payload，并回源 PostgreSQL 做租户校验。
7. PostgreSQL 自建恢复、Qdrant cluster / rebuild、Neo4j 生产必选恢复 runbook 和至少一次恢复演练证据齐备。
8. 告警暂不纳入当前门禁；正式生产发布前需重新评估 DB/Qdrant/Neo4j 不可用、backup age、restore failure、outbox backlog、reconciliation drift、tenant isolation violation 等告警范围。
9. 发布前存在 migration dry-run、rollback/forward-fix 策略、release checklist 和 launch acceptance 证据。
10. P0 测试矩阵全部通过前，不声明企业高可靠生产放行。

## 用户故事 / 运维场景

### US-1: 数据库级租户隔离

**作为** 企业平台管理员，
**我想要** 记忆存储在数据库层也强制绑定租户，
**以便** 即使某个应用路径遗漏过滤条件，也不会跨租户读取或修改数据。

**验收标准：**

- memory-owned 表具备 `tenant_id` 字段或明确兼容迁移阶段说明。
- 所有关键查询优先使用 `(tenant_id, id)` 或 `(tenant_id, business_key)`。
- RLS 或等价策略在缺失租户上下文时拒绝访问。
- 跨租户 update/history/time-travel negative tests 通过。

### US-2: 可恢复的向量索引一致性

**作为** 后端工程师，
**我想要** LTM 事实写入和 Qdrant 向量索引同步通过 durable outbox 解耦，
**以便** Qdrant 写失败、worker 崩溃或重复重放时仍可恢复。

**验收标准：**

- PostgreSQL transaction 内写入 LTM 事实和 outbox event。
- worker 使用幂等 key upsert Qdrant。
- outbox 支持 pending / processing / applied / failed / dead-letter 状态。
- reconciliation job 能发现并修复 DB 与 Qdrant 漂移。

### US-3: 多步写入事务一致性

**作为** QA 工程师，
**我想要** STM、KG 等多步写入在任一步失败时完整回滚，
**以便** 不会留下半写 message、relation 或错误计数。

**验收标准：**

- STM message append 与 session context update 在同一事务内。
- STM delete session 相关子记录删除具备事务边界。
- KG relation 插入和 relation_count 更新在同一事务内。
- 故障注入测试证明失败时无半成品。

### US-4: 可执行恢复演练

**作为** DevOps 工程师，
**我想要** PostgreSQL 自建恢复、Qdrant cluster / rebuild、Neo4j 生产必选恢复步骤、验证命令和演练记录，
**以便** 生产事故时可以按 runbook 恢复服务和数据。

**验收标准：**

- PostgreSQL PITR 或备份恢复流程可执行。
- Qdrant snapshot 或从 PostgreSQL + outbox 重建索引流程可执行。
- Neo4j backup / dump / restore 流程可执行。
- 每次演练记录时间、环境、输入、结果、失败项和残余风险。

### US-5: 发布前可靠性准入

**作为** 技术负责人，
**我想要** 发布前看到测试、迁移、恢复、告警、回滚证据，
**以便** 判断是否允许进入企业内部生产环境。

**验收标准：**

- `test-plan.md` 的 P0 测试和 operational drills 有执行证据。
- release checklist 明确 rollback / forward-fix 策略。
- unresolved Critical / High 风险必须阻塞放行或升级给 tech-lead。

## 范围

### In Scope

- STM / LTM / KG / MM 主路径的显式租户隔离设计。
- PostgreSQL `tenant_id`、tenant-scoped indexes/constraints、RLS 或等价策略。
- repository / service / router 层 TenantId 强制传递。
- LTM 与 Qdrant outbox、worker、幂等重放、reconciliation 设计。
- STM / KG / LTM 关键多步写事务化。
- 持久化审计事件范围定义。
- PostgreSQL / Qdrant / Neo4j backup restore runbook。
- HA 拓扑说明、告警规则范围和发布回滚准入。
- 单元、集成、E2E、operational drill 测试计划。

### Out of Scope

- 不替换 PostgreSQL、Qdrant 或 Neo4j。
- 不承诺跨地域 active-active 或零 RPO 架构。
- 不重写全部 memory API 或 SDK。
- 不处理非 memory storage 的 planner、runtime、billing、workflow 可靠性。
- 不把现有 `docker-compose.yml` 升级为完整生产部署系统；生产拓扑可以通过文档或独立部署配置承接。
- 不在本 PRD 中定义具体供应商托管服务采购方案。

## 技术约束

- 保留现有 Rust / Axum / SQLx / PostgreSQL / Qdrant / Neo4j 技术栈。
- 兼容现有 `t:{tenant}` prefix 数据，迁移期允许 dual-write / dual-read，但不能继续把 prefix 作为长期主安全边界。
- Qdrant 与 PostgreSQL 不追求同步强一致，采用可恢复的 eventual consistency。
- migration 必须采用 expand-contract 思路，避免不可逆一次性切换。
- 企业生产准入需要同时覆盖代码、数据、部署、观测和演练证据。

## 风险与依赖

| 风险 | 影响 | 缓解措施 | Owner |
|------|------|----------|-------|
| 历史数据无法可靠反推 tenant | backfill 不完整，可能阻塞 RLS enforce | 先产出数据归属报告；无法归属数据进入只读隔离 | backend-engineer |
| RLS context 与连接池结合错误 | 误拒绝或潜在泄漏 | 使用 transaction-local setting；增加缺失 context negative tests | backend-engineer |
| outbox 带来搜索可见性延迟 | 新写入 LTM 可能短暂不可搜索 | PRD/API 文档说明 eventual consistency；暴露 outbox lag 指标 | architect |
| Qdrant rebuild 成本高 | 恢复时间超过 RTO | 建立 snapshot + incremental reconciliation 双路径 | devops-engineer |
| 生产 HA 依赖部署环境 | 代码仓无法单独证明 HA | 在 deployment-context / release-plan 中记录托管服务或集群配置证据 | devops-engineer |
| 告警无人接收 | 告警闭环失效 | 明确 pager routing、owner 和升级策略 | tech-lead |

## 待确认项

- [x] 企业内部目标应用等级：不涉及 T1 / T2 / T3 / T4 定级；本轮按项目内部可靠性整改推进，不挂企业应用等级门禁。
- [x] 目标 RPO / RTO：暂不定义传统 RPO/RTO；性能与容量基线按默认支持 50 并发请求设计和验证。
- [x] PostgreSQL 是否使用托管 HA 服务，还是自建 Patroni/repmgr/Stolon：采用自建方案，具体 HA 组件在 deployment-context / release-plan 中细化。
- [x] Qdrant 是否启用 cluster mode，还是先采用 snapshot + rebuild：启用 Qdrant cluster 方向，同时保留 DB + outbox/reconciliation rebuild 作为恢复路径。
- [x] Neo4j 是否为生产必选，是否需要 causal cluster：Neo4j 为生产必选；需纳入 backup / restore / cluster 或等价高可用设计。
- [x] 告警接收渠道和 on-call owner：告警暂时不考虑，不作为当前 handoff-ready 前置门禁；后续如进入正式生产发布需重新评估。
- [x] 历史 prefix 数据的保留、归属和 quarantine 策略：无法归属的数据进入只读隔离，不做删除或猜测归属。
- [ ] 企业审计日志保留周期和敏感字段脱敏规则。
- [x] outbox 带来的写后搜索延迟：可以接受 eventual consistency。
- [x] read-after-write fallback：P0 支持按 ID 回源读取兜底，向量搜索仍允许短暂 pending。
