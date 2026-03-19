# Adaptive Memory SDKs

Official SDKs for integrating with the Adaptive Memory System.

## Python SDK

### Installation

```bash
pip install adaptive-memory
```

### Quick Start

```python
from adaptive_memory import MemoryClient

# Create client
client = MemoryClient("http://localhost:8008")

# Store in STM
result = client.store_stm(
    user_id="user1",
    agent_id="assistant",
    content="Hello, world!"
)
print(f"Session: {result['sessionId']}, Message: {result['messageId']}")

# Search in LTM
results = client.search_ltm("greeting")
for r in results.get("results", []):
    print(f"Found: {r['content']} (score: {r['score']})")

# Health check
health = client.health_check()
print(f"Status: {health}")
```

### Async Client

```python
import asyncio
from adaptive_memory import AsyncMemoryClient

async def main():
    client = AsyncMemoryClient("http://localhost:8008")

    # Async operations
    await client.store_stm("user1", "agent1", "Hello!")
    results = await client.search_ltm("greeting")

    await client.close()

asyncio.run(main())
```

## Rust SDK

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
adaptive-memory = "0.1"
```

### Quick Start

```rust
use adaptive_memory::{Client, models::*};

#[tokio::main]
async fn main() {
    let client = Client::new("http://localhost:8008");

    // Store in STM
    let req = StoreStmRequest {
        user_id: "user1".to_string(),
        agent_id: "assistant".to_string(),
        session_type: "default".to_string(),
        role: "user".to_string(),
        content: "Hello, world!".to_string(),
    };

    let result = client.store_stm(req).await.unwrap();
    println!("Session: {}", result.session_id);

    // Search in LTM
    let results = client.search_ltm("greeting", None, Some(10)).await.unwrap();
    for r in results {
        println!("Found: {} (score: {})", r.content, r.score);
    }
}
```

## API Reference

### MemoryClient Methods

| Method | Description |
|--------|-------------|
| `store_stm()` | Store content in Short-Term Memory |
| `store_ltm()` | Store content in Long-Term Memory |
| `search_stm()` | Search in Short-Term Memory |
| `search_ltm()` | Search in Long-Term Memory |
| `search_hybrid()` | Perform hybrid search |
| `list_sessions()` | List STM sessions |
| `list_ltm_entries()` | List LTM entries |
| `recall_session()` | Recall memories from session |
| `select_memory_config()` | Get optimal memory config |
| `health_check()` | Check API health |

### MCP Tools

The SDK also supports MCP (Model Context Protocol) tools:

```python
# Initialize MCP
client.initialize_mcp()

# List tools
tools = client.list_mcp_tools()

# Call a tool
result = client.call_mcp_tool("memory_search", {
    "query": "hello",
    "layer": "ltm",
    "limit": 10
})
```
