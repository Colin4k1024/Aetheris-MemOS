# PRD: 记忆架构缺口补齐

## 背景

Aetheris-MemOS 定位为 AI Agent 的记忆存储与检索服务。经对标企业AI Agent三位一体架构（enterprise_agent_arch_v2），在"智能记忆与个性化"层发现三个明确缺口：

1. **程序记忆 (Procedural Memory)** 语义偏差：当前 MM Layer 实现的是多模态媒体存储（图片/音频/视频），而非架构要求的"how-to-do"技能/过程记忆。
2. **多后端适配器** 不足：架构期望 4 种记忆后端可插拔（Builtin/Mem0/Zep/Letta），当前仅内建 SQLx + Redis STM。
3. **GraphRAG 混合检索** 缺统一编排层：KG（Neo4j）和 Vector（Qdrant）各自独立查询，无混合排序/融合入口。

触发原因：架构合规性评审，确保 MemOS 能无缝嵌入上层 Agent Runtime 作为记忆子系统。

当前约束：
- 项目技术栈为 Rust (Axum)，前端 React (Ant Design Pro)
- 已有 `MemoryLayer` trait 定义和 4 个 layer 实现（STM/LTM/KG/MM）
- 已有 `MemoryKernel` / `VectorSearch` / `GraphMemory` trait 体系
- 不涉及 Agent Runtime 能力（推理/编排/技能），仅限记忆存储检索

## 目标与成功标准

### 业务目标
- 完成记忆存储检索服务对企业架构的 100% 覆盖
- 为上层 Agent Runtime 提供标准化、可插拔的记忆接口

### 成功指标
1. Procedural Memory Layer 可存储/检索技能模板、操作步骤、工具调用链
2. 至少新增 2 个外部记忆后端适配器（Mem0 + Zep），接口可扩展到更多
3. GraphRAG 混合检索 API 单次调用融合 KG 图遍历 + Vector 语义搜索结果

## 用户故事

### US-1: 程序记忆存储与检索
**作为** Agent Runtime  
**我希望** 存储和检索"操作步骤/技能模板/工具调用链"类型的过程记忆  
**以便** Agent 在遇到相似任务时能复用历史成功执行路径

验收标准：
- [ ] 新增 `ProceduralMemoryLayer` 实现 `MemoryLayer` trait
- [ ] 支持存储结构化步骤（steps: Vec<Step>）、前置条件、成功率、调用上下文
- [ ] 支持按任务类型/工具名/相似度检索过程记忆
- [ ] 支持过程记忆的版本演进（同一技能多版本，按成功率排序）
- [ ] 与现有 4 层 layer 共存，LayerType 新增 `Procedural` 变体

### US-2: 多后端适配器
**作为** 平台运维  
**我希望** 通过配置切换记忆后端（内建/Mem0/Zep/Letta）  
**以便** 不同部署场景选择最适合的记忆引擎

验收标准：
- [ ] 定义 `MemoryBackend` trait 作为后端适配器统一接口
- [ ] 实现 `BuiltinBackend`（当前实现包装）
- [ ] 实现 `Mem0Backend`（通过 HTTP API 对接 Mem0）
- [ ] 实现 `ZepBackend`（通过 HTTP API 对接 Zep）
- [ ] 后端选择通过配置文件 (TOML/env) 驱动，支持运行时热切换
- [ ] Letta 适配预留接口定义，标记为 `unimplemented!()`

### US-3: GraphRAG 混合检索
**作为** Agent Runtime  
**我希望** 单次 API 调用同时利用知识图谱和向量搜索  
**以便** 获得兼具结构关系和语义相关性的检索结果

验收标准：
- [ ] 新增 `HybridSearchService` 编排层
- [ ] 支持 3 种融合策略：`VectorFirst`（向量为主，图谱补充关系）、`GraphFirst`（图谱为主，向量补充语义）、`Reciprocal Rank Fusion`（RRF 分数融合）
- [ ] 融合结果包含来源标注（vector/graph/both）
- [ ] 支持配置权重（vector_weight / graph_weight）
- [ ] 暴露 REST API: `POST /api/v1/memory/search/hybrid`

## 范围

### In Scope
- Procedural Memory Layer 数据模型 + 存储 + 检索
- MemoryBackend trait 定义 + Builtin/Mem0/Zep 三个实现
- HybridSearchService 编排层 + RRF 融合算法
- 对应的 REST API 端点
- 单元测试 + 集成测试

### Out of Scope
- Agent Runtime 推理/编排能力
- 用户画像 / 情感感知
- 前端页面变更（后续迭代）
- Letta 后端完整实现（仅预留接口）
- 性能基准测试（后续迭代）

## 风险与依赖

| 风险 | 影响 | 缓解 |
|------|------|------|
| Mem0/Zep API 兼容性变化 | 适配器失效 | 版本锁定 + 抽象层隔离 |
| LayerType 枚举变更 | 破坏序列化兼容 | 使用 `#[serde(rename)]` 保证向后兼容 |
| RRF 融合效果不佳 | 检索质量下降 | 提供多策略可选 + 权重可调 |
| 现有 MM Layer 语义变更 | 多模态功能回归 | MM Layer 保持不变，新增 Procedural Layer |

### 关键依赖
- Qdrant 实例可用（Vector 检索）
- Neo4j 实例可用（Graph 检索）
- Mem0 / Zep 服务实例（集成测试需要）

### 待确认项
1. Procedural Memory 的步骤结构是否需要支持嵌套/DAG？—— 建议 v1 仅线性步骤
2. Mem0/Zep 对接使用哪个 API 版本？—— 建议使用最新稳定版
3. 混合检索的默认融合策略？—— 建议默认 RRF

## 企业治理待确认项

- 应用等级：T3（内部平台服务）
- 数据/合规风险：低（记忆数据不直接包含用户PII，由上层Agent负责脱敏）
- 集团组件约束：无（独立开源项目）

## 参与角色清单

| 角色 | 职责 |
|------|------|
| tech-lead | 架构决策、接口设计收口、验收 |
| backend-engineer | Rust 实现、测试、API 开发 |
| architect | trait 体系设计、后端适配器模式确认 |

## 领域技能包启用建议

- `rust-patterns`: trait 设计、enum state machine
- `rust-testing`: 集成测试、mockall
- `api-design`: REST API 端点设计

## UI 范围

本次迭代不涉及前端变更。后续迭代可能在 MemoryDetails 页面增加 Procedural Memory 列表视图。

## 需求挑战会候选分组

**Group A: 架构层**
- 议题：`MemoryBackend` trait 与现有 `MemoryLayer` trait 的关系界定
- 参与：tech-lead + architect

**Group B: 数据模型**
- 议题：Procedural Memory 步骤结构设计（线性 vs DAG）
- 参与：tech-lead + backend-engineer

**Group C: 检索策略**
- 议题：RRF 融合算法参数选择 + 多策略切换机制
- 参与：tech-lead + backend-engineer
