# Adaptive Memory Rust SDK

Type-safe Rust client for Adaptive Memory System.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
adaptive-memory = "0.1"
```

## Usage

```rust
use adaptive_memory::{Config, MemoryClient, Layer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new("http://localhost:8008")
        .with_api_key("your-api-key");

    let client = MemoryClient::new(config)?;

    // Write memory
    let result = client.write("Important information", Layer::Stm)?;
    println!("Stored: {:?}", result);

    // Search memory
    let results = client.search("information", Layer::Ltm, 10)?;
    for r in results {
        println!("- {}", r.content.unwrap_or_default());
    }

    Ok(())
}
```

## Async Usage

```rust
use adaptive_memory::{Config, AsyncMemoryClient, Layer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new("http://localhost:8008");

    let client = AsyncMemoryClient::new(config).await?;

    let result = client.write("Important info", Layer::Stm).await?;
    println!("Stored: {:?}", result);

    Ok(())
}
```

## Modules

- `memory` - Memory read/write/search operations
- `agent` - Agent management
- `knowledge_graph` - Knowledge graph operations
- `mcp` - MCP protocol client

## Features

- `default` - Uses native-tls (OpenSSL)
- `rustls-tls` - Uses rustls instead
