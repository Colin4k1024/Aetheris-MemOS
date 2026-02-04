# 会话历史写入与记忆提取 - 快速开始

## 📌 核心要点

### 系统已具备的功能

✅ **短期记忆（STM）存储** - 已实现
- API: `POST /api/v1/memory/storage/stm`
- 功能：存储会话消息到 SQLite

✅ **长期记忆（LTM）存储** - 已实现  
- API: `POST /api/v1/memory/storage/ltm`
- 功能：LLM 总结 + 向量化 + 存储到 Milvus

✅ **自动转移服务** - 已实现
- 服务：`MemoryTransferService`
- 功能：自动将 STM 转移到 LTM

## 🚀 快速使用

### 1. 写入会话历史

```rust
use crate::services::memory_storage::MemoryStorageService;

// 写入用户消息
let (session_id, _) = MemoryStorageService::store_stm(
    "user_123",
    "agent_456", 
    "conversation",
    "user",
    "用户的消息",
    4096,  // max_context_length
    24,    // retention_hours
).await?;

// 写入 AI 回复
MemoryStorageService::store_stm(
    "user_123",
    "agent_456",
    "conversation", 
    "assistant",
    "AI 的回复",
    4096,
    24,
).await?;
```

### 2. 提取为长期记忆

```rust
use crate::services::memory_storage::MemoryStorageService;

// 方式 1：从 STM 转移到 LTM（推荐）
let entry_ids = MemoryStorageService::auto_transfer_stm_to_ltm(
    &session_id,
    100,  // message_count_threshold
).await?;

// 方式 2：直接存储为 LTM
let entry_id = MemoryStorageService::store_ltm(
    &session_id,
    "conversation",
    &combined_content,
    Some("会话标题"),
).await?;
```

### 3. 启用自动转移

在 `main.rs` 中添加：

```rust
use crate::services::memory_transfer::init_transfer_service;

// 初始化自动转移服务
// 参数：检查间隔(秒), 消息数量阈值, 会话时间阈值(小时)
init_transfer_service(3600, 100, 24).await?;
```

## 📡 API 调用示例

### 写入会话消息

```bash
curl -X POST http://localhost:8008/api/v1/memory/storage/stm \
  -H "Content-Type: application/json" \
  -d '{
    "userId": "user_123",
    "agentId": "agent_456",
    "sessionType": "conversation",
    "role": "user",
    "content": "你好",
    "maxContextLength": 4096,
    "retentionHours": 24
  }'
```

### 获取会话消息

```bash
curl http://localhost:8008/api/v1/memory/storage/stm/{session_id}?limit=100
```

### 手动触发转移

```bash
curl -X POST http://localhost:8008/api/v1/memory/storage/transfer \
  -H "Content-Type: application/json" \
  -d '{
    "sessionId": "session_id_here",
    "messageCountThreshold": 100
  }'
```

## 🔄 完整流程示例

```rust
async fn handle_chat_message(user_id: &str, agent_id: &str, message: &str) -> Result<()> {
    use crate::services::memory_storage::MemoryStorageService;
    use crate::db::stm::STMRepository;
    
    // 1. 写入用户消息
    let (session_id, _) = MemoryStorageService::store_stm(
        user_id, agent_id, "conversation", "user", message, 4096, 24
    ).await?;
    
    // 2. 生成 AI 回复
    let ai_response = generate_ai_response(message).await?;
    
    // 3. 写入 AI 回复
    MemoryStorageService::store_stm(
        user_id, agent_id, "conversation", "assistant", &ai_response, 4096, 24
    ).await?;
    
    // 4. 检查是否需要转移到 LTM
    let messages = STMRepository::get_session_messages(&session_id, None).await?;
    if messages.len() >= 100 {
        MemoryStorageService::auto_transfer_stm_to_ltm(&session_id, 100).await?;
    }
    
    Ok(())
}
```

## 📚 详细文档

- [完整集成指南](./会话历史集成指南.md) - 详细的使用说明和最佳实践
- [技术文档](./conversation_history_integration.md) - 系统架构和技术细节
- [代码示例](../examples/conversation_history_example.rs) - 更多代码示例

## ⚙️ 配置说明

### 记忆提取阈值

- **消息数量阈值**：建议 50-200 条
- **时间阈值**：建议 24-72 小时
- **重要性评分**：可选，用于优先提取重要消息

### 系统配置

在 `config.toml` 中配置：

```toml
[llm]
base_url = "http://localhost:11434"
model = "llama2"
timeout_seconds = 30

[embedding]
base_url = "http://localhost:11434"
model = "nomic-embed-text"
dimension = 768

[milvus]
host = "localhost"
port = 19530
collection_name = "long_term_memory"
vector_dimension = 768
```

## ❓ 常见问题

**Q: 如何判断会话是否需要提取？**
A: 系统会自动检查消息数量和会话时间，也可以手动触发。

**Q: 提取失败怎么办？**
A: 检查 LLM 服务（Ollama）、嵌入模型和 Milvus 是否正常运行。

**Q: 如何避免重复提取？**
A: 系统会标记已转移的会话，使用相同的 `session_id` 作为 `source_id`。

## 🎯 总结

系统已提供完整的会话历史写入和记忆提取功能，只需：

1. 调用 `store_stm()` 写入会话消息
2. 调用 `auto_transfer_stm_to_ltm()` 或 `store_ltm()` 提取记忆
3. （可选）启用自动转移服务实现自动化

所有功能都已实现，可以直接使用！

