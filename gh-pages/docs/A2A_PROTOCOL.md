# A2A Protocol Integration

## Overview

Aetheris MemOS supports the [Agent2Agent (A2A) Protocol](https://a2a-protocol.org), an open standard enabling communication and interoperability between AI agents. This allows other agents to discover and interact with MemOS's memory capabilities using a standardized protocol.

## Agent Discovery

### Agent Card

The Agent Card describes MemOS's capabilities and can be accessed at:

```
GET /.well-known/agent-card.json
```

Example:
```bash
curl http://localhost:8008/.well-known/agent-card.json
```

Response:
```json
{
  "name": "Aetheris MemOS",
  "description": "Adaptive Memory Management System for AI Agents...",
  "version": "1.0.0",
  "capabilities": {
    "streaming": true,
    "push_notifications": false
  },
  "skills": [
    {
      "id": "memory_search",
      "name": "Memory Search",
      "description": "Search across all memory layers (STM, LTM, KG, MM) with hybrid retrieval",
      "tags": ["memory", "search", "hybrid"]
    },
    {
      "id": "memory_store",
      "name": "Memory Store",
      "description": "Store information in STM or LTM memory layers",
      "tags": ["memory", "store", "stm", "ltm"]
    },
    {
      "id": "memory_fusion",
      "name": "Memory Fusion",
      "description": "Query across all memory layers with unified fusion results",
      "tags": ["memory", "fusion", "multi-layer"]
    },
    {
      "id": "memory_status",
      "name": "Memory Status",
      "description": "Get health status and statistics of memory system",
      "tags": ["memory", "status", "health"]
    },
    {
      "id": "knowledge_graph",
      "name": "Knowledge Graph",
      "description": "Query and manage knowledge graph entities and relations",
      "tags": ["knowledge", "graph", "entities"]
    }
  ],
  "supported_interfaces": [
    {
      "url": "http://localhost:8008/a2a/jsonrpc",
      "protocol_binding": "JSONRPC"
    },
    {
      "url": "http://localhost:8008/a2a/rest",
      "protocol_binding": "HTTP+JSON"
    }
  ]
}
```

## Protocol Bindings

MemOS supports two A2A protocol bindings:

### JSON-RPC 2.0

Endpoint: `POST /a2a/jsonrpc`

#### Send Message

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
            "text": "Search for memories about machine learning"
          }
        ]
      }
    },
    "id": 1
  }'
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "task": {
      "id": "task-uuid",
      "contextId": "context-uuid",
      "status": {
        "state": "TASK_STATE_COMPLETED",
        "timestamp": "2026-07-09T12:00:00Z"
      },
      "history": [...]
    }
  },
  "id": 1
}
```

#### Get Task

```bash
curl -X POST http://localhost:8008/a2a/jsonrpc \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "task/get",
    "params": {
      "id": "task-uuid"
    },
    "id": 2
  }'
```

### REST / HTTP+JSON

#### Send Message

Endpoint: `POST /a2a/rest/messages`

```bash
curl -X POST http://localhost:8008/a2a/rest/messages \
  -H "Content-Type: application/json" \
  -d '{
    "message": {
      "messageId": "msg-2",
      "role": "ROLE_USER",
      "parts": [
        {
          "text": "Remember this important fact"
        }
      ]
    }
  }'
```

#### Streaming Messages (SSE)

Endpoint: `POST /a2a/rest/messages/stream`

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
          "text": "Search for memories about AI"
        }
      ]
    }
  }'
```

Response (Server-Sent Events):
```
data: {"taskId":"...","contextId":"...","status":{"state":"working","timestamp":"..."},"final":false}

data: {"taskId":"...","contextId":"...","status":{"state":"completed","timestamp":"..."},"final":true}
```

#### Get Task

Endpoint: `GET /a2a/rest/tasks/{task_id}`

```bash
curl http://localhost:8008/a2a/rest/tasks/task-uuid
```

#### List Tasks

Endpoint: `GET /a2a/rest/tasks`

```bash
curl http://localhost:8008/a2a/rest/tasks
```

## Skills Reference

### memory_search

Search across all memory layers with hybrid retrieval.

Example message:
```
Search for memories about machine learning algorithms
```

### memory_store

Store information in short-term or long-term memory.

Example message:
```
Remember this fact: The A2A protocol enables agent interoperability
```

### memory_fusion

Query across all memory layers with unified fusion results.

Example message:
```
Get comprehensive context about this topic from all memory layers
```

### memory_status

Get health status and statistics of the memory system.

Example message:
```
Check memory system health status
```

### knowledge_graph

Query and manage knowledge graph entities and relations.

Example message:
```
Find entities related to artificial intelligence
```

## Message Format

### Message Structure

```json
{
  "messageId": "unique-message-id",
  "contextId": "optional-context-id",
  "taskId": "optional-task-id",
  "role": "ROLE_USER | ROLE_AGENT",
  "parts": [
    {
      "text": "message content"
    }
  ],
  "metadata": {}
}
```

### Part Types

| Type | Description | Example |
|------|-------------|---------|
| `text` | Plain text content | `{"text": "Hello"}` |
| `data` | Structured JSON data | `{"data": {"key": "value"}}` |
| `url` | URL reference | `{"url": "https://example.com"}` |

### Task States

| State | Description |
|-------|-------------|
| `TASK_STATE_UNSPECIFIED` | Default state |
| `TASK_STATE_SUBMITTED` | Task submitted |
| `TASK_STATE_WORKING` | Task in progress |
| `TASK_STATE_COMPLETED` | Task completed |
| `TASK_STATE_FAILED` | Task failed |
| `TASK_STATE_CANCELED` | Task canceled |

## Integration Examples

### Python (using requests)

```python
import requests

# Send message via REST
response = requests.post(
    "http://localhost:8008/a2a/rest/messages",
    json={
        "message": {
            "messageId": "python-msg-1",
            "role": "ROLE_USER",
            "parts": [{"text": "Search for memories about Python"}]
        }
    }
)

result = response.json()
print(result)
```

### JavaScript (using fetch)

```javascript
// Send message via REST
const response = await fetch('http://localhost:8008/a2a/rest/messages', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    message: {
      messageId: 'js-msg-1',
      role: 'ROLE_USER',
      parts: [{ text: 'Search for memories about JavaScript' }]
    }
  })
});

const result = await response.json();
console.log(result);
```

### Streaming with JavaScript

```javascript
// Stream messages using EventSource
const eventSource = new EventSource('http://localhost:8008/a2a/rest/messages/stream', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    message: {
      messageId: 'js-stream-1',
      role: 'ROLE_USER',
      parts: [{ text: 'Search for memories' }]
    }
  })
});

eventSource.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Status update:', data);
};
```

## Error Handling

### JSON-RPC Error Codes

| Code | Message | Description |
|------|---------|-------------|
| -32600 | Invalid Request | Invalid JSON-RPC request |
| -32601 | Method not found | Method does not exist |
| -32603 | Internal error | Server-side error |

### Error Response Example

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32601,
    "message": "Method not found"
  },
  "id": 1
}
```

## Security

Currently, the A2A endpoints do not require authentication. For production deployments, consider:

1. Adding JWT authentication
2. Implementing rate limiting
3. Validating message content
4. Restricting access to trusted agents

## References

- [A2A Protocol Specification](https://a2a-protocol.org/latest/specification/)
- [A2A Rust SDK](https://github.com/a2aproject/a2a-rs)
- [A2A Samples](https://github.com/a2aproject/a2a-samples)
