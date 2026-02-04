# 会话历史写入与记忆提取集成方案

## 概述

本文档分析如何向自适应记忆系统中写入会话历史，并将这些历史提取为最终的长期记忆。

## 系统架构分析

### 当前系统组件

1. **短期记忆（STM）存储**
   - 数据库表：`context_sessions` 和 `session_messages`
   - 服务：`MemoryStorageService::store_stm()`
   - 仓库：`STMRepository`

2. **长期记忆（LTM）存储**
   - 向量数据库：Milvus
   - 关系数据库：SQLite (`long_term_memory` 表)
   - 服务：`MemoryStorageService::store_ltm()`
   - 处理流程：LLM 总结 → 向量化 → 存储

3. **记忆转移服务**
   - 服务：`MemoryTransferService`
   - 功能：自动将 STM 转移到 LTM（基于阈值）

### 数据流

```
会话消息 → STM (SQLite) → [自动/手动触发] → LTM (LLM总结 + 向量化) → Milvus + SQLite
```

## 实现方案

### 方案一：直接使用现有 API（推荐）

系统已经提供了完整的 API 接口，可以直接使用：

#### 1. 写入会话历史到 STM

**API 端点：** `POST /api/memory-storage/stm`

**请求示例：**
```json
{
  "userId": "user_123",
  "agentId": "agent_456",
  "sessionType": "conversation",
  "role": "user",
  "content": "用户的消息内容",
  "maxContextLength": 4096,
  "retentionHours": 24
}
```

**响应：**
```json
{
  "sessionId": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
  "messageId": "01ARZ3NDEKTSV4RRFFQ69G5FAW"
}
```

**使用场景：**
- 每次用户发送消息时调用
- 每次 AI 回复时调用（role 设为 "assistant"）

#### 2. 提取会话历史为长期记忆

**方式 A：手动触发转移**

**API 端点：** `POST /api/memory-storage/transfer`

**请求示例：**
```json
{
  "sessionId": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
  "messageCountThreshold": 100
}
```

**处理流程：**
1. 获取会话中的所有消息
2. 合并消息内容
3. 调用 LLM 进行总结和结构化提取
4. 生成向量嵌入
5. 存储到 Milvus 和 SQLite

**方式 B：自动转移（已实现）**

系统已实现自动转移服务 `MemoryTransferService`，会定期检查符合条件的会话并自动转移到 LTM。

**配置参数：**
- `check_interval`: 检查间隔（秒）
- `message_count_threshold`: 消息数量阈值
- `session_time_threshold`: 会话时间阈值（小时）

### 方案二：创建专门的会话历史 API（增强版）

如果需要更细粒度的控制，可以创建专门的 API：

#### 1. 批量写入会话历史

**新 API 端点：** `POST /api/conversation-history/batch`

**功能：**
- 一次性写入多条消息
- 自动创建或更新会话
- 支持消息元数据

#### 2. 智能记忆提取

**新 API 端点：** `POST /api/conversation-history/extract-memory`

**功能：**
- 分析会话重要性
- 智能选择需要提取的内容
- 支持自定义提取策略

## 代码实现示例

### 示例 1：写入单条会话消息

```rust
use crate::services::memory_storage::MemoryStorageService;

// 写入用户消息
let (session_id, message_id) = MemoryStorageService::store_stm(
    "user_123",
    "agent_456",
    "conversation",
    "user",
    "用户的问题",
    4096,
    24,
).await?;

// 写入 AI 回复
let (_, _) = MemoryStorageService::store_stm(
    "user_123",
    "agent_456",
    "conversation",
    "assistant",
    "AI 的回答",
    4096,
    24,
).await?;
```

### 示例 2：提取会话为长期记忆

```rust
use crate::services::memory_storage::MemoryStorageService;

// 手动触发转移
let entry_ids = MemoryStorageService::auto_transfer_stm_to_ltm(
    &session_id,
    100, // 消息数量阈值
).await?;
```

### 示例 3：直接存储为长期记忆

```rust
use crate::services::memory_storage::MemoryStorageService;

// 直接存储为长期记忆（跳过 STM）
let entry_id = MemoryStorageService::store_ltm(
    &session_id,
    "conversation",
    &combined_content,
    Some("会话标题"),
).await?;
```

## 集成建议

### 1. 在聊天应用中集成

```rust
// 伪代码示例
async fn handle_chat_message(user_id: &str, agent_id: &str, message: &str) {
    // 1. 写入用户消息到 STM
    let (session_id, _) = MemoryStorageService::store_stm(
        user_id,
        agent_id,
        "conversation",
        "user",
        message,
        4096,
        24,
    ).await?;
    
    // 2. 生成 AI 回复
    let ai_response = generate_ai_response(message).await?;
    
    // 3. 写入 AI 回复到 STM
    MemoryStorageService::store_stm(
        user_id,
        agent_id,
        "conversation",
        "assistant",
        &ai_response,
        4096,
        24,
    ).await?;
    
    // 4. 检查是否需要转移到 LTM
    let messages = STMRepository::get_session_messages(&session_id, None).await?;
    if messages.len() >= 100 {
        // 自动转移到 LTM
        MemoryStorageService::auto_transfer_stm_to_ltm(&session_id, 100).await?;
    }
}
```

### 2. 定时任务集成

系统已实现自动转移服务，可以在 `main.rs` 中初始化：

```rust
// 在 main() 函数中
use crate::services::memory_transfer::init_transfer_service;

// 初始化自动转移服务
// 参数：检查间隔(秒), 消息数量阈值, 会话时间阈值(小时)
init_transfer_service(3600, 100, 24).await?;
```

## 数据模型

### 会话消息结构

```rust
pub struct SessionMessage {
    pub message_id: String,
    pub session_id: String,
    pub role: String,           // "user" 或 "assistant"
    pub content: String,
    pub created_at: String,
    pub token_count: Option<i32>,
    pub importance_score: Option<f64>,
}
```

### 长期记忆结构

存储在 Milvus 中的向量包含以下元数据：

```json
{
  "title": "会话标题",
  "summary": "LLM 生成的摘要",
  "entities": ["实体1", "实体2"],
  "relations": [
    {"from": "实体1", "to": "实体2", "type": "关系类型"}
  ],
  "key_facts": ["关键事实1", "关键事实2"]
}
```

## 最佳实践

### 1. 会话管理

- **会话 ID 复用**：同一对话使用相同的 `session_id`
- **会话类型**：根据场景设置 `session_type`（conversation/task/query）
- **保留时间**：根据重要性设置 `retention_hours`

### 2. 记忆提取策略

- **阈值设置**：
  - 消息数量阈值：建议 50-200 条
  - 时间阈值：建议 24-72 小时
- **重要性评分**：可以为重要消息设置 `importance_score`
- **批量处理**：对于大量历史数据，使用批量 API

### 3. 性能优化

- **异步处理**：记忆提取是耗时操作，建议异步执行
- **批量操作**：使用批量 API 减少数据库交互
- **缓存策略**：对于频繁访问的会话，可以添加缓存

## 监控和调试

### 1. 日志记录

系统已集成 `tracing`，所有操作都有日志记录：

```rust
tracing::info!("Storing STM: user_id={}, agent_id={}", user_id, agent_id);
```

### 2. 性能指标

可以通过以下 API 监控系统状态：

- `GET /api/memory/status` - 获取记忆系统状态
- `GET /api/memory/health` - 健康检查
- `GET /api/memory/resources` - 资源使用情况

## 故障处理

### 常见问题

1. **LLM 调用失败**
   - 检查 Ollama 服务是否运行
   - 检查网络连接
   - 查看日志中的错误信息

2. **向量化失败**
   - 检查嵌入模型服务
   - 验证向量维度配置

3. **Milvus 连接失败**
   - 检查 Milvus 服务状态
   - 验证连接配置

## 扩展功能建议

### 1. 会话重要性评估

在提取记忆前，评估会话的重要性，只提取有价值的会话。

### 2. 增量提取

不是一次性提取整个会话，而是增量提取关键信息。

### 3. 记忆去重

在存储到 LTM 前，检查是否已存在相似记忆，避免重复。

### 4. 记忆更新

对于已存在的记忆，支持更新而不是创建新条目。

## 总结

系统已经提供了完整的会话历史写入和记忆提取功能：

1. **写入会话历史**：使用 `MemoryStorageService::store_stm()`
2. **提取长期记忆**：使用 `MemoryStorageService::auto_transfer_stm_to_ltm()` 或 `store_ltm()`
3. **自动转移**：使用 `MemoryTransferService` 实现自动化

建议直接使用现有 API，根据实际需求调整阈值和策略参数。

