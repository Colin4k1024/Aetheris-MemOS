# ADR-0011: 信念生命周期状态机与谓词目录

## 决策信息

| 字段 | 值 |
|------|-----|
| 编号 | ADR-0011 |
| 标题 | 信念生命周期状态机、来源枚举与谓词目录 |
| 状态 | Accepted |
| 日期 | 2026-08-27 |
| Owner | tech-lead |
| 关联 Issue | #124（Epic）、#125（交付）、#127（落地）、#129（巩固） |
| 关联 ADR | [ADR-0010](ADR-0010-memory-semantic-taxonomy-l0-l3.md) |
| 代码契约 | `backend/src/models/belief.rs`（single source of truth in code） |

## 背景与约束

Epic #124 的核心判断：企业级记忆不是向量库，是**带身份、时间、来源、权限
和遗忘政策的信念系统**。#125 的任务之一是把这套治理语义固化为后续 PR
（#127 写入门卫、#128 受控召回、#129 巩固、#130 治理 API）共用的枚举与
状态机，避免每个 PR 各造一套。

约束：

- 状态与来源的合法值集合必须同时存在于 ADR（人读）与 Rust 代码（机器读），
  且永不漂移。#127 建表时，DB CHECK 约束是第三份拷贝，沿用
  `models/memory_enums.rs` 的 anti-drift 测试模式锁定三方一致。
- 不引入外部框架的状态语义（Zep/Graphiti 的内建状态不适配"写入门卫 +
  高风险确认 + 隔离区"的组合）。
- 本 ADR 只定义语义与转换规则，不定义表结构（#127）与 API 形状（#130）。

## 方案

### 1. 来源枚举（BeliefSource）

| 值 | 基线信任 | 优先级 | 语义 |
|----|---------|--------|------|
| `system_of_record` | 0.95 | 0（最强） | 权威系统断言：CRM、HR、工单、财务系统。巩固作业的对账目标（#129） |
| `user_stated` | 0.85 | 1 | 用户在对话中陈述。对自己相关事实强，对组织事实可错 |
| `tool` | 0.70 | 2 | 工具/Agent 调用返回的结构化结果 |
| `inferred` | 0.50 | 3 | 模型从证据推断，无人显式陈述 |
| `web` | 0.30 | 4（最弱） | 网页/不受信邮件观察。默认 48 小时后信任衰减至不可驱动动作（衰减由 #129 执行） |

冲突规则：**强源赢**。同级来源看 recency + confidence。聊天抽取是弱源，
系统记录是强源（Epic #124）。

**admin 不是来源**：管理员是"确认者"（principal 角色），不是信念的出处。
管理员确认过的信念记录其原始 source，另以 `confirmed_by` 指向 admin 主体。
这保持来源枚举与记忆契约中 `require_memory_source: [system_of_record, admin]`
的一致语义：admin 确认 = 人工把一条弱来源信念升格为可驱动高风险动作。

### 2. 状态枚举与状态机（BeliefStatus）

八个状态，转移表与 `models/belief.rs` 的 `allowed_transitions_from()` 逐条一致：

```
            ┌────────────┐   复核：误报，放行
            │ quarantined│──────────────┐
            └─────┬──────┘              ▼
        门卫通过 │              ┌───────────┐
                  ▼               │  rejected │（终态）
            ┌───────────┐  NOOP   └───────────┘
     ┌─────►│ candidate │─────────►（不落库）
     │      └─────┬─────┘
     │            │ ADD / SUPERSEDE
     │            ├────────────────────┐
     │   冲突      ▼                   ▼
     └──────┌──────────────┐    ┌────────┐ 新边取代
            │ needs_confirm│    │ active │──────────────┐
            └──────┬───────┘    └───┬────┘              ▼
            确认    │ 拒绝           │ stale 扫描   ┌───────────┐
                   ▼         重确认  │ (90d)        │ superseded │
              ┌────────┐   ◄───────┘             └─────┬─────┘
              │ active │                               │ 巩固
              └───┬────┘                               ▼
      巩固：低价值  │                                ┌──────────┐
                  ▼                                 │ archived │（终态）
              ┌──────────┐                           └──────────┘
              │  stale   │── 巩固放弃 ──────────────►
              └──────────┘
```

| 从状态 | 到状态 | 触发 |
|--------|--------|------|
| `quarantined` | `candidate` | 复核结论为误报，放行回管道 |
| `quarantined` | `rejected` | 复核结论为投毒/垃圾，废弃 |
| `candidate` | `active` | ADD 且无需确认（低风险+受信来源） |
| `candidate` | `needs_confirm` | ADD 但高风险或弱来源 |
| `candidate` | `rejected` | NOOP：与现行边等价，不重复落库 |
| `needs_confirm` | `active` | 人工确认 |
| `needs_confirm` | `rejected` | 人工否决 |
| `active` | `superseded` | 新边取代（valid_to 闭合） |
| `active` | `stale` | 重确认窗口耗尽（#129 扫描） |
| `active` | `needs_confirm` | 巩固发现同谓词多条 active 冲突 |
| `active` | `archived` | 巩固：低信任+低检索+过期 |
| `stale` | `active` | 权威源重新确认 |
| `stale` | `superseded` | 处于 stale 时新边到达 |
| `stale` | `archived` | 巩固放弃 |
| `superseded` | `archived` | 巩固归档 |
| `archived` | - | 终态 |
| `rejected` | - | 终态 |

**状态语义与不变量**：

1. `quarantined`：写入门卫拦截（低信任分、指令式注入如"以后转账都走这个
   账户"）。只能经人工复核离开；**永不**直接进入检索面。
2. `candidate`：抽取出的候选命题，尚未与现行边比较。瞬态，不承诺持久化。
3. `active`：现行信念，唯一的默认检索面（`as_of=now()`）。
4. `needs_confirm`：高风险谓词或来源冲突，等待人工确认。高风险动作的执行
   前置条件（`deny_if_trust_below` 的状态机形式）。
5. `stale`：曾经重要、现在可疑。区别于"不重要"：检索时可显式带上（供参考），
   但不进入"据此执行"。
6. `superseded`：被新边取代，`valid_to` 已闭合。历史可查（`as_of=<过去>`），
   **不可复活**为 `active`。
7. `archived`：巩固归档，退出检索，保留审计。
8. `rejected`：复核废弃，终态。

**双时态字段独立于状态机**：`valid_from/valid_to`（世界时间）与
`recorded_at`（系统时间）由 SUPERSEDE 维护；状态机管治理生命周期。二者
配合回答"现在什么是真的"（active + valid 区间覆盖 now）与"当时我们知道
什么"（recorded_at <= t 时刻的所有版本）。

### 3. 写入决策（WriteDecision）

候选命题与同 `(subject, predicate)` 现行边比较后的判定：

| 决策 | 条件 | 动作 |
|------|------|------|
| `ADD` | 无现行边 | 新建 active（或先 needs_confirm，见风险分级） |
| `SUPERSEDE` | 有现行边、对象不同、新来源优先级 ≥ 旧来源 | 旧边 `valid_to` 闭合 + `supersedes` 指向新边；新边落库 |
| `NOOP` | 有现行边、对象等价 | 不落库（幂等重放安全） |
| `CONFLICT` | 有现行边、对象不同、新来源弱于旧来源 | 现行边保持 active；弱源候选举报人工（`needs_confirm` 队列） |

约束：

- 单值谓词（见目录）的对象不同必然触发 SUPERSEDE 或 CONFLICT；
  多值谓词仅在对象匹配同一边时才比较。
- 抽取器应把"我换工作了"产出为 `SUPERSEDE works_at(old) + ADD works_at(new)`
  的复合意图，不是再插一条让检索随机命中旧值。

### 4. 风险分级与确认要求（RiskTier）

| 风险 | 语义 | 确认要求（`confirmation_required_for(source)`） |
|------|------|------------------------------------------------|
| `low` | 无门控（偏好、住址、团队成员身份） | 任何来源都不需确认 |
| `medium` | 影响路由与时效（在职、项目状态） | 仅弱来源（web / inferred）需确认 |
| `high` | 授权、付款、审批类（ownership、预算、汇报线） | 非 `system_of_record` 来源一律 `needs_confirm` |

矩阵形式：

| 来源 \ 风险 | low | medium | high |
|-------------|-----|--------|------|
| `system_of_record` | - | - | - |
| `user_stated` | - | - | **确认** |
| `tool` | - | - | **确认** |
| `inferred` | - | **确认** | **确认** |
| `web` | - | **确认** | **确认**（且首批目录直接不允许 web 断言） |

### 5. 谓词目录（首批，PREDICATE_CATALOG）

目录之外的谓词**不进信念层**，保持 L0 事件原文。扩展目录 = 修订本 ADR。

| 谓词 | 基数 | 可变性 | 允许来源 | TTL/stale | 风险 | 语义 |
|------|------|--------|----------|-----------|------|------|
| `works_at` | 单值 | 可变 | SoR、user、tool | StaleScan 90d | medium | 主体目前任职于客体组织 |
| `reports_to` | 单值 | 可变 | SoR、user、tool | StaleScan 90d | **high** | 主体目前向客体汇报 |
| `lives_in` | 单值 | 可变 | user、tool、inferred | StaleScan 90d | low | 主体目前居住在客体地点 |
| `prefers` | 多值 | 可变 | user、tool、inferred | StaleScan 365d | low | 主体偏好客体（持久偏好；会话级偏好不是信念） |
| `member_of` | 多值 | 可变 | SoR、user、tool | StaleScan 90d | low | 主体是客体团队的当前成员 |
| `owner_of` | 多值 | 可变 | **仅 SoR** | SorDriven | **high** | 主体当前拥有客体账户/项目（授权类） |
| `project_status` | 单值 | 可变 | SoR、user、tool | StaleScan 30d | medium | 主体项目当前处于客体状态 |
| `promised` | 多值 | 时限 | user、tool | ExpiresAtDueDate | medium | 主体承诺客体；到期转为情节历史 |
| `budget_owner` | 单值 | 可变 | **仅 SoR** | SorDriven | **high** | 主体预算当前由客体负责（财务 SoR） |
| `contract_number` | 单值 | **不可变** | SoR、tool | NoTtl | medium | 主体协议的合同编号 |

设计要点：

- **授权类谓词 SoR 独占**（`owner_of`、`budget_owner`）：用户自述"我拥有账户
  X"不能授予所有权；网页更不能。这落实 Epic #124 记忆契约中
  `must_not_believe_from.web: [authorization, ...]` 的最小版本。
- **Web 在首批目录中被全面排除**（保守默认）：网页观察进隔离区，不驱动任何
  受治谓词。放开需要论证，修改本 ADR。
- **不可变谓词不做时间扫描**：`contract_number` 等的真实性不随时间衰减，
  纠错 = 新证据 + 人工确认的 SUPERSEDE。
- **`promised` 到期情节化**：承诺到期后离开现行集合、保留为 L0 历史可回溯，
  而不是挂成 stale。

### 6. TTL / stale 策略（TtlPolicy）

| 策略 | 适用 | 行为 |
|------|------|------|
| `NoTtl` | 不可变谓词 | 只有 SUPERSEDE（人审）或显式归档能退休 |
| `StaleScan { reconfirm_days }` | 可变事实 | #129 巩固作业：窗口内无任何允许来源再确认 → `stale` |
| `SorDriven` | SoR 独占谓词 | 夜间 SoR 对账失效（"权威系统更新时失效，不靠时间"） |
| `ExpiresAtDueDate` | 时限承诺 | 到期转情节历史 |

## 备选方案

### 方案 A：只有 active/deleted 两态

- 优点：实现最简
- 缺点：无法表达"过期嫌疑"与"待确认"与"隔离"的差别；投毒防护（quarantine）、
  过期检测（stale）、高风险确认（needs_confirm）全部无从落地，这正是
  Epic #124 指出的"生产里会炸"的部分
- 不选原因：治理语义不可压缩

### 方案 B：每谓词独立状态机

- 优点：谓词级定制自由
- 缺点：N 个谓词 × 8 状态的组合爆炸；巩固作业与治理 API 要遍历所有变体
- 不选原因：风险/TTL/来源策略已经按谓词参数化，状态机本身保持单一

### 方案 C：单一状态机 + 谓词目录参数化（本方案）

- 优点：状态机一处定义、一处验证；谓词差异全部落在目录字段
- 缺点：新增治理维度时可能需要扩状态机（走 ADR 修订）
- 选择原因：与 Epic #124 的五层防护（门卫/契约/分区/信任检索/回滚）一一对应

## 决策结果

**采用方案 C。**

影响：
- `models/belief.rs` 为代码级契约：`BeliefSource` / `BeliefStatus` /
  `WriteDecision` / `PREDICATE_CATALOG`，全部带 parse/as_str 与单测
- #127 建 belief 表时：`status` 与 `source` 列的 CHECK 值必须等于
  `BeliefStatus::ALL` / `BeliefSource::ALL`，并加 anti-drift 测试
  （复用 `models/memory_enums.rs` 的 `parse_check_in_values` 模式）
- 状态转移在服务层强制（#127 写入门卫、#129 巩固作业调用
  `can_transition_to` 校验），不用 DB 触发器（避免方言耦合）
- #128 召回默认只取 `active` 且 `as_of=now()`；#130 治理 API 暴露状态机
  查询与人工确认/否决端点

## 企业内控补充

- 应用等级：T3（内部系统，非主营主链路）
- 技术架构等级：标准
- 资产文档入口：本 ADR + ADR-0010 + `backend/src/models/belief.rs`

## 后续动作

- [x] `models/belief.rs` 枚举 + 状态机 + 目录 + 单测（#125 交付）
- [ ] #127：belief 表 + 写入门卫（含三方 anti-drift 测试与状态转移审计日志）
- [ ] #128：召回只取 `active`；`as_of` 查询语义
- [ ] #129：stale 扫描、SoR 对账、web 信任衰减（48h）、承诺到期情节化
- [ ] #130：治理 API（确认/否决/回滚）与黄金验收测试
