# 记忆系统 API 调用示例

本文档提供自适应记忆系统所有核心功能的 API 调用示例。

## 服务启动

```bash
# 1. 启动后端服务
cd backend
cargo run

# 2. 服务地址
# API: http://localhost:5800
# Swagger文档: http://localhost:5800/swagger-ui
```

---

## 1. 短期记忆 (STM) API

### 1.1 存储短期记忆（会话+消息）

```bash
# 同时创建会话并添加消息
curl -X POST http://localhost:5800/api/memory/storage/stm \
  -H "Content-Type: application/json" \
  -d '{
    "userId": "user_001",
    "agentId": "agent_001",
    "sessionType": "conversation",
    "role": "user",
    "content": "请介绍一下Python语言",
    "maxContextLength": 4096,
    "retentionHours": 24
  }'
```

**响应示例:**

```json
{
  "sessionId": "session_01abc123...",
  "messageId": "msg_01def456..."
}
```

### 1.2 创建会话

```bash
curl -X POST http://localhost:5800/api/memory/storage/session \
  -H "Content-Type: application/json" \
  -d '{
    "userId": "user_001",
    "agentId": "agent_001",
    "sessionType": "conversation",
    "maxContextLength": 4096
  }'
```

### 1.3 添加消息到会话

```bash
curl -X POST http://localhost:5800/api/memory/storage/message \
  -H "Content-Type: application/json" \
  -d '{
    "sessionId": "session_01abc123...",
    "role": "user",
    "content": "请介绍一下Python语言",
    "contentType": "text"
  }'
```

### 1.4 获取会话消息

```bash
curl -X GET "http://localhost:5800/api/memory/storage/session/{session_id}/messages"
```

### 1.5 搜索短期记忆

```bash
curl -X POST http://localhost:5800/api/memory/search/stm \
  -H "Content-Type: application/json" \
  -d '{
    "query": "Python 编程",
    "topK": 5
  }'
```

---

## 2. 长期记忆 (LTM) API

### 2.1 存储知识条目

```bash
curl -X POST http://localhost:5800/api/memory/storage/ltm \
  -H "Content-Type: application/json" \
  -d '{
    "sourceId": "doc_python_001",
    "sourceType": "document",
    "title": "Python语言简介",
    "content": "Python是一种高级编程语言，由Guido van Rossum于1991年首次发布。Python强调代码的可读性和简洁的语法，相比于C++或Java，Python让开发者能够用更少的代码表达想法。",
    "category": "编程语言",
    "domain": "计算机科学"
  }'
```

**响应示例:**

```json
{
  "entryId": "ltm_01abc123..."
}
```

### 2.2 获取知识条目

```bash
curl -X GET "http://localhost:5800/api/memory/search/ltm/entry/{entry_id}"
```

### 2.3 向量搜索长期记忆

```bash
curl -X POST http://localhost:5800/api/memory/search/ltm \
  -H "Content-Type: application/json" \
  -d '{
    "query": "机器学习 神经网络",
    "topK": 5,
    "enableRerank": true,
    "minScore": 0.5
  }'
```

**响应示例:**

```json
{
  "results": [
    {
      "entryId": "ltm_01abc123...",
      "score": 0.92,
      "title": "深度学习",
      "content": "深度学习是机器学习的一个分支...",
      "metadata": {}
    }
  ]
}
```

### 2.4 混合搜索

```bash
curl -X POST http://localhost:5800/api/memory/search/hybrid \
  -H "Content-Type: application/json" \
  -d '{
    "query": "Python 编程 学习",
    "topK": 5,
    "keywordWeight": 0.3,
    "vectorWeight": 0.7
  }'
```

---

## 3. 知识图谱 (KG) API

### 3.1 创建实体

```bash
curl -X POST http://localhost:5800/api/memory/kg/entity \
  -H "Content-Type: application/json" \
  -d '{
    "entityName": "埃隆·马斯克",
    "entityType": "人物",
    "description": "企业家和工程师，特斯拉和SpaceX的创始人",
    "aliases": ["Elon Musk", "马斯克"]
  }'
```

### 3.2 搜索实体

```bash
curl -X GET "http://localhost:5800/api/memory/kg/entity/search?q=马斯克"
```

### 3.3 创建关系

```bash
curl -X POST http://localhost:5800/api/memory/kg/relation \
  -H "Content-Type: application/json" \
  -d '{
    "sourceEntityId": "entity_01abc123...",
    "targetEntityId": "entity_01def456...",
    "relationType": "founded",
    "relationName": "创立",
    "weight": 0.95
  }'
```

### 3.4 获取实体关系

```bash
curl -X GET "http://localhost:5800/api/memory/kg/entity/{entity_id}/relations"
```

### 3.5 实体搜索知识

```bash
curl -X POST http://localhost:5800/api/memory/search/entity \
  -H "Content-Type: application/json" \
  -d '{
    "entityName": "人工智能",
    "topK": 10
  }'
```

---

## 4. 记忆整合 API

### 4.1 STM 到 LTM 转移

```bash
curl -X POST http://localhost:5800/api/memory/storage/transfer \
  -H "Content-Type: application/json" \
  -d '{
    "sessionId": "session_01abc123..."
  }'
```

**响应示例:**

```json
{
  "transferredCount": 5,
  "entryIds": ["ltm_01abc123...", "ltm_01def456..."]
}
```

### 4.2 批量存储长期记忆

```bash
curl -X POST http://localhost:5800/api/memory/storage/ltm/batch \
  -H "Content-Type: application/json" \
  -d '{
    "entries": [
      {
        "sourceId": "doc_001",
        "sourceType": "document",
        "title": "标题1",
        "content": "内容1"
      },
      {
        "sourceId": "doc_002",
        "sourceType": "document",
        "title": "标题2",
        "content": "内容2"
      }
    ]
  }'
```

---

## 5. 记忆调度 API

### 5.1 分析任务特征

```bash
curl -X POST http://localhost:5800/api/memory/analyze \
  -H "Content-Type: application/json" \
  -d '{
    "taskContext": "用户询问如何学习编程,需要给出详细的学习路线和建议",
    "expectedComplexity": "medium",
    "reasoningDepth": "deep"
  }'
```

**响应示例:**

```json
{
  "characteristics": {
    "taskType": "knowledge_query",
    "complexity": "medium",
    "reasoningDepth": "deep",
    "domain": "education",
    "requiresContext": true
  },
  "memoryStrategy": {
    "useStm": true,
    "useLtm": true,
    "useKg": true,
    "useMm": false,
    "weights": {
      "stm": 0.3,
      "ltm": 0.4,
      "kg": 0.3,
      "mm": 0.0
    }
  },
  "confidenceScore": 0.85
}
```

### 5.2 批量分析任务

```bash
curl -X POST http://localhost:5800/api/memory/analyze/batch \
  -H "Content-Type: application/json" \
  -d '{
    "tasks": [
      {
        "taskId": "task_001",
        "taskContext": "用户询问Python问题"
      },
      {
        "taskId": "task_002",
        "taskContext": "用户需要代码调试帮助"
      }
    ]
  }'
```

### 5.3 预测记忆配置性能

```bash
curl -X POST http://localhost:5800/api/memory/predict \
  -H "Content-Type: application/json" \
  -d '{
    "memoryConfig": {
      "stmWeight": 0.7,
      "ltmWeight": 0.3,
      "kgWeight": 0.0,
      "mmWeight": 0.0
    }
  }'
```

**响应示例:**

```json
{
  "efficiencyGain": 0.75,
  "coherenceGain": 0.82,
  "resourceCost": 0.45,
  "confidence": 0.88
}
```

---

## 6. 系统状态 API

### 6.1 获取记忆系统状态

```bash
curl -X GET http://localhost:5800/api/memory/status
```

**响应示例:**

```json
{
  "stmSessions": 156,
  "stmMessages": 2340,
  "ltmEntries": 5280,
  "kgEntities": 1245,
  "kgRelations": 3890
}
```

### 6.2 获取系统资源状态

```bash
curl -X GET http://localhost:5800/api/memory/resources
```

### 6.3 获取性能基线

```bash
curl -X GET http://localhost:5800/api/memory/baselines
```

### 6.4 获取权重历史

```bash
curl -X GET "http://localhost:5800/api/memory/weights/history?start_time=2024-01-01&end_time=2024-01-31"
```

---

## 7. 记忆配置 API

### 7.1 获取所有配置

```bash
curl -X GET http://localhost:5800/api/memory/configs
```

### 7.2 获取单个配置

```bash
curl -X GET http://localhost:5800/api/memory/config/{config_id}
```

### 7.3 创建配置

```bash
curl -X POST http://localhost:5800/api/memory/config \
  -H "Content-Type: application/json" \
  -d '{
    "configName": "平衡模式",
    "description": "平衡使用STM和LTM",
    "stmWeight": 0.4,
    "ltmWeight": 0.4,
    "kgWeight": 0.2,
    "mmWeight": 0.0,
    "isDefault": false
  }'
```

### 7.4 更新配置

```bash
curl -X PUT http://localhost:5800/api/memory/config/{config_id} \
  -H "Content-Type: application/json" \
  -d '{
    "configName": "平衡模式(已更新)",
    "stmWeight": 0.5,
    "ltmWeight": 0.3,
    "kgWeight": 0.2
  }'
```

### 7.5 删除配置

```bash
curl -X DELETE http://localhost:5800/api/memory/config/{config_id}
```

---

## 8. 决策追踪 API

### 8.1 获取最近的决策追踪

```bash
curl -X GET "http://localhost:5800/api/memory/decision-traces?limit=10"
```

### 8.2 获取特定任务的追踪

```bash
curl -X GET "http://localhost:5800/api/memory/decision-trace/task/{task_id}"
```

---

## 9. 完整演示流程

以下是一个完整的演示流程，涵盖所有核心功能：

```bash
# 1. 查看系统状态
curl -s http://localhost:5800/api/memory/status | jq .

# 2. 存储短期记忆
SESSION=$(curl -s -X POST http://localhost:5800/api/memory/storage/stm \
  -H "Content-Type: application/json" \
  -d '{
    "userId": "demo_user",
    "agentId": "demo_agent",
    "sessionType": "conversation",
    "role": "user",
    "content": "请给我讲讲人工智能的发展历史"
  }' | jq -r '.sessionId')

echo "Session ID: $SESSION"

# 3. 添加助手回复
curl -s -X POST http://localhost:5800/api/memory/storage/stm \
  -H "Content-Type: application/json" \
  -d "{
    \"userId\": \"demo_user\",
    \"agentId\": \"demo_agent\",
    \"sessionType\": \"conversation\",
    \"role\": \"assistant\",
    \"content\": \"人工智能的发展可以追溯到1956年的达特茅斯会议...\"
  }" | jq .

# 4. 存储长期记忆
curl -s -X POST http://localhost:5800/api/memory/storage/ltm \
  -H "Content-Type: application/json" \
  -d '{
    "sourceId": "doc_ai_history",
    "sourceType": "document",
    "title": "人工智能发展历史",
    "content": "人工智能经历了符号主义、连接主义、深度学习等阶段..."
  }' | jq .

# 5. 搜索知识
curl -s -X POST http://localhost:5800/api/memory/search/ltm \
  -H "Content-Type: application/json" \
  -d '{
    "query": "神经网络 深度学习",
    "topK": 3
  }' | jq '.results[] | {title, score}'

# 6. 分析任务
curl -s -X POST http://localhost:5800/api/memory/analyze \
  -H "Content-Type: application/json" \
  -d '{
    "taskContext": "用户询问如何学习编程"
  }' | jq '.memoryStrategy'

# 7. 查看最终状态
curl -s http://localhost:5800/api/memory/status | jq .
```

---

## 10. 常见错误处理

| 状态码 | 说明         | 常见原因                     |
| ------ | ------------ | ---------------------------- |
| 400    | 请求参数错误 | 缺少必填字段、格式不正确     |
| 404    | 资源不存在   | session_id、entry_id 不存在  |
| 500    | 服务器错误   | 数据库连接失败、服务异常     |
| 503    | 服务不可用   | 依赖服务(Qdrant/Neo4j)未启动 |

### 错误响应示例

```json
{
  "code": 400,
  "message": "Validation failed: session_type must be one of 'conversation', 'task', 'query'"
}
```

---

## 11. 快速演示脚本

除了手动使用 curl 调用 API 外，项目还提供了可直接运行的 Shell 脚本，方便快速验证记忆系统的各项功能。

### 11.1 脚本位置

```bash
backend/scripts/memory_demo.sh
```

### 11.2 功能说明

该脚本会依次演示以下功能：

1. **短期记忆 (STM)** - 创建会话、存储用户消息和助手回复
2. **长期记忆 (LTM)** - 存储知识条目、向量搜索（依赖 Qdrant）
3. **自适应调度** - 任务特征分析、性能预测
4. **系统状态** - 资源监控、健康检查

### 11.3 运行方式

```bash
# 默认使用 localhost:8008（需已启动后端）
bash backend/scripts/memory_demo.sh

# 自动拉取并启动 Docker Compose 服务，等待就绪后再执行演示（推荐一键测试）
bash backend/scripts/memory_demo.sh --docker
# 或: USE_DOCKER=1 bash backend/scripts/memory_demo.sh

# Docker 环境（容器内部调用 backend 服务名）
BASE_URL=http://backend:8008 bash backend/scripts/memory_demo.sh

# CI 模式（跳过等待按键）
CI=1 bash backend/scripts/memory_demo.sh
```

使用 `--docker` 时，脚本会：拉取镜像、启动 `backend`/`qdrant`/`ollama` 容器、轮询后端健康接口直至可用（默认最多 120 秒）、再执行演示；若 Qdrant 不可达则仅给出警告，演示仍会继续。

### 11.4 前置条件

- **非 Docker 模式**：后端服务已启动并监听 `http://localhost:8008`；（可选）Qdrant 运行在 `localhost:6334`
- **Docker 模式（`--docker`）**：本机已安装 Docker 与 Docker Compose，脚本会在项目根目录执行 `docker compose up -d`

### 11.5 输出示例

```
############################################################
# Adaptive Memory System - Shell Demo
############################################################

Base URL: http://localhost:8008

--- 1.1 Create session and add user message ---
✓ Success (HTTP 200)
{"sessionId":"...","messageId":"..."}

--- 2.2 Search knowledge entries (requires Qdrant) ---
! Search failed - Qdrant service may not be running
```

> 注意：如果 Qdrant 未启动，LTM 搜索会显示警告但脚本会继续执行。
