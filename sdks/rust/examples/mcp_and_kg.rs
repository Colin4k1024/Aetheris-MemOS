//! MCP Protocol and Knowledge Graph Demo
//!
//! Demonstrates: MCP tool discovery and invocation via the SDK client, plus
//! Knowledge Graph entity/relation CRUD performed through raw HTTP calls.
//!
//! Run: cargo run --example mcp_and_kg

use adaptive_memory::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let base_url = "http://localhost:8008";
    let sdk = Client::new(base_url);

    // ── 1. Initialize MCP ──────────────────────────────────────────────────
    println!("=== MCP Initialization ===");
    match sdk.initialize_mcp().await {
        Ok(result) => println!("MCP initialized: {result}"),
        Err(e) => println!("MCP init failed (server may be offline): {e}"),
    }

    // ── 2. List MCP tools ─────────────────────────────────────────────────
    println!("\n=== MCP Tool Discovery ===");
    match sdk.list_mcp_tools().await {
        Ok(tools) => {
            // The response contains a "tools" array; extract names if present.
            let names: Vec<&str> = tools
                .get("tools")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
                        .collect()
                })
                .unwrap_or_default();

            if names.is_empty() {
                println!("Available tools (raw): {tools}");
            } else {
                println!("Available MCP tools ({}):", names.len());
                for name in &names {
                    println!("  - {name}");
                }
            }
        }
        Err(e) => println!("List tools failed: {e}"),
    }

    // ── 3. Call memory_write via MCP ─────────────────────────────────────
    println!("\n=== MCP Tool Call: memory_write ===");
    let write_args = json!({
        "content": "Rust async/await is built on top of the Future trait. \
                    The tokio runtime drives futures to completion.",
        "source_type": "document",
        "title": "Rust Async Fundamentals"
    });

    match sdk.call_mcp_tool("memory_write", Some(write_args)).await {
        Ok(result) => println!("memory_write result: {result}"),
        Err(e) => println!("memory_write failed: {e}"),
    }

    // ── 4. Call memory_search via MCP ────────────────────────────────────
    println!("\n=== MCP Tool Call: memory_search ===");
    let search_args = json!({
        "query": "Rust async Future tokio",
        "limit": 3
    });

    match sdk.call_mcp_tool("memory_search", Some(search_args)).await {
        Ok(result) => println!("memory_search result: {result}"),
        Err(e) => println!("memory_search failed: {e}"),
    }

    // ── 5. KG entity and relation CRUD via raw HTTP ───────────────────────
    // The SDK does not yet expose KG endpoints directly, so we use
    // reqwest::Client for these calls.
    println!("\n=== Knowledge Graph (raw HTTP) ===");
    let http = reqwest::Client::new();

    // Create entity 1: Rust programming language
    println!("Creating entity: Rust");
    let entity1_payload = json!({
        "name": "Rust",
        "entity_type": "ProgrammingLanguage",
        "properties": {
            "paradigm": "systems, functional, concurrent",
            "memory_model": "ownership",
            "first_appeared": "2010"
        }
    });
    let entity1_resp = http
        .post(format!("{base_url}/api/kg/entities"))
        .json(&entity1_payload)
        .send()
        .await?;
    let entity1_status = entity1_resp.status();
    let entity1_body: serde_json::Value = entity1_resp.json().await?;
    println!("  status={entity1_status}  body={entity1_body}");
    let entity1_id = entity1_body
        .get("id")
        .or_else(|| entity1_body.get("entity_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("rust-entity")
        .to_string();

    // Create entity 2: Tokio async runtime
    println!("Creating entity: Tokio");
    let entity2_payload = json!({
        "name": "Tokio",
        "entity_type": "Library",
        "properties": {
            "language": "Rust",
            "purpose": "async runtime",
            "license": "MIT"
        }
    });
    let entity2_resp = http
        .post(format!("{base_url}/api/kg/entities"))
        .json(&entity2_payload)
        .send()
        .await?;
    let entity2_status = entity2_resp.status();
    let entity2_body: serde_json::Value = entity2_resp.json().await?;
    println!("  status={entity2_status}  body={entity2_body}");
    let entity2_id = entity2_body
        .get("id")
        .or_else(|| entity2_body.get("entity_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("tokio-entity")
        .to_string();

    // Create relation: Tokio "implements_runtime_for" Rust
    println!("Creating relation: Tokio -[implements_runtime_for]-> Rust");
    let relation_payload = json!({
        "source_id": entity2_id,
        "target_id": entity1_id,
        "relation_type": "implements_runtime_for",
        "properties": {
            "since": "2019",
            "stability": "stable"
        }
    });
    let relation_resp = http
        .post(format!("{base_url}/api/kg/relations"))
        .json(&relation_payload)
        .send()
        .await?;
    let relation_status = relation_resp.status();
    let relation_body: serde_json::Value = relation_resp.json().await?;
    println!("  status={relation_status}  body={relation_body}");

    // List entities
    println!("\nListing KG entities:");
    let list_resp = http
        .get(format!("{base_url}/api/kg/entities"))
        .send()
        .await?;
    let list_status = list_resp.status();
    let list_body: serde_json::Value = list_resp.json().await?;
    println!("  status={list_status}");

    // Print entity names if we can parse them
    let entities = list_body
        .as_array()
        .cloned()
        .or_else(|| {
            list_body
                .get("entities")
                .and_then(|e| e.as_array())
                .cloned()
        })
        .unwrap_or_default();

    if entities.is_empty() {
        println!("  (raw response) {list_body}");
    } else {
        println!("  Found {} entity/entities:", entities.len());
        for entity in &entities {
            let name = entity
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("<unnamed>");
            let etype = entity
                .get("entity_type")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown");
            println!("    [{etype}] {name}");
        }
    }

    // ── Summary ────────────────────────────────────────────────────────────
    println!("\n=== Summary ===");
    println!("  MCP tools discovered and invoked (memory_write, memory_search)");
    println!("  KG entity 1 created : Rust (id={entity1_id})");
    println!("  KG entity 2 created : Tokio (id={entity2_id})");
    println!("  KG relation created : implements_runtime_for");
    println!("  KG entity list      : {} total", entities.len());
    println!("\nMCP + Knowledge Graph demo completed.");

    Ok(())
}
