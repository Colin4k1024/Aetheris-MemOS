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

# Write memory
result = client.memory.write(
    content="Important information",
    layer="stm"  # stm, ltm, kg, mm
)
print(f"Stored in {result['layer']}: {result.get('sessionId') or result.get('entryId')}")

# Search memory
results = client.memory.search(
    query="information",
    layer="ltm",
    limit=10
)
for r in results:
    print(f"- {r['content'][:100]}...")

# List memories
sessions = client.memory.list(layer="stm", limit=20)
```

## API Reference

### MemoryClient

Main client for interacting with Adaptive Memory System.

#### Parameters
- `base_url` (str): Base URL of the memory server
- `api_key` (str, optional): API key for authentication
- `timeout` (int, optional): Request timeout in seconds (default: 30)

### Memory API

#### write(content, layer, **kwargs)
Write memory to specified layer.

#### search(query, layer, limit)
Search memory in specified layer.

#### recall(session_id, limit)
Recall memories from a session.

#### forget(memory_id, layer)
Delete memory from specified layer.

#### list(layer, limit, offset)
List memories in specified layer.

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
        result = await client.memory.write(content="test", layer="stm")
        print(result)

asyncio.run(main())
```
