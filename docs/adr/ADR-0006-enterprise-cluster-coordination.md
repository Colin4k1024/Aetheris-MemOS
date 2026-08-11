# ADR-0006: 企业集群协调采用成熟协调原语（非自建共识）

## 决策信息

- 编号：ADR-0006
- 决策标题：企业集群能力（leader 选举 / 集群成员 / 记忆分片路由 / 许可分级）用**成熟协调原语**实现真实版，**不自建 Raft/Paxos**；协调状态落 PostgreSQL，多节点选主用 PG advisory lock（可演进到 etcd/Consul）
- 状态：Proposed
- 实现状态：未落地 —— 现状仍是 `services/enterprise.rs` 内存假 Raft（`become_leader` 自增内存 term）+ 假分片；无 PG advisory lock、无 `cluster_nodes`/`memory_shards`/`tenant_licenses` 迁移；前置条件未满足前不得推进。
- 日期：2026-07-16（提出）；2026-08-11（按实现核实确认仍未落地）
- Owner：architect
- 收口责任人：待 tech-lead 主持 Design Review Board 收口（后续动作首项，仍 open）
- 关联：`delivery-plan.md`；`ADR-0005-ha-infrastructure-selection.md`（数据存储 HA 走托管，不自建）；用户决策「必须实现真实的 enterprise」+「成熟协调原语」
- 取代：`services/enterprise.rs` 的内存假 Raft/假分片（P0 曾建议删除，用户决定改为做真）

## 实现核实与前置条件（2026-08-11 收口）

按代码核实，本 ADR 主张的 PG advisory lock 选主 + `cluster_nodes` 成员表 + 一致性哈希分片 + `tenant_licenses` 许可**一项都未落地**，因此保持 `Proposed`。**现状仍是本 ADR 结论先行第 1 点自述的「假集群」。**

**核实证据（现状 = 未落地）：**

- **仍是内存假 Raft**：`backend/src/services/enterprise.rs:48-143` 的 `ClusterManager` 用 `Arc<RwLock<HashMap>>` 存节点，`become_leader`（`:115-143`）直接 `leader.term += 1` 自增内存 term——正是本 ADR 要取代的假实现。`EnterpriseShardManager`（`:202+`）同为内存 HashMap 假分片。
- **无 PG advisory lock**：`rg "advisory_lock|pg_try_advisory" src/` **零命中**。
- **无迁移**：`rg "cluster_nodes|memory_shards|tenant_licenses" migrations/` **零命中**——三张表都不存在。
- **端点仍读内存假状态**：`backend/src/routers/enterprise.rs:17-27` 用 `OnceLock<ClusterManager>` + `ClusterManager::new("node_1")` 硬编码单节点，`become_leader`（`:94-97`）仍走假实现。

**前置条件 / 阻塞原因（对应 backlog；本 ADR 结论先行第 6 点自陈）：**

1. **需真实 PG + 多实例环境**：advisory lock / 心跳 / 迁移 / 选主接管 / 分片 re-balance 都需真 PG 与多实例集成测试，离线单会话不可完成。
2. **依赖 ADR-0001 RLS 基线**：`tenant_licenses` 等表需随 P1 RLS 基线加租户隔离。
3. **许可门控依赖治理 hooks 接线**：LicenseTier 门控 + 配额需接入治理 hooks（与 backlog C-1 治理层覆盖、C-2 配额生效联动）。
4. **Design Review Board 尚未收口本 ADR**（后续动作首项仍 open）。

> 诚实收口口径：`enterprise.rs` 假集群挂在 9 个活端点并对外宣传，属「文档/端点声称 > 实现」。在本 ADR 落地前，这些端点返回的是单进程内存玩具结果——保持 `Proposed` 并显式标注「尚未落地」，避免误判为已采纳。

## 结论先行

1. **现状是假集群**：`services/enterprise.rs` 的 `ClusterManager`（内存 HashMap 假 Raft：`become_leader` 自增 term）+ `EnterpriseShardManager`（内存 HashMap 假分片），挂在 `/api/v1/memory/enterprise/cluster|shards` 9 个活端点、并在公开 API 文档对外宣传——单进程内存玩具，企业买家调用即得到失实结果。
2. **做真，但不自建共识**：应用层多实例协调（选主/成员/分片路由）是**独立于数据存储 HA** 的关注点，可用**现成协调原语**做到真实，无需手写 Raft——与 ADR-0005「不自建」一致。
3. **选主 = PG advisory lock**：`pg_try_advisory_lock(key)` 提供真实单主语义，持有者断连自动释放（天然故障转移），**零新增基建**（复用已有 PG）。规模/跨区需求上来后可演进到 etcd/Consul（本 ADR 留演进位）。
4. **成员 = `cluster_nodes` 表 + 心跳**；**分片 = 一致性哈希 over 活节点 + 持久化 owner 映射**（应用层路由/归属，非数据物理分区——数据仍在共享托管存储）。
5. **许可分级 = `tenant_licenses` 真实记录 + 在治理 hooks 强制**（LicenseTier 门控功能 + 配额）。
6. **多周工作流、需真 PG**：本 ADR 为设计；落码与验证需真实 PG（advisory lock / 心跳 / 迁移）+ 多实例环境，离线不可完成。

## 背景与约束

- 用户定位：企业级可售卖；「必须实现真实的 enterprise」；实现路径选定「成熟协调原语」。
- ADR-0005 已定：数据存储（PG/Qdrant/Neo4j）HA 走托管基建、不自建 Raft。本 ADR 处理的是**应用节点层协调**，属不同层次，但同样遵循「不手写共识」红线。
- 现有资产：应用已强依赖 PostgreSQL（可直接用 advisory lock + 表）；`LicenseTier`（Free/Starter/Pro/Enterprise）枚举已存在于 `hoops/enterprise.rs`；治理 hooks（RBAC/配额/审计）P1 已有接入计划（见 outbox-and-governance-plan）。
- 约束：跨平台、不引入未经评审的重基建；应用层分片不得与"数据在共享托管存储"矛盾（分片是**协调/归属/路由**层面，不是把数据切到不同库）。

### 非目标
- 不自建共识/复制/分片算法（Raft/Paxos）。
- 不做数据存储层的分区（数据 HA/扩展归托管基建，ADR-0005）。
- 不在本 ADR 定义治理 hooks 的具体接线（见 P1 outbox-and-governance-plan 子项 b）。

## 备选方案

| 方案 | 选主机制 | 优点 | 风险/成本 | 取舍 |
|---|---|---|---|---|
| ① PG advisory lock + 表（**推荐 P1**）| `pg_try_advisory_lock` | 零新增基建（复用 PG）；断连自动释放=天然 failover；事务/心跳同库好实现 | 绑定 PG 可用性（但 PG 本就是核心依赖）；单 PG 实例是协调单点（由 ADR-0005 的 PG HA 兜底）| **采用**为 P1 起点 |
| ② etcd / Consul | lease + 选举 API | 专业协调、watch、跨服务复用 | 新增基建 + 运维；当前无编排底座 | **留演进位**：规模/跨区/多服务协调需求出现时引入 |
| ③ Redis（SETNX + lease）| 分布式锁 | 轻量、已有 Redis 可选依赖 | 锁正确性边界（需 Redlock 谨慎）；比 PG lock 弱 | 备选，不优先 |
| ④ 自建 Raft | 手写共识 | 功能最"纯" | 巨大工作量、高风险、违反 ADR-0005 | **不选**（用户已选成熟原语）|

## 决策结果

### 采用：PG advisory lock 选主 + `cluster_nodes` 成员表 + 一致性哈希分片路由 + `tenant_licenses` 许可

**1. Leader 选举 / 集群成员**
- 每个实例启动时在 `cluster_nodes` 注册（`node_id`(ULID)、`role`、`endpoint`、`started_at`、`last_heartbeat`、`status`）；后台心跳定期刷新 `last_heartbeat`。
- "active nodes" = `last_heartbeat > now() - ttl`（如 30s）；过期视为下线。
- 选主：候选实例尝试 `pg_try_advisory_lock(LEADER_LOCK_KEY)`；成功者为 leader，持锁期间周期确认；进程崩溃/断连 → 会话结束 → 锁自动释放 → 其他实例接管。`become_leader` 端点语义改为"尝试竞选"（真实），不再自增内存 term。
- `/enterprise/cluster/{nodes,active,leader,is-leader}` 全部改读真实表/锁状态。

**2. 记忆分片路由**
- 一致性哈希环 over 活节点（来自 `cluster_nodes`）；shard key（如 tenant_id 或 memory key）→ 归属节点。
- owner 映射持久化到 `memory_shards` 表（`shard_key`、`owner_node_id`、`version`），节点增减时 re-balance（迁移最少 key）。
- **重要边界**：数据仍在共享托管 PG/Qdrant；分片是**协调/归属/本地缓存亲和/工作路由**层面（哪个节点负责某 shard 的调度/缓存），不是把数据切到不同物理库。文档需讲清，避免"分片=数据分区"的误解。
- `/enterprise/shards/*` 改为真实的 owner 查询/分配。

**3. 许可 / 套餐分级**
- `tenant_licenses` 表（`tenant_id`、`tier`(LicenseTier)、`limits_json`、`valid_until`）；启动/请求时加载。
- 在治理 hooks（P1，ADR/plan 已定）中按 `tier` 强制：功能门控（如 enterprise-only 端点）+ 配额上限（对齐 `ResourceQuota`）。
- 与配额真生效（outbox-and-governance-plan 子项 b 的 `used` increment）联动。

### 影响范围
- `backend/src/services/enterprise.rs`（`ClusterManager`/`EnterpriseShardManager` 从内存假实现改为 PG-backed 真实现）
- `backend/src/routers/enterprise.rs`（端点语义改为真实竞选/查询/分配；**路由保留、不删**）
- 新增迁移：`cluster_nodes`、`memory_shards`、`tenant_licenses` 表
- `hoops/enterprise.rs`（LicenseTier 门控接入治理 hooks）
- 后台守护：心跳刷新 + leader 续约 + 分片 re-balance（生命周期纳入 main.rs，优雅停机时注销节点/释放锁）
- API 文档（`API_ENDPOINTS.md` / `API_USAGE_GUIDE.md` / gh-pages）：从"假集群"更新为真实语义

### 兼容性 / 迁移影响
- 端点路径不变（`/api/v1/memory/enterprise/cluster|shards`），语义从"内存玩具"变为"真实协调"——对外契约形状兼容、行为变真。
- 单实例部署：advisory lock 恒成功=自己是 leader；分片环只有一个节点=全部归自己——单节点优雅退化，无需多实例即可正确运行。
- SQLite（dev）：advisory lock 无对应物→降级为"永远是 leader、单节点"；仅 PG 生产走真协调。

### 失败 / 回退
- PG 不可用 → 选主/心跳失效：由 ADR-0005 的 PG HA 兜底；协调层在 PG 恢复前进入"保持上一已知 leader / 只读"降级，不假装成功。
- 分片 re-balance 抖动：加最小迁移 + 冷却窗口；异常时回退到"全部归当前 leader"。
- 若 PG advisory lock 在规模上成为瓶颈 → 按备选②演进到 etcd/Consul（本 ADR 已留位）。

## 企业内控补充
- 应用等级：涉及多实例协调与企业售卖，按 T2/T3 高可用口径；协调单点由 PG HA（ADR-0005）覆盖。
- 技术架构等级：选主/成员/分片/许可均须纳入监控告警（leader 变更、节点下线、分片迁移、许可越限）与资产文档。
- 关键组件偏离：优先复用 PG（集团认可）；若引入 etcd/Consul 需在此登记原因、运维归属、退场路径。
- 资产文档入口：本 ADR、delivery-plan、后续 enterprise 集群测试计划与 deployment-context。

## 后续动作

| 动作 | Owner | 完成条件 |
|---|---|---|
| Design Review Board 收口本 ADR（含与 ADR-0005 边界确认）| tech-lead / architect | Proposed→Accepted |
| 迁移：`cluster_nodes` / `memory_shards` / `tenant_licenses` | backend + architect | 迁移评审通过，Testcontainers 验证 |
| ClusterManager 改 PG advisory lock 选主 + 心跳守护 | backend | 多实例集成测试：杀 leader→他人接管；节点下线→active 更新 |
| EnterpriseShardManager 改一致性哈希 + 持久 owner + re-balance | backend | 节点增减 re-balance 正确、最小迁移 |
| LicenseTier 门控 + 配额接入治理 hooks | backend | tier 门控 + 越限拒绝负向测试通过 |
| 端点语义改真 + 更新对外 API 文档 | backend | 端点返回真实状态；文档无"假集群"表述 |
| 多实例 + 真 PG 的集成测试（选主/分片/故障转移）| qa | 放行标准证据齐全 |
