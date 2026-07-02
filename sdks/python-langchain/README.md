# adaptive-memory-langchain

LangChain integration for [Aetheris MemOS](https://github.com/Colin4k1024/Aetheris-MemOS) Adaptive Memory System.

Provides LangChain-native interfaces so agents, chains, and RAG pipelines can use MemOS as a persistent memory backend.

## Installation

```bash
pip install adaptive-memory-langchain
```

## Quick Start

### Memory Tool for Agents

```python
from adaptive_memory import MemoryClient
from adaptive_memory_langchain import create_memory_tools

client = MemoryClient("http://localhost:8008", api_key="your-token")
tools = create_memory_tools(client=client, user_id="user-1", agent_id="agent-1")

# Use with any LangChain agent
from langchain.agents import AgentExecutor, create_tool_calling_agent
agent = create_tool_calling_agent(llm, tools, prompt)
```

### Retriever for RAG

```python
from adaptive_memory_langchain import AdaptiveMemoryRetriever

retriever = AdaptiveMemoryRetriever(
    client=client,
    search_type="hybrid",  # hybrid, ltm, triple, scored
    top_k=5,
    min_score=0.3,
)

# Use in a RAG chain
docs = retriever.invoke("What does the user prefer?")
```

### Chat Message History

```python
from adaptive_memory_langchain import AdaptiveMemoryChatMessageHistory

history = AdaptiveMemoryChatMessageHistory(
    client=client,
    user_id="user-1",
    agent_id="agent-1",
)

# Use with RunnableWithMessageHistory
from langchain_core.runnables.history import RunnableWithMessageHistory

def get_history(session_id):
    return AdaptiveMemoryChatMessageHistory(
        client=client, user_id="user-1", agent_id="agent-1", session_id=session_id
    )

with_history = RunnableWithMessageHistory(chain, get_history, ...)
```

## API Reference

### AdaptiveMemoryRetriever

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `client` | `MemoryClient` | required | Adaptive Memory client |
| `search_type` | `str` | `"hybrid"` | Search strategy: hybrid, ltm, triple, scored |
| `top_k` | `int` | `5` | Maximum documents to return |
| `min_score` | `float` | `0.0` | Minimum relevance score threshold |
| `user_id` | `str` | `None` | User ID for scoped search |

### AdaptiveMemoryChatMessageHistory

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `client` | `MemoryClient` | required | Adaptive Memory client |
| `user_id` | `str` | required | User identifier |
| `agent_id` | `str` | required | Agent identifier |
| `session_id` | `str` | `None` | Session ID (auto-created if not provided) |
| `session_type` | `str` | `"conversation"` | STM session type |

### AdaptiveMemoryTool

Unified tool with `action` parameter (remember/search/forget).

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `client` | `MemoryClient` | required | Adaptive Memory client |
| `user_id` | `str` | required | User identifier |
| `agent_id` | `str` | required | Agent identifier |

### create_memory_tools()

Factory returning three focused tools: `memory_store`, `memory_search`, `memory_forget`.

```python
tools = create_memory_tools(client, user_id="u1", agent_id="a1")
```

## Requirements

- Python >= 3.9
- `adaptive-memory >= 0.1.0`
- `langchain-core >= 0.2`

## Examples

See the `examples/` directory:
- `agent_with_memory.py` — Agent with memory tools
- `rag_with_retriever.py` — RAG pipeline with memory retriever
- `chat_with_history.py` — Conversational chain with persistent history
