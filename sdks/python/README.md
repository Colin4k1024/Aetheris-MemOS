# Adaptive Memory Python SDK

Python SDK for integrating Adaptive Memory System into applications.

## Installation

```bash
pip install adaptive-memory
```

## Quick Start

```python
from adaptive_memory import MemoryClient

# Initialize client
client = MemoryClient(
    base_url="http://localhost:8008",
    api_key="your-api-key"  # Optional
)

# Remember memory
result = client.remember(
    content="User prefers concise technical answers",
    user_id="user-1",
    agent_id="agent-1",
    session_id="session-1",
    layer="stm"  # stm, ltm, kg, mm
)
print(result)

# Recall task context
context = client.recall(
    query="How should I answer this user?",
    user_id="user-1",
    agent_id="agent-1",
    session_id=result.get("sessionId"),
    limit=5,
)
print(context)

# Search memory directly
results = client.search(
    query="concise technical answers",
    layer="hybrid",
    user_id="user-1",
    limit=10
)
print(results)
```

## API Reference

### MemoryClient

Main client for interacting with Adaptive Memory System.

#### Parameters
- `base_url` (str): Base URL of the memory server
- `api_key` (str, optional): API key for authentication
- `timeout` (int, optional): Request timeout in seconds (default: 30)

### Agent Memory Contract

#### remember(content, user_id, agent_id, session_id=None, layer="stm", importance=None, metadata=None)
Store a memory through the stable agent-facing contract.

#### recall(query, user_id, agent_id, session_id=None, limit=10)
Recall relevant context for a task.

#### search(query, layer="hybrid", user_id=None, limit=10)
Search memory in STM, LTM, hybrid, KG, or MM layers.

#### forget(memory_id, layer="ltm")
Forget or invalidate a memory where supported by the server.

#### explain(trace_id=None, task_id=None, limit=20)
Fetch decision traces for memory selection explainability.

#### feedback(memory_id, useful, query=None, trace_id=None, metadata=None)
Record retrieval feedback using the agent-facing contract.

### Low-level Memory API

#### store_stm(user_id, agent_id, content, session_type="default", role="user")
Store content in short-term memory.

#### store_ltm(source_id, source_type, content, title=None)
Store content in long-term memory.

#### search_stm(query, user_id=None, limit=10)
Search short-term memory.

#### search_ltm(query, user_id=None, limit=10)
Search long-term memory.

#### search_hybrid(query, user_id=None, limit=10)
Run hybrid retrieval.

### Knowledge Graph API

#### kg.create_entity(name, entity_type, description)
Create a knowledge graph entity.

#### kg.list_entities(entity_type, limit)
List knowledge graph entities.

#### kg.get_related(entity_id)
Get related entities.

### Agent API

#### agents.list(limit, offset)
List all agents.

#### agents.get(agent_id)
Get agent details.

#### agents.create(name, description)
Create new agent.

## Async Usage

```python
import asyncio
from adaptive_memory import AsyncMemoryClient

async def main():
    async with AsyncMemoryClient("http://localhost:8008") as client:
        result = await client.remember(
            content="test",
            user_id="user-1",
            agent_id="agent-1",
            session_id="session-1",
        )
        print(result)

asyncio.run(main())
```
