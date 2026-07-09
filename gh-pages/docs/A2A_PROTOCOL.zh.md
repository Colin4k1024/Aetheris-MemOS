# A2A 协议集成

## 概述

Aetheris MemOS 支持 [Agent2Agent (A2A) 协议](https://a2a-protocol.org)，这是一个开放标准，用于AI代理之间的通信和互操作。这允许其他代理使用标准化协议发现和与MemOS的记忆功能进行交互。

## 代理发现

### 代理卡片

代理卡片描述了MemOS的功能，可以通过以下方式访问：

```
GET /.well-known/agent-card.json
```

示例：
```bash
curl http://localhost:8008/.well-known/agent-card.json
```

响应：
```json
{
  "name": "Aetheris MemOS",
  "description": "自适应记忆管理系统...",
  "version": "1.0.0",
  "capabilities": {
    "streaming": true,
    "push_notifications": false
  },
  "skills": [
    {
      "id": "memory_search",
      "name": "记忆搜索",
      "description": "跨所有记忆层（STM、LTM、KG、MM）进行混合检索搜索"
    },
    {
      "id": "memory_store",
      "name": "记忆存储",
      "description": "在STM或LTM记忆层中存储信息"
    },
    {
      "id": "memory_fusion",
      "name": "记忆融合",
      "description": "跨所有记忆层进行统一融合查询"
    },
    {
      "id": "memory_status",
      "name": "记忆状态",
      "description": "获取记忆系统的健康状态和统计信息"
    },
    {
      "id": "knowledge_graph",
      "name": "知识图谱",
      "description": "查询和管理知识图谱实体和关系"
    }
  ]
}
```

## 协议绑定

MemOS支持两种A2A协议绑定：

### JSON-RPC 2.0

端点：`POST /a2a/jsonrpc`

#### 发送消息

```bash
curl -X POST http://localhost:8008/a2a/jsonrpc \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "message/send",
    "params": {
      "message": {
        "messageId": "msg-1",
        "role": "ROLE_USER",
        "parts": [
          {
            "text": "搜索关于机器学习的记忆"
          }
        ]
      }
    },
    "id": 1
  }'
```

### REST / HTTP+JSON

#### 发送消息

端点：`POST /a2a/rest/messages`

```bash
curl -X POST http://localhost:8008/a2a/rest/messages \
  -H "Content-Type: application/json" \
  -d '{
    "message": {
      "messageId": "msg-2",
      "role": "ROLE_USER",
      "parts": [
        {
          "text": "记住这个重要事实"
        }
      ]
    }
  }'
```

#### 流式消息 (SSE)

端点：`POST /a2a/rest/messages/stream`

```bash
curl -X POST http://localhost:8008/a2a/rest/messages/stream \
  -H "Content-Type: application/json" \
  -H "Accept: text/event-stream" \
  -d '{
    "message": {
      "messageId": "msg-3",
      "role": "ROLE_USER",
      "parts": [
        {
          "text": "搜索关于AI的记忆"
        }
      ]
    }
  }'
```

响应（服务器发送事件）：
```
data: {"taskId":"...","contextId":"...","status":{"state":"working","timestamp":"..."},"final":false}

data: {"taskId":"...","contextId":"...","status":{"state":"completed","timestamp":"..."},"final":true}
```

## 技能参考

### memory_search

跨所有记忆层进行混合检索搜索。

示例消息：
```
搜索关于机器学习算法的记忆
```

### memory_store

在短期或长期记忆中存储信息。

示例消息：
```
记住这个事实：A2A协议实现了代理互操作性
```

### memory_fusion

跨所有记忆层进行统一融合查询。

示例消息：
```
从所有记忆层获取关于这个主题的全面上下文
```

### memory_status

获取记忆系统的健康状态和统计信息。

示例消息：
```
检查记忆系统健康状态
```

### knowledge_graph

查询和管理知识图谱实体和关系。

示例消息：
```
查找与人工智能相关的实体
```

## 集成示例

### Python (使用 requests)

```python
import requests

# 通过REST发送消息
response = requests.post(
    "http://localhost:8008/a2a/rest/messages",
    json={
        "message": {
            "messageId": "python-msg-1",
            "role": "ROLE_USER",
            "parts": [{"text": "搜索关于Python的记忆"}]
        }
    }
)

result = response.json()
print(result)
```

### JavaScript (使用 fetch)

```javascript
// 通过REST发送消息
const response = await fetch('http://localhost:8008/a2a/rest/messages', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    message: {
      messageId: 'js-msg-1',
      role: 'ROLE_USER',
      parts: [{ text: '搜索关于JavaScript的记忆' }]
    }
  })
});

const result = await response.json();
console.log(result);
```

## 错误处理

### JSON-RPC 错误代码

| 代码 | 消息 | 描述 |
|------|------|------|
| -32600 | Invalid Request | 无效的JSON-RPC请求 |
| -32601 | Method not found | 方法不存在 |
| -32603 | Internal error | 服务器端错误 |

## 参考资料

- [A2A协议规范](https://a2a-protocol.org/latest/specification/)
- [A2A Rust SDK](https://github.com/a2aproject/a2a-rs)
- [A2A示例](https://github.com/a2aproject/a2a-samples)
