# PRD: LangChain Framework SDK Integration

## 背景

Aetheris MemOS 已提供稳定的 Python SDK (`adaptive-memory`)，通过 REST/MCP 接口完成记忆的读写、搜索和反馈。但当前 SDK 是原始 HTTP client，LangChain 开发者无法直接在 agent/chain 工作流中即插即用。需要提供标准的 LangChain 接口实现，降低外部开发者集成门槛。

## 目标与成功标准

### 业务目标

- 让 LangChain 生态开发者能零适配使用 Aetheris MemOS 作为 agent 记忆后端。
- 发布独立 Python 包 `adaptive-memory-langchain`，可通过 pip 安装。

### 用户价值

- LangChain agent 可以直接使用 `AdaptiveMemoryTool` 进行 remember/recall/search/forget。
- LangChain chain 可以使用 `AdaptiveMemoryChatMessageHistory` 做会话记忆持久化。
- LangChain RAG pipeline 可以使用 `AdaptiveMemoryRetriever` 召回 LTM 上下文。

### 成功指标

1. LangChain agent 使用 Tool 完成 remember → recall → feedback 全流程 ✅
2. LangChain chain 使用 ChatMessageHistory 完成多轮会话存取 ✅
3. LangChain retriever 从 hybrid search 召回 top-K 相关记忆 ✅
4. 有完整 contract tests 和 live E2E demo ✅
5. README 文档可指导用户 5 分钟完成集成 ✅

## 用户故事

### US-1: Agent Memory Tool

**作为** LangChain agent 开发者，
**我想要** 在 agent 的 tools 列表中加入一个记忆工具，
**以便** agent 可以自主决定何时存储、检索和遗忘信息。

**验收标准：**
- Tool 支持 remember/recall/search/forget 四个 action
- 返回结构化结果（memoryId, content, score）
- 异常时返回可读的错误信息而非 500 traceback

### US-2: Chat Message History

**作为** LangChain conversational chain 开发者，
**我想要** 用 Aetheris MemOS 替代 InMemory/Redis ChatMessageHistory，
**以便** 会话记忆可以跨 session 持久化并支持语义搜索。

**验收标准：**
- 实现 `BaseChatMessageHistory` 接口（add_messages, messages, clear）
- 自动创建/复用 STM session
- 支持指定 user_id / agent_id / session_id

### US-3: Memory Retriever for RAG

**作为** LangChain RAG pipeline 开发者，
**我想要** 一个 retriever 能从 MemOS LTM 中按语义搜索召回上下文，
**以便** 我的 chain 可以利用长期记忆增强回答质量。

**验收标准：**
- 实现 `BaseRetriever` 接口（_get_relevant_documents）
- 支持配置 top_k, min_score, search_type(hybrid/ltm/triple)
- 返回 LangChain `Document` 对象，带 metadata

## 范围

### In Scope

- 独立 Python 包 `adaptive-memory-langchain`
- LangChain BaseChatMessageHistory 实现
- LangChain BaseRetriever 实现
- LangChain BaseTool 实现（支持多 action）
- Contract tests (mock backend)
- Live E2E demo script
- README + API 文档

### Out of Scope

- LlamaIndex 集成（后续迭代）
- CrewAI / AutoGen 支持
- 后端 API 变更
- Streaming 搜索
- Token 预算管理
- LCEL Runnable 深度集成
- 多 agent 共享记忆编排

## 技术约束

- LangChain 基线：`langchain-core >= 0.2`
- 依赖核心 SDK：`adaptive-memory >= 0.1.0`
- Python 版本：>= 3.9
- 不引入额外基础设施依赖（纯 client-side library）

## 风险与依赖

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| LangChain API 频繁变动 | 集成模块过期 | 仅依赖 `langchain-core` 稳定接口，pin 最低版本 |
| 核心 SDK client 内部重构 | 集成层 break | 通过 contract tests 守护接口边界 |
| 搜索结果格式变化 | Document 构造失败 | 使用 defensive parsing + fallback |

## 待确认项

- [x] LangChain 版本基线 → `langchain-core >= 0.2`
- [x] LlamaIndex 本期是否必须 → 否
- [x] 是否支持 CrewAI / AutoGen → 否
- [x] SDK 包名策略 → 独立包 `adaptive-memory-langchain`
