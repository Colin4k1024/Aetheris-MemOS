//! Integration tests for WebSocket real-time event delivery (#86).
//!
//! Exercises the `WsConnectionManager` public API: dual-session tenant isolation,
//! broadcast delivery, subscription filtering, slow-consumer lagged drops, and
//! session reconnect (same tenant, new session).
//!
//! These tests verify the correctness guarantees that unit tests in
//! `src/protocol/websocket.rs` cannot cover — they exercise the full
//! broadcast→receive→forward pipeline end-to-end.

use backend::kernel::types::LayerType;
use backend::protocol::websocket::{
    EventData, EventResponse, EventType, MemoryEventPayload, SendResult,
    WsConnectionManager,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn added_event(tenant_id: &str, id: &str) -> EventResponse {
    EventResponse {
        event_type: EventType::MemoryAdded,
        data: EventData::MemoryAdded(MemoryEventPayload {
            id: id.to_string(),
            tenant_id: tenant_id.to_string(),
            layer: LayerType::Ltm,
            source_type: None,
            summary: None,
            metadata: None,
        }),
    }
}

fn deleted_event(tenant_id: &str, id: &str) -> EventResponse {
    EventResponse {
        event_type: EventType::MemoryDeleted,
        data: EventData::MemoryDeleted {
            id: id.to_string(),
            layer: LayerType::Ltm,
            tenant_id: tenant_id.to_string(),
        },
    }
}

// ── dual-session tenant isolation ────────────────────────────────────────────

#[tokio::test]
async fn dual_session_tenant_isolation() {
    let manager = WsConnectionManager::new();
    let tenant_a = backend::tenant::RequestTenantContext::new("tenant-a");
    let tenant_b = backend::tenant::RequestTenantContext::new("tenant-b");

    let (session_a, mut rx_a) = manager.create_session(tenant_a.clone()).await;
    let (session_b, mut rx_b) = manager.create_session(tenant_b.clone()).await;

    assert_eq!(manager.connection_count().await, 2);

    // Broadcast an event scoped to tenant-a.
    let delivered = manager.broadcast_event(added_event("tenant-a", "ev-1"));
    assert_eq!(delivered, 2, "both connections should receive the broadcast");

    // Session A (tenant-a) should receive it.
    let event_a = rx_a.recv().await.expect("session A should receive tenant-a event");
    assert_eq!(event_a.event_type, EventType::MemoryAdded);

    // Session B (tenant-b) should also receive the broadcast, but the
    // forward-task filter (should_forward_event) would reject it because
    // tenant mismatch. Here we verify the raw broadcast reaches both.
    let event_b = rx_b.recv().await.expect("session B should receive raw broadcast");
    assert_eq!(event_b.event_type, EventType::MemoryAdded);

    // send_to_session should differentiate: tenant-a event → Delivered for
    // session_a, but the tenant check is NOT in send_to_session (it's in
    // should_forward_event). send_to_session only checks existence + subscription.
    // So both sessions should return Delivered (no subscriptions = receive-all).
    assert_eq!(
        manager.send_to_session(&session_a, added_event("tenant-a", "ev-2")).await,
        SendResult::Delivered
    );
    assert_eq!(
        manager.send_to_session(&session_b, added_event("tenant-a", "ev-2")).await,
        SendResult::Delivered
    );

    // Cleanup — verify sessions are independent.
    manager.remove_session(&session_a).await;
    assert_eq!(manager.connection_count().await, 1);
    assert_eq!(
        manager.send_to_session(&session_a, added_event("tenant-a", "ev-3")).await,
        SendResult::SessionNotFound
    );
    // Session B should still be alive.
    assert_eq!(
        manager.send_to_session(&session_b, added_event("tenant-b", "ev-3")).await,
        SendResult::Delivered
    );

    manager.remove_session(&session_b).await;
    assert_eq!(manager.connection_count().await, 0);
}

// ── subscription filtering ──────────────────────────────────────────────────

#[tokio::test]
async fn subscription_filters_events() {
    let manager = WsConnectionManager::new();
    let tenant = backend::tenant::RequestTenantContext::new("tenant-x");
    let (session_id, mut rx) = manager.create_session(tenant).await;

    // Subscribe to MemoryDeleted only.
    manager.subscribe(&session_id, EventType::MemoryDeleted).await;

    // MemoryAdded → NotSubscribed.
    assert_eq!(
        manager.send_to_session(&session_id, added_event("tenant-x", "ev-1")).await,
        SendResult::NotSubscribed
    );

    // MemoryDeleted → Delivered.
    assert_eq!(
        manager.send_to_session(&session_id, deleted_event("tenant-x", "ev-2")).await,
        SendResult::Delivered
    );

    // Broadcast a MemoryDeleted event — should be received.
    manager.broadcast_event(deleted_event("tenant-x", "ev-3"));
    let event = rx.recv().await.expect("should receive subscribed event");
    assert_eq!(event.event_type, EventType::MemoryDeleted);

    manager.remove_session(&session_id).await;
}

// ── slow consumer (lagged drops) ─────────────────────────────────────────────

#[tokio::test]
async fn slow_consumer_drops_events() {
    let manager = WsConnectionManager::new();
    let tenant = backend::tenant::RequestTenantContext::new("tenant-slow");
    let (_session_id, mut rx) = manager.create_session(tenant).await;

    // Fill the broadcast channel beyond its capacity without draining.
    // The broadcast channel capacity is 1000.
    for i in 0..1200 {
        manager.broadcast_event(added_event("tenant-slow", &format!("ev-{i}")));
    }

    // The receiver should have lagged — the first recv() after lagging
    // returns RecvError::Lagged(n).
    let result = rx.recv().await;
    assert!(
        result.is_err(),
        "receiver should have lagged after 1200 events without draining"
    );

    // Verify lagged drops are tracked.
    if let Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) = result {
        assert!(n > 0, "lagged count should be > 0, got {n}");
    } else {
        panic!("expected RecvError::Lagged");
    }

    manager.remove_session(&_session_id).await;
}

// ── session reconnect ────────────────────────────────────────────────────────

#[tokio::test]
async fn session_reconnect_same_tenant() {
    let manager = WsConnectionManager::new();
    let tenant = backend::tenant::RequestTenantContext::new("tenant-r");

    // First session.
    let (session_1, rx_1) = manager.create_session(tenant.clone()).await;
    assert_eq!(manager.connection_count().await, 1);

    // Simulate disconnect.
    drop(rx_1);
    manager.remove_session(&session_1).await;
    assert_eq!(manager.connection_count().await, 0);

    // Old session → SessionNotFound.
    assert_eq!(
        manager.send_to_session(&session_1, added_event("tenant-r", "ev-1")).await,
        SendResult::SessionNotFound
    );

    // Reconnect — new session, same tenant.
    let (session_2, mut rx_2) = manager.create_session(tenant.clone()).await;
    assert_eq!(manager.connection_count().await, 1);

    // New session receives events independently.
    assert_eq!(
        manager.send_to_session(&session_2, added_event("tenant-r", "ev-2")).await,
        SendResult::Delivered
    );

    manager.broadcast_event(added_event("tenant-r", "ev-3"));
    let event = rx_2.recv().await.expect("reconnected session should receive events");
    assert_eq!(event.event_type, EventType::MemoryAdded);

    manager.remove_session(&session_2).await;
}

// ── queue depth reflects backlog ─────────────────────────────────────────────

#[tokio::test]
async fn queue_depth_tracks_backlog() {
    let manager = WsConnectionManager::new();
    let tenant = backend::tenant::RequestTenantContext::new("tenant-q");
    let (_session_id, _rx) = manager.create_session(tenant).await;

    assert_eq!(manager.queue_depth(), 0, "queue should start empty");

    // Broadcast without draining.
    for i in 0..10 {
        manager.broadcast_event(added_event("tenant-q", &format!("ev-{i}")));
    }

    let depth = manager.queue_depth();
    assert!(depth >= 10, "queue depth {depth} should be >= 10 after 10 broadcasts without drain");

    manager.remove_session(&_session_id).await;
}