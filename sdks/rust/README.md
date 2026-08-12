# Adaptive Memory Rust SDK

Type-safe Rust client for the Adaptive Memory System.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
adaptive-memory = "0.1"
```

The crate re-exports its public API at the root, so `Client`, `AsyncClient`,
and the request/response types can all be imported directly from
`adaptive_memory`:

```rust
use adaptive_memory::{Client, AsyncClient, StoreStmRequest, StoreLtmRequest};
```

## Usage

The SDK exposes an async [`Client`](src/client.rs) backed by `reqwest`. All
methods are `async` and return `Result<T, adaptive_memory::client::Error>`.

```rust
use adaptive_memory::{Client, StoreStmRequest, StoreLtmRequest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::new("http://localhost:8008").with_api_key("your-api-key");

    // ── Store in STM ───────────────────────────────────────────────────────
    let stm = client.store_stm(StoreStmRequest {
        user_id: "user1".to_string(),
        agent_id: "assistant".to_string(),
        session_type: "conversation".to_string(),
        role: "user".to_string(),
        content: "Hello, world!".to_string(),
    }).await?;
    println!("session={} message={}", stm.session_id, stm.message_id);

    // ── Store in LTM ───────────────────────────────────────────────────────
    let ltm = client.store_ltm(StoreLtmRequest {
        source_id: "doc-1".to_string(),
        source_type: "user_input".to_string(),
        content: "User prefers concise technical answers".to_string(),
        title: Some("Style preference".to_string()),
    }).await?;
    println!("entry={}", ltm.entry_id);

    // ── Search LTM ─────────────────────────────────────────────────────────
    let results = client.search_ltm("concise", None, Some(10)).await?;
    for r in results {
        println!("- {} (score: {})", r.content, r.score);
    }

    // ── List STM sessions ──────────────────────────────────────────────────
    let response = client.list_sessions(Some("user1"), Some(10)).await?;
    println!("Found {} sessions (total: {})", response.sessions.len(), response.total);
    for s in response.sessions {
        println!("session={} status={}", s.session_id, s.status);
    }

    // ── Adaptive memory selection ──────────────────────────────────────────
    let config = client.select_memory_config("summarize a long document").await?;
    println!("config: {}", config);

    // ── Health check ───────────────────────────────────────────────────────
    let health = client.health_check().await?;
    println!("health: {}", health);

    Ok(())
}
```

## MCP Protocol

`Client` also exposes the MCP (Model Context Protocol) surface:

```rust
use adaptive_memory::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::new("http://localhost:8008");

    // Initialize (POST /api/initialize)
    let _info = client.initialize_mcp().await?;

    // List tools (GET /api/mcp/tools)
    let _tools = client.list_mcp_tools().await?;

    // Call a tool (POST /api/mcp/tools/call)
    let _result = client.call_mcp_tool("memory_search", None).await?;

    Ok(())
}
```

## AsyncClient

`AsyncClient` is a thin wrapper around `Client` that exposes a subset of the
storage and search methods: `store_stm`, `store_ltm`, `search_ltm`, and
`health_check`. MCP, session listing, and adaptive-selection methods are only
available on `Client`.

```rust
use adaptive_memory::{AsyncClient, StoreStmRequest, StoreLtmRequest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = AsyncClient::new("http://localhost:8008").with_api_key("your-api-key");

    let stm = client.store_stm(StoreStmRequest {
        user_id: "user1".to_string(),
        agent_id: "assistant".to_string(),
        session_type: "conversation".to_string(),
        role: "user".to_string(),
        content: "Hello!".to_string(),
    }).await?;
    println!("session={}", stm.session_id);

    let ltm = client.store_ltm(StoreLtmRequest {
        source_id: "doc-1".to_string(),
        source_type: "user_input".to_string(),
        content: "User prefers concise answers".to_string(),
        title: None,
    }).await?;
    println!("entry={}", ltm.entry_id);

    let results = client.search_ltm("concise", None, Some(10)).await?;
    for r in results {
        println!("- {} (score: {})", r.content, r.score);
    }

    let health = client.health_check().await?;
    println!("health: {}", health);

    Ok(())
}
```

## API Reference

### `Client`

| Method | Endpoint | Description |
|--------|----------|-------------|
| `new(base_url)` | — | Create a new client |
| `with_api_key(key)` | — | Attach a bearer token |
| `store_stm(req)` | `POST /api/v1/memory/storage/stm` | Store in STM |
| `store_ltm(req)` | `POST /api/v1/memory/storage/ltm` | Store in LTM |
| `search_ltm(query, user_id, limit)` | `POST /api/v1/memory/search/ltm` | Search LTM |
| `list_sessions(user_id, limit)` | `GET /api/v1/memory/storage/sessions` | List STM sessions → `SessionListResponse` |
| `select_memory_config(task_description)` | `POST /api/v1/memory/adaptive/select` | Select memory config |
| `health_check()` | `GET /api/v1/memory/health` | Server health |
| `initialize_mcp()` | `POST /api/initialize` | MCP initialize |
| `list_mcp_tools()` | `GET /api/mcp/tools` | List MCP tools |
| `call_mcp_tool(name, arguments)` | `POST /api/mcp/tools/call` | Call MCP tool |

### `AsyncClient`

| Method | Description |
|--------|-------------|
| `new(base_url)` | Create a new async client |
| `with_api_key(key)` | Attach a bearer token |
| `store_stm(req)` | Store in STM |
| `store_ltm(req)` | Store in LTM |
| `search_ltm(query, user_id, limit)` | Search LTM |
| `health_check()` | Server health |

Request and response types (`StoreStmRequest`, `StoreStmResponse`,
`StoreLtmRequest`, `StoreLtmResponse`, `SessionListResponse`, `SearchResult`,
`Session`, `LayerType`, …) are defined in [`models`](src/models.rs) and
re-exported at the crate root via `pub use models::*`, so
`use adaptive_memory::StoreStmRequest` resolves directly. The error enum is
[`client::Error`](src/client.rs) (path: `adaptive_memory::client::Error`).

## Features

- `default` — native-tls (OpenSSL) via `reqwest`'s default features
