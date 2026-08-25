# ADR-0010: L0–L3 记忆语义分层与 Persona/Scenario 模型

## 决策信息

| 字段 | 值 |
|------|-----|
| 编号 | ADR-0010 |
| 标题 | L0–L3 记忆语义分层与 Persona/Scenario 模型 |
| 状态 | Proposed |
| 日期 | 2026-08-25 |
| Owner | tech-lead |
| 关联 Issue | #88 |

## 背景与约束

### 当前状态

Aetheris 当前以 STM / LTM / KG / MM / Procedural 五层作为物理/逻辑存储层：

| 层 | 职责 | 存储介质 |
|----|------|---------|
| STM | 会话级短期记忆 | PostgreSQL (sessions + messages) |
| LTM | 持久化长期记忆 | PostgreSQL + Qdrant (vector) |
| KG | 结构化知识图谱 | PostgreSQL + Neo4j |
| MM | 多模态记忆 | PostgreSQL |
| Procedural | 程序性技能/步骤 | PostgreSQL |

这些层描述的是**存储与检索介质**，而非记忆的**语义生命周期**。当前缺少：
- 从原始 turn 到可更新事实的提炼路径
- 用户画像/偏好的独立建模与证据追溯
- 长期情境/目标的聚合与演化

### 目标

建立与现有存储层**正交**的 L0–L3 语义分层，定义每种语义记忆的：
- Schema、来源、更新策略、召回策略、删除策略
- 与物理存储层的映射关系
- Persona 和 Scenario 作为一等记忆类型

### 约束

- L0–L3 不替换 STM/LTM/KG/MM；前者描述语义和生命周期，后者描述存储与检索介质
- 每条语义记忆可映射到一个或多个物理存储层
- 冲突更新不直接覆盖历史版本（bitemporal 或 append-only）
- Persona/Scenario 的每条结论均可追溯到 L0/L1 证据

### 非目标

- 本次 ADR 不定义具体的 API 或数据库 schema
- 不覆盖 Skill/Wiki/CodeGraph 资产模型（见 #90）
- 不覆盖 Agent Loadout/ACL（见 #89）

## 方案

### L0–L3 语义分层

```
┌─────────────────────────────────────────────────┐
│ L3: Scenario — 长期情境、目标、行为模式          │
│   聚合自 L1/L2，跨会话，慢更新，高稳定性         │
├─────────────────────────────────────────────────┤
│ L2: Persona — 用户画像、偏好、约束               │
│   每结论带置信度与证据指针，可修正                │
├─────────────────────────────────────────────────┤
│ L1: Facts — 可更新的事实、偏好、约束              │
│   从 L0 提炼，可合并/冲突/过期                   │
├─────────────────────────────────────────────────┤
│ L0: Events — 原始 turn/event，完整来源            │
│   不可变，append-only，保留完整上下文             │
└─────────────────────────────────────────────────┘
```

### 各层定义

#### L0: Events（原始事件）

- **来源**：用户消息、Agent 回复、工具调用结果、系统事件
- **Schema**：`{ id, session_id, source, raw_content, metadata, timestamp }`
- **生命周期**：不可变，append-only；按 session 归档；定期清理过期 session
- **存储**：STM（热数据）+ 归档（冷数据）
- **更新**：不允许
- **删除**：按 retention policy 批量清理
- **召回**：按 session_id 顺序读取

#### L1: Facts（可更新事实）

- **来源**：从 L0 提炼（LLM 抽取 + 规则）
- **Schema**：`{ id, subject, predicate, object, confidence, evidence_l0_ids, version, valid_from, valid_until, superseded_by }`
- **生命周期**：
  - 创建：从 L0 自动抽取
  - 更新：新证据到达时更新，旧版本标记 `valid_until`，指向新版本 `superseded_by`
  - 过期：标记 `valid_until`（如"用户住址"变更）
  - 冲突：同时存在多个版本时，按 confidence + recency 排序，保留冲突历史
- **存储**：LTM（结构化）+ KG（subject-predicate-object 三元组）
- **更新策略**：bitemporal（valid_time + transaction_time），不直接覆盖
- **召回**：按 subject 查询最新版本；按时间范围回溯历史版本
- **删除**：软删除，保留审计轨迹

#### L2: Persona（用户画像）

- **来源**：从 L1 聚合 + LLM 生成
- **Schema**：`{ id, user_id, trait_type, trait_value, confidence, evidence_l1_ids, generated_at, updated_at, version }`
- **trait_type 示例**：`preference`、`constraint`、`expertise`、`communication_style`、`role`
- **生命周期**：
  - 生成：定期从 L1 事实聚合生成画像快照
  - 更新：新 L1 事实触发增量更新
  - 修正：用户显式反馈 > 自动推断
- **存储**：LTM（结构化 JSON）+ KG（用户节点属性）
- **置信度**：每条 trait 附带 confidence（0.0–1.0），基于：
  - 证据数量与质量
  - 用户显式确认/否定
  - 时间衰减（旧证据降低置信度）
- **召回**：按 user_id 加载画像；支持按 trait_type 过滤
- **删除**：用户可请求删除特定 trait 或完整画像；GDPR 合规

#### L3: Scenario（长期情境）

- **来源**：从 L1 + L2 聚合 + LLM 生成
- **Schema**：`{ id, user_id, scenario_type, summary, goals, constraints, participants, evidence_ids, created_at, updated_at, status }`
- **scenario_type 示例**：`project`、`support_ticket`、`learning_path`、`relationship`
- **生命周期**：
  - 创建：L1/L2 模式识别 + LLM 生成
  - 更新：增量更新，保留历史快照
  - 结束：标记 `status: closed`，保留为历史情境
- **存储**：LTM（结构化 JSON）+ KG（情境节点与关系）
- **与 L1/L2 的关系**：Scenario 聚合多个 L1 事实和 L2 画像，形成高层情境理解
- **召回**：按 user_id 加载活跃情境；按时间范围回溯历史情境

### 与物理存储层的映射

| 语义层 | 主存储层 | 辅助存储层 | 索引 |
|--------|---------|-----------|------|
| L0 Events | STM (sessions + messages) | — | session_id, timestamp |
| L1 Facts | LTM (structured) | KG (triples) | subject, embedding |
| L2 Persona | LTM (structured JSON) | KG (node attributes) | user_id, trait_type |
| L3 Scenario | LTM (structured JSON) | KG (scenario nodes) | user_id, status |

### 数据流

```
User Message → L0 Event (STM)
     ↓
LLM Extraction → L1 Facts (LTM + KG)
     ↓
LLM Aggregation → L2 Persona (LTM)
     ↓
Pattern Recognition → L3 Scenario (LTM + KG)
     ↓
Context Injection ← L1 + L2 + L3 (on next turn)
```

### 冲突与版本策略

- L0：不可变，append-only
- L1：bitemporal 版本控制（valid_time + transaction_time），冲突保留多版本，按 confidence 排序
- L2：每次更新生成新版本，旧版本保留为历史快照
- L3：增量更新，保留历史快照，status 标记生命周期

### 隐私与合规

- L2 Persona 支持用户显式查看、修正、删除
- L1/L2 证据链支持"被遗忘权"——删除 L0 源数据时，级联标记依赖的 L1/L2 为"证据过期"
- 所有层支持 tenant 隔离（现有 RLS 机制）

## 备选方案

### 方案 A：直接在 STM/LTM 上建模 Persona/Scenario（不引入 L0–L3）

- 优点：实现简单，不需要新增抽象层
- 缺点：语义生命周期与存储生命周期耦合，Persona 更新 = LTM 行更新，丢失版本历史；无法区分"用户偏好"和"对话事实"
- 不选原因：无法满足冲突版本、证据追溯、置信度管理等需求

### 方案 B：引入独立的 Persona/Scenario 服务（微服务拆分）

- 优点：独立部署、独立扩展
- 缺点：增加运维复杂度，跨服务事务，与现有单体架构不一致
- 不选原因：当前阶段不适合引入新的微服务

### 方案 C：L0–L3 作为逻辑层，复用现有存储（本方案）

- 优点：最小化架构变更，语义清晰，版本控制可叠加
- 缺点：需要仔细设计映射关系，避免概念混淆
- 选择原因：满足所有需求，不引入新的基础设施

## 决策结果

**采用方案 C：L0–L3 作为逻辑语义层，正交于现有物理存储层。**

影响：
- `kernel/types.rs` 新增 `SemanticLayer` 枚举（L0/L1/L2/L3）
- `MemoryMetadata` 新增 `semantic_layer` 字段
- 新增 L1/L2/L3 的 schema 定义（`models/` 或独立 crate）
- 新增 L0→L1 提炼、L1→L2 聚合、L2→L3 模式识别的 service 模块
- 蒸馏/转移 pipeline 需要感知语义层（L0→L1 是提炼，STM→LTM 是物理转移）
- 现有 API 不受影响，语义层为内部概念

兼容性：向后兼容。现有 STM/LTM/KG/MM 操作不受影响。语义层是可选的增强。

## 企业内控补充

- 应用等级：T3（内部系统，非主营主链路）
- 技术架构等级：标准
- 关键组件：复用现有 PostgreSQL + Qdrant + Neo4j
- 资产文档入口：本 ADR + `docs/artifacts/l0-l3-taxonomy/`

## 后续动作

- [ ] 在 `kernel/types.rs` 中定义 `SemanticLayer` 枚举和 L1/L2/L3 基础 schema
- [ ] 实现 L0→L1 事实提炼（复用 `memory_transfer.rs` 的 summary/entity/relation 提取）
- [ ] 实现 L1→L2 Persona 聚合
- [ ] 实现 L2/L1→L3 Scenario 模式识别
- [ ] 更新蒸馏/转移 pipeline 支持语义层
- [ ] 添加双时序版本控制测试
- [ ] 添加 Persona 置信度 + 证据追溯测试
- [ ] 更新 `CLAUDE.md` 记忆架构文档