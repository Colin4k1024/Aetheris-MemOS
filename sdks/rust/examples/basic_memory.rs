//! Basic Memory Operations with the Adaptive Memory Rust SDK
//!
//! Demonstrates: health check, STM storage, LTM storage, semantic search,
//! session listing, and adaptive memory config selection.
//!
//! Run: cargo run --example basic_memory

use adaptive_memory::{Client, StoreLtmRequest, StoreStmRequest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let base_url = "http://localhost:8008";
    let client = Client::new(base_url);

    // ── 1. Health check ────────────────────────────────────────────────────
    println!("=== Health Check ===");
    match client.health_check().await {
        Ok(health) => println!("Status: {}", health),
        Err(e) => println!("Health check failed (server may be offline): {e}"),
    }

    // ── 2. Store STM messages ──────────────────────────────────────────────
    println!("\n=== Short-Term Memory (STM) ===");

    // First message — creates the session
    let first_req = StoreStmRequest {
        user_id: "user-001".to_string(),
        agent_id: "agent-rust-demo".to_string(),
        session_type: "chat".to_string(),
        role: "user".to_string(),
        content: "Hello! Can you help me learn Rust?".to_string(),
    };
    let first_resp = client.store_stm(first_req).await?;
    let session_id = first_resp.session_id.clone();
    println!(
        "Stored message 1 — session_id: {}, message_id: {}",
        first_resp.session_id, first_resp.message_id
    );

    // Second message — agent reply in the same session
    let second_req = StoreStmRequest {
        user_id: "user-001".to_string(),
        agent_id: "agent-rust-demo".to_string(),
        session_type: "chat".to_string(),
        role: "assistant".to_string(),
        content: "Of course! Rust is a systems programming language focused on safety and performance. Where would you like to start?".to_string(),
    };
    let second_resp = client.store_stm(second_req).await?;
    println!(
        "Stored message 2 — session_id: {}, message_id: {}",
        second_resp.session_id, second_resp.message_id
    );

    // Third message — user follow-up
    let third_req = StoreStmRequest {
        user_id: "user-001".to_string(),
        agent_id: "agent-rust-demo".to_string(),
        session_type: "chat".to_string(),
        role: "user".to_string(),
        content: "Let's start with ownership and borrowing concepts.".to_string(),
    };
    let third_resp = client.store_stm(third_req).await?;
    println!(
        "Stored message 3 — session_id: {}, message_id: {}",
        third_resp.session_id, third_resp.message_id
    );

    println!("Active session_id: {session_id}");

    // ── 3. Store LTM entry ─────────────────────────────────────────────────
    println!("\n=== Long-Term Memory (LTM) ===");

    let ltm_req = StoreLtmRequest {
        source_id: "doc-rust-ownership-001".to_string(),
        source_type: "documentation".to_string(),
        content: "Rust ownership rules: (1) Each value has a single owner. \
                  (2) When the owner goes out of scope the value is dropped. \
                  (3) There can be either one mutable reference or any number \
                  of immutable references at the same time — never both."
            .to_string(),
        title: Some("Rust Ownership and Borrowing".to_string()),
    };

    let ltm_resp = client.store_ltm(ltm_req).await?;
    println!("Stored LTM entry — entry_id: {}", ltm_resp.entry_id);

    // ── 4. Search LTM ──────────────────────────────────────────────────────
    println!("\n=== LTM Semantic Search ===");

    let results = client
        .search_ltm("Rust ownership borrowing", Some("user-001"), Some(5))
        .await?;

    if results.is_empty() {
        println!("No results found (index may still be building).");
    } else {
        println!("Found {} result(s):", results.len());
        for (i, r) in results.iter().enumerate() {
            let preview: String = r.content.chars().take(80).collect();
            println!("  [{i}] score={:.4}  content=\"{preview}...\"", r.score);
        }
    }

    // ── 5. List sessions ───────────────────────────────────────────────────
    println!("\n=== Session List ===");

    let response = client.list_sessions(Some("user-001"), Some(10)).await?;
    println!(
        "Sessions for user-001: {} found (total: {})",
        response.sessions.len(),
        response.total
    );
    for s in &response.sessions {
        println!("  session_id: {}  status: {}", s.session_id, s.status);
    }

    // ── 6. Adaptive memory config ──────────────────────────────────────────
    println!("\n=== Adaptive Memory Config Selection ===");

    let task = "Multi-step Rust coding tutorial with code examples and explanations";
    match client.select_memory_config(task).await {
        Ok(config) => println!("Recommended config: {config}"),
        Err(e) => println!("Config selection failed: {e}"),
    }

    // ── Summary ────────────────────────────────────────────────────────────
    println!("\n=== Summary ===");
    println!("  STM session created : {session_id}");
    println!("  LTM entry stored    : {}", ltm_resp.entry_id);
    println!("  LTM search results  : {}", results.len());
    println!("  Active sessions     : {}", response.sessions.len());
    println!("\nBasic memory operations demo completed.");

    Ok(())
}
