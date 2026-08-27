# ADR-0010: 记忆语义生命周期与物理存储正交模型（L0-L3）

## 决策信息

| 字段 | 值 |
|------|-----|
| 编号 | ADR-0010 |
| 标题 | 记忆语义生命周期与物理存储正交模型（L0-L3） |
| 状态 | Accepted（2026-08-27 修订，#125；取代 2026-08-25 初版） |
| 日期 | 2026-08-27（初版 2026-08-25） |
| Owner | tech-lead |
| 关联 Issue | #124（Epic）、#125（本次修订）、#88（初版） |
| 关联 ADR | [ADR-0011](ADR-0011-belief-lifecycle-state-machine.md)（信念状态机与谓词目录） |

## 背景与约束

### 初版的问题：三套 L0-L3 并存

初版 ADR-0010（2026-08-25，#88）把语义分层定义为 Events / Facts / Persona / Scenario，
但同仓库还存在另外两套编号：

1. **distillation 子系统**的本地编号：L1 = atoms（`distillation_atoms`）、L2 = scenes
   （`distillation_scenes`）、L3 = personas（`distillation_personas`），并被
   `distillation_jobs.job_type` 的 CHECK 约束（`l0_to_l1/l1_to_l2/l2_to_l3`）固化；
2. **Epic #124** 用 Event / Belief / Persona-Scenario / Procedure 描述语义生命周期，
   并明确"工作记忆（RAM）/情节/语义信念/程序记忆"的四层心智模型。

三套术语指称交叠但边界不同（例如初版 L1 叫 Facts，Epic 叫 Belief；初版 L3 是
Scenario，Epic 的 Procedure 在初版里却是物理层），新实现（#126-#130）无从引用
单一事实源。本修订（#125）收口为**一套** canonical 术语。

### 目标

- 两个正交维度各归其位：**物理存储**描述介质，**语义生命周期**描述"这条记忆
  在认知流水线里是什么"，二者映射但不耦合。
- Working Memory 明确定义为运行时组装视图，堵住"再加一个持久化层"的滑坡。
- 与 Epic #124 的术语一一对应，后续 PR（#126-#130）只引用本 ADR。
- 数据库迁移基线收口：PostgreSQL 与 SQLite 各自持有单一方言的迁移集。

### 约束

- 不替换 STM/LTM/KG/MM 的物理结构；语义层叠加在现有存储之上。
- 不改存量外部 API、表名与 DB CHECK 枚举值（向后兼容）。
- 冲突更新不直接覆盖历史版本（双时态或 append-only）。
- Persona/Scenario 的每条结论均可追溯到 L0/L1 证据。

### 非目标

- 不定义信念写入/召回的实现（#127、#128）。
- 不引入 Graphiti/Zep 等外部框架（Epic #124 明确政策、身份、权威源自建）。
- 不覆盖 Skill/Wiki/CodeGraph 资产模型（#90）、Agent Loadout/ACL（#89）。

## 方案

### 维度一：物理存储（存储与检索介质）

| 层 | 职责 | 存储介质 |
|----|------|---------|
| STM | 会话级短期记忆 | PostgreSQL（context_sessions + session_messages） |
| LTM | 持久化长期记忆 | PostgreSQL（knowledge_entries 等）+ Qdrant 向量索引 |
| KG | 结构化知识图谱 | PostgreSQL + Neo4j（可选） |
| Qdrant | LTM 的向量倒排索引，**不是主存储** | Qdrant |
| MM | 多模态记忆 | PostgreSQL |

说明：

- **Qdrant 是索引，不是主存储**：信念图的真身在 PG/KG，向量只承担"图邻域 +
  关键词 + 向量"混合检索中的一路（Epic #124："向量索引只是图的倒排"）。
- **L3 Procedure 的物理基座**是 PG 内的过程性表（skills 等）。存量代码中的
  `LayerType::Procedural` 枚举值保留为存储类型（兼容），语义上对应本 ADR的 L3。

### 维度二：语义生命周期（L0-L3）

```
┌─────────────────────────────────────────────────────────┐
│ L3: Procedure 程序记忆 —— 怎么干活                          │
│   路由规则、成功/失败剧本；变更必须评审，                     │
│   禁止被对话或网页写入                                      │
├─────────────────────────────────────────────────────────┤
│ L2: Persona-Scenario 画像与情境                             │
│   人是谁（画像）＋ 长期在做什么（情境）；                     │
│   每条结论带置信度与证据指针，可被修正                        │
├─────────────────────────────────────────────────────────┤
│ L1: Belief 语义信念 —— 现在什么是真的                        │
│   双时态图：valid_from/valid_to + recorded_at；             │
│   取代不覆盖（supersedes 链）；来源/信任/风险受谓词目录治理     │
├─────────────────────────────────────────────────────────┤
│ L0: Event 情节事件 —— 发生过什么                            │
│   不可变，append-only；带时间、参与者、结果                   │
└─────────────────────────────────────────────────────────┘
        ↑ 稳定性梯度：L0 每轮产生 → L1 随新证据变化
          → L2 慢聚合 → L3 评审后变更
```

与 Epic #124 的对应：L0 = 情节记忆、L1 = 语义信念、L2 = 画像与情境、
L3 = 程序记忆。

#### Working Memory：运行时组装视图，不是持久化层

Working Memory（工作记忆，Epic #124 的"RAM"）**不属于 L0-L3 中的任何一层**，
也永远不是新的持久化层。它是每轮推理前的组装结果：

```
Working Memory = 近 N 轮对话
               + 受控召回的 5-10 条现行信念（as_of=now，已过权限与信任过滤）
               + 本轮工具草稿
```

推论：工作记忆不膨胀的治理手段是控制组装预算与召回条数（#128），而不是
新增存储；"把全历史摘要当长期记忆灌进上下文"是被明确禁止的反模式。

### 各层定义

#### L0: Event（情节事件）

- **来源**：用户消息、Agent 回复、工具调用结果、系统事件
- **形态**：`{ id, session_id, source, raw_content, metadata, timestamp }`
- **生命周期**：不可变、append-only；按 session 归档；按 retention policy 清理
- **存储**：STM（热）+ 归档（冷）
- **召回**：按 session 顺序读取；检索权重随时间衰减

#### L1: Belief（语义信念）

- **来源**：从 L0 抽取（门卫 + 实体对齐后）或 SoR 对账写入
- **形态**：带时间的命题 `{ subject, predicate, object, valid_from, valid_to,
  recorded_at, source, trust, provenance, scope, supersedes, status }`
- **生命周期**（详见 [ADR-0011](ADR-0011-belief-lifecycle-state-machine.md)
  与 `backend/src/models/belief.rs`）：
  - 双时态：`valid_*` 是世界上何时为真；`recorded_at` 是系统何时知道
  - 取代不覆盖：新证据到达时旧边 `valid_to` 闭合、`supersedes` 指向新边
  - 状态机：`quarantined / candidate / active / needs_confirm / stale /
    superseded / archived / rejected`；只有 `active` 进入默认检索面
  - 每个谓词受谓词目录治理（cardinality / mutability / 来源 / TTL / 风险）
- **存储**：LTM（结构化）+ KG（三元组）+ Qdrant（向量倒排）
- **召回**：`as_of=now()` 取现行边；显式 `as_of=<t>` 回溯历史
- **删除**：软删除 + 归档，保留审计轨迹

> 术语变更说明：初版的 "L1 Facts" 更名为 **Belief**。"Fact" 暗示客观真值，
> 与来源分级、可取代、需确认的治理语义直接冲突——一条 belief 可能来自网页、
> 可能过期、可能被人否决；它始终是"被系统相信的命题"，不是事实本身。

#### L2: Persona-Scenario（画像与情境）

- **来源**：从 L1 聚合 + LLM 生成
- **形态**：
  - Persona（画像）：`{ user_id, trait_type, trait_value, confidence,
    evidence_ids, version }`，trait_type 如 preference / constraint /
    expertise / communication_style / role
  - Scenario（情境）：`{ user_id, scenario_type, summary, goals, constraints,
    participants, evidence_ids, status }`，scenario_type 如 project /
    support_ticket / learning_path
- **生命周期**：定期从 L1 聚合生成快照；新 L1 事实触发增量更新；用户显式
  反馈 > 自动推断；`status: closed` 后保留为历史情境
- **置信度**：每条结论附 confidence（0.0-1.0），基于证据数量与质量、用户
  显式确认/否定、时间衰减
- **存储**：LTM（结构化 JSON）+ KG（节点属性）
- **删除**：用户可查看、修正、删除特定 trait 或完整画像（GDPR）

#### L3: Procedure（程序记忆）

- **来源**：反复成功的情节经人工评审升级；或管理员显式创建
- **形态**：`skills` 表（trigger_conditions / execution_steps /
  validation_rules / status: draft|active|deprecated）
- **生命周期**：**变更必须评审**——对话或网页输入永远不能直接改写程序记忆
  （写入门卫在 #127 强制这一点）；废弃走 `deprecated` 状态
- **存储**：PG（skills 表）

### 语义层 ↔ 物理层映射

| 语义层 | 主存储层 | 辅助 | 索引 |
|--------|---------|------|------|
| L0 Event | STM | 归档 | session_id, timestamp |
| L1 Belief | LTM（结构化）+ KG（三元组） | Qdrant（向量倒排） | subject, predicate, embedding |
| L2 Persona-Scenario | LTM（结构化 JSON） | KG | user_id, scenario_type, status |
| L3 Procedure | PG skills 表 | - | tenant_id, status |

### 与 distillation 子系统的映射（消除两套编号）

存量 distillation 实现使用本地 L1/L2/L3 编号（atoms/scenes/personas）。canonical
映射如下，**后续代码与文档一律使用本 ADR 的术语**：

| distillation 实现 | canonical 语义层 |
|-------------------|------------------|
| `context_sessions` / `session_messages`、episodic atoms | L0 Event |
| 结构化 atoms（`distillation_atoms`，persona/instruction 抽取产物） | L1 Belief 的候选命题（#127 引入 belief 表后收口为 canonical 存储） |
| `distillation_scenes` | L2 Scenario |
| `distillation_personas` | L2 Persona |
| `skills` 表、instruction atoms | L3 Procedure |

`distillation_jobs.job_type` 的 `l0_to_l1 / l1_to_l2 / l2_to_l3` 是存量 DB CHECK
枚举值，**保留不改**（向后兼容），其语义分别是 Event→Belief 提炼、
Belief→Scenario 巩固、Scenario→Persona 聚合。

### 数据流

```
写入（同步段要短，巩固异步）
  User Message -> L0 Event（append-only）
       ↓ 门卫（来源/指令检测/信任分，低于阈值进隔离区）
  抽取 + 实体对齐（主体挂 principal，#126）
       ↓ 与现行边比较：ADD | SUPERSEDE | NOOP | CONFLICT
  L1 Belief（高风险先 needs_confirm）
       ↓ 聚合
  L2 Persona-Scenario
       ↓ 人工评审升级
  L3 Procedure

读取（每轮）
  解析本轮 principal -> 权限裁剪 -> 混合检索（as_of=now）
       -> 信任过滤 -> 5-10 条现行信念
       -> Working Memory = 近 N 轮 + 这 5-10 条 + 工具草稿

巩固（离线，#129）
  合并重复 / 闭合被取代边 / SoR 对账 / stale 扫描 / 情节升级为程序记忆（人审）
```

### 数据库迁移基线（#125 收口）

- `backend/migrations/` 只允许 PostgreSQL 方言；`backend/migrations_sqlite/`
  只允许 SQLite 方言。**不存在跨方言回退**：SQLite 后端在
  `migrations_sqlite/` 缺失时直接报错，不再静默回退到 PG 迁移
  （`db/mod.rs`）。
- 迁移文件名的版本前缀统一为 **14 位时间戳**。曾存在的
  `20260813_distillation_tables.sql`（8 位前缀）导致 sqlx 数值排序错位，
  且为 SQLite 方言、与 PG 版 `20260813000001_distillation_pipeline.sql`
  重复定义 `skills`/`agent_equipment`——已删除（其表在 Rust 代码中零引用）。
- 防漂移测试 `backend/tests/migration_drift.rs` 强制：两种后端各自从空库
  跑通全量迁移、目录内无方言污染、无重复表定义、文件名版本格式一致。

## 备选方案

### 方案 A：沿用初版 Events/Facts/Persona/Scenario 命名

- 优点：不动文档存量引用
- 缺点："Facts" 与来源分级/可取代语义冲突；与 Epic #124 术语不一致，
  两套定义的问题原样保留
- 不选原因：本 ADR 的存在意义就是消除多套定义

### 方案 B：引入独立的 Persona/Scenario 服务（微服务拆分）

- 优点：独立部署、独立扩展
- 缺点：增加运维复杂度、跨服务事务，与现有单体架构不一致
- 不选原因：当前阶段不适合引入新的微服务

### 方案 C：正交双维度 + 单一 canonical 术语（本方案）

- 优点：最小架构变更；与 Epic #124 一一对应；存量表/API 零改动
- 缺点：distillation 存量编号与 canonical 编号的映射需要文档维护（见上表）
- 选择原因：满足所有需求，且为 #126-#130 提供单一事实源

## 决策结果

**采用方案 C。**

影响：
- `kernel::types::SemanticLayer` 文档注释已同步为 canonical 术语（枚举值
  L0-L3 不变，序列化兼容）
- `models/belief.rs` 是 L1 语义（来源/状态/写入决策/谓词目录）的代码级
  单一事实源；ADR-0011 是其 prose 契约
- 存量 API、表名、DB CHECK 枚举值不变；向后兼容

## 企业内控补充

- 应用等级：T3（内部系统，非主营主链路）
- 技术架构等级：标准
- 关键组件：复用现有 PostgreSQL + Qdrant + Neo4j
- 资产文档入口：本 ADR + `docs/artifacts/l0-l3-taxonomy/`

## 后续动作

- [x] `kernel/types.rs` `SemanticLayer` 文档同步（#125）
- [x] `models/belief.rs`：来源/状态枚举、状态机、写入决策、谓词目录（#125）
- [x] 迁移基线收口 + 双后端防漂移测试（#125）
- [ ] #126：Event 流与 Principal 身份图表落地
- [ ] #127：Belief 表 + 写入门卫（状态机落地，CHECK 与枚举 anti-drift）
- [ ] #128：受控召回与 Working Memory 组装（只取 `active`，`as_of=now`）
- [ ] #129：巩固作业（stale 扫描、SoR 对账、情节升级）
- [ ] #130：治理 API 与黄金验收测试
