//! WebSocket Protocol — P2 implementation (ADR-0007).
//!
//! Message types + an in-memory connection manager. The `WsConnection` now
//! binds `RequestTenantContext` so all frames are tenant-scoped. An axum
//! WebSocket upgrade handler (`ws_upgrade_handler`) authenticates during
//! the HTTP handshake via `hoops::jwt::authenticate()`.
//!
//! `send_to_session` returns a differentiated `SendResult` (Delivered /
//! SessionNotFound / NotSubscribed); the primary push path is `broadcast_event`
//! + per-connection forward filtering. A real axum WS route is mounted at
//! `/api/v1/ws` (see `routers::mod::root`).
#![allow(dead_code)]

use crate::hoops::jwt;
use crate::kernel::types::*;
use crate::tenant::RequestTenantContext;
use crate::AppError;
use axum::extract::Extension;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::{broadcast, RwLock};

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WsMessageType {
    // Client -> Server
    Store,
    Retrieve,
    Update,
    Delete,
    Search,
    Subscribe,
    Unsubscribe,
    Ping,

    // Server -> Client
    Stored,
    Retrieved,
    Updated,
    Deleted,
    SearchResults,
    Event,
    Error,
    Connected,
    Pong,
}

/// WebSocket message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessage {
    pub msg_type: WsMessageType,
    pub request_id: Option<String>,
    pub payload: WsPayload,
}

/// WebSocket payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", content = "data")]
pub enum WsPayload {
    // Client requests
    Store(StoreRequest),
    Retrieve(RetrieveRequest),
    Update(UpdateRequest),
    Delete(DeleteRequest),
    Search(SearchRequest),
    Subscribe(SubscribeRequest),
    Unsubscribe(UnsubscribeRequest),
    Ping,

    // Server responses
    Stored(StoredResponse),
    Retrieved(RetrievedResponse),
    Updated(UpdatedResponse),
    Deleted(DeletedResponse),
    SearchResults(SearchResultsResponse),
    Event(EventResponse),
    Error(ErrorResponse),
    Connected(ConnectedResponse),
    Pong,
}

/// Store request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreRequest {
    pub layer: LayerType,
    pub content: MemoryContent,
    pub metadata: Option<MemoryMetadata>,
}

/// Retrieve request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveRequest {
    pub id: String,
}

/// Update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRequest {
    pub id: String,
    pub content: Option<MemoryContent>,
    pub metadata: Option<MemoryMetadata>,
}

/// Delete request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRequest {
    pub id: String,
}

/// Search request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub layer: Option<LayerType>,
    pub filters: Option<MemoryFilters>,
    pub limit: Option<usize>,
}

/// Subscribe request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeRequest {
    pub event_type: EventType,
}

/// Unsubscribe request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeRequest {
    pub subscription_id: String,
}

/// Event types for subscriptions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    MemoryAdded,
    MemoryUpdated,
    MemoryDeleted,
    MemoryEvicted,
    LayerFull,
}

/// Differentiated result for `send_to_session` — replaces the old `bool` that
/// could not distinguish "session closed" from "not subscribed".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendResult {
    /// The event would be forwarded to the session (tenant matches and either
    /// no subscriptions = receive-all, or the event type is subscribed).
    Delivered,
    /// The session does not exist (connection closed or never established).
    SessionNotFound,
    /// The session exists but has active subscriptions that do not include
    /// this event type.
    NotSubscribed,
}

/// Lightweight event payload pushed over WebSocket — avoids carrying the full
/// kernel `MemoryEntry` (which may hold `Binary`/`GraphData`) over the wire.
/// `summary` is truncated to ~256 chars so a client can decide whether to
/// fetch the full entry. `tenant_id` is present on every variant so the
/// broadcast forward task can filter by tenant without deserializing deeply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEventPayload {
    pub id: String,
    pub tenant_id: String,
    pub layer: LayerType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Stored response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredResponse {
    pub id: String,
    pub layer: LayerType,
}

/// Retrieved response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedResponse {
    pub entry: MemoryEntry,
}

/// Updated response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatedResponse {
    pub id: String,
    pub success: bool,
}

/// Deleted response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedResponse {
    pub id: String,
    pub success: bool,
}

/// Search results response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultsResponse {
    pub results: Vec<MemoryMatch>,
}

/// Event response (for subscriptions)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventResponse {
    pub event_type: EventType,
    pub data: EventData,
}

/// Event data — lightweight variants; every variant carries `tenant_id` so
/// the broadcast forward task can filter by tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EventData {
    MemoryAdded(MemoryEventPayload),
    MemoryUpdated {
        id: String,
        payload: MemoryEventPayload,
    },
    MemoryDeleted {
        id: String,
        layer: LayerType,
        tenant_id: String,
    },
    MemoryEvicted {
        ids: Vec<String>,
        tenant_id: String,
    },
    LayerFull {
        layer: LayerType,
        capacity: usize,
        tenant_id: String,
    },
}

/// Error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: i32,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

/// Connected response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedResponse {
    pub session_id: String,
    pub server_version: String,
}

/// WebSocket connection information.
///
/// Binds `RequestTenantContext` so all frames on this connection are
/// tenant-scoped (ADR-0007). The tenant context is set once during the
/// HTTP handshake and does not change for the connection lifetime.
#[derive(Debug, Clone)]
pub struct WsConnection {
    pub session_id: String,
    /// Tenant-scoped identity from the handshake JWT.
    pub tenant_ctx: RequestTenantContext,
    pub subscriptions: Vec<Subscription>,
    pub connected_at: i64,
}

/// Subscription info
#[derive(Debug, Clone)]
pub struct Subscription {
    pub id: String,
    pub event_type: EventType,
}

/// WebSocket connection manager
pub struct WsConnectionManager {
    connections: RwLock<HashMap<String, WsConnection>>,
    event_tx: broadcast::Sender<EventResponse>,
    session_counter: RwLock<u64>,
}

impl WsConnectionManager {
    /// Create a new WebSocket connection manager.
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(1000);
        Self {
            connections: RwLock::new(HashMap::new()),
            event_tx,
            session_counter: RwLock::new(0),
        }
    }

    /// Create a new session and register connection.
    pub async fn create_session(
        &self,
        tenant_ctx: RequestTenantContext,
    ) -> (String, broadcast::Receiver<EventResponse>) {
        let mut counter = self.session_counter.write().await;
        *counter += 1;
        let session_id = format!("ws_session_{}", *counter);

        let connection = WsConnection {
            session_id: session_id.clone(),
            tenant_ctx,
            subscriptions: Vec::new(),
            connected_at: chrono::Utc::now().timestamp(),
        };

        self.connections
            .write()
            .await
            .insert(session_id.clone(), connection);

        // Subscribe to event broadcasts
        let rx = self.event_tx.subscribe();

        (session_id, rx)
    }

    /// Get connection info by session ID.
    pub async fn get_connection(&self, session_id: &str) -> Option<WsConnection> {
        self.connections.read().await.get(session_id).cloned()
    }

    /// Check if session exists.
    pub async fn has_session(&self, session_id: &str) -> bool {
        self.connections.read().await.contains_key(session_id)
    }

    /// Remove a session.
    pub async fn remove_session(&self, session_id: &str) -> bool {
        self.connections.write().await.remove(session_id).is_some()
    }

    /// Subscribe to events.
    pub async fn subscribe(&self, session_id: &str, event_type: EventType) -> Option<String> {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(session_id) {
            let subscription_id = format!("sub_{}", conn.subscriptions.len());
            conn.subscriptions.push(Subscription {
                id: subscription_id.clone(),
                event_type,
            });
            Some(subscription_id)
        } else {
            None
        }
    }

    /// Unsubscribe from events.
    pub async fn unsubscribe(&self, session_id: &str, subscription_id: &str) -> bool {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(session_id) {
            let original_len = conn.subscriptions.len();
            conn.subscriptions.retain(|s| s.id != subscription_id);
            conn.subscriptions.len() < original_len
        } else {
            false
        }
    }

    /// Broadcast an event to all subscribed connections. Synchronous and
    /// non-blocking (`broadcast::Sender::send` never awaits) so callers can
    /// fire-and-forget it from hot paths (memory write/delete chokepoints).
    pub fn broadcast_event(&self, event: EventResponse) -> usize {
        match self.event_tx.send(event) {
            Ok(n) => n,
            Err(tokio::sync::broadcast::error::SendError(_e)) => {
                // No receivers → all connections are closed. Not an error
                // condition worth surfacing to the caller; the memory write
                // succeeded regardless.
                tracing::debug!(
                    "WS broadcast dropped: no active receivers (all connections closed or channel full)"
                );
                0
            }
        }
    }

    /// Get active connection count.
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Current number of messages buffered in the broadcast channel (queue depth).
    /// High values indicate slow consumers or a consumer that has stopped draining.
    pub fn queue_depth(&self) -> usize {
        self.event_tx.len()
    }

    /// Whether a session would receive `event`: the session must exist and
    /// either have no subscriptions (receive-all default) or be subscribed to
    /// this event type. Returns a differentiated `SendResult` instead of the
    /// old `bool` so callers can distinguish "session closed" from "not
    /// subscribed". NOTE: actual delivery happens via the broadcast channel
    /// the forward task in `handle_ws_connection` drains — this is a truthful
    /// query, not a send.
    pub async fn send_to_session(&self, session_id: &str, event: EventResponse) -> SendResult {
        let connections = self.connections.read().await;
        let Some(conn) = connections.get(session_id) else {
            return SendResult::SessionNotFound;
        };
        if conn.subscriptions.is_empty()
            || conn
                .subscriptions
                .iter()
                .any(|sub| sub.event_type == event.event_type)
        {
            SendResult::Delivered
        } else {
            SendResult::NotSubscribed
        }
    }

    /// Clean up old sessions (based on timestamp).
    pub async fn cleanup_stale_sessions(&self, max_age_seconds: i64) -> usize {
        let mut connections = self.connections.write().await;
        let now = chrono::Utc::now().timestamp();
        let initial_count = connections.len();

        connections.retain(|_, conn| now - conn.connected_at < max_age_seconds);

        initial_count - connections.len()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Global singleton — the WS manager is reached both from the axum upgrade
// handler and from the memory write/delete chokepoints (static service methods
// with no DI). Mirrors LLM_SERVICE / DATABASE_POOL.
// ─────────────────────────────────────────────────────────────────────────────

static WS_MANAGER: std::sync::OnceLock<std::sync::Arc<WsConnectionManager>> =
    std::sync::OnceLock::new();

/// Get the global WebSocket connection manager. Panics if not initialized —
/// `init_ws_manager()` MUST be called from `main` at startup.
pub fn ws_manager() -> &'static std::sync::Arc<WsConnectionManager> {
    WS_MANAGER.get().expect("WS_MANAGER not initialized")
}

/// Initialize the global WS manager singleton. Call once at startup (main.rs),
/// before the HTTP server accepts connections.
pub fn init_ws_manager() {
    WS_MANAGER
        .set(std::sync::Arc::new(WsConnectionManager::new()))
        .ok()
        .expect("WS_MANAGER already initialized");
}

// ─────────────────────────────────────────────────────────────────────────────
// Axum WebSocket upgrade handler with handshake auth (ADR-0007, P2 PR-3).
//
// Authenticates the HTTP upgrade request via `hoops::jwt::authenticate()`
// before establishing the WebSocket connection. The tenant context is bound
// once at handshake time and applies to all frames on the connection.
// ─────────────────────────────────────────────────────────────────────────────

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;

/// Axum WebSocket upgrade handler.
///
/// The HTTP handshake request must carry a valid JWT (cookie or Bearer).
/// On success, the connection is established with `RequestTenantContext`
/// bound to the `WsConnection`. On auth failure, the upgrade is rejected
/// with a close frame.
/// WebSocket upgrade handler (ADR-0007, backlog A-5).
///
/// Auth is handled by the outer `auth_middleware` layer on the route that mounts
/// this handler — `RequestTenantContext` is already in the request extensions by
/// the time the upgrade occurs. The tenant context is captured here and threaded
/// into the connection handler.
pub async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    Extension(tenant_ctx): Extension<crate::tenant::RequestTenantContext>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, tenant_ctx))
}

async fn handle_ws_connection(
    mut socket: WebSocket,
    tenant_ctx: crate::tenant::RequestTenantContext,
) {
    let manager = ws_manager();
    let (session_id, mut broadcast_rx) = manager.create_session(tenant_ctx.clone()).await;
    crate::services::prometheus_exporter::get_exporter()
        .set_ws_connections_active(manager.connection_count().await as f64);

    // Send the Connected frame so the client knows its session_id.
    let connected = WsMessage {
        msg_type: WsMessageType::Connected,
        request_id: None,
        payload: WsPayload::Connected(ConnectedResponse {
            session_id: session_id.clone(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
        }),
    };
    let send_start = std::time::Instant::now();
    let send_ok = socket
        .send(Message::Text(
            serde_json::to_string(&connected).unwrap_or_default().into(),
        ))
        .await
        .is_ok();
    crate::services::prometheus_exporter::get_exporter()
        .record_ws_send_duration(send_start.elapsed().as_secs_f64());
    if !send_ok {
        manager.remove_session(&session_id).await;
        crate::services::prometheus_exporter::get_exporter()
            .set_ws_connections_active(manager.connection_count().await as f64);
        return;
    }

    // Drive client→server (request/response) and server→client (broadcast
    // push) concurrently. Either side breaking ends the connection.
    loop {
        tokio::select! {
            // ── Client → Server ──
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let response = match serde_json::from_str::<WsMessage>(&text) {
                            Ok(ws_msg) => {
                                let result = handle_ws_message(
                                    &ws_msg, &tenant_ctx, &session_id, manager,
                                ).await;
                                serde_json::json!({
                                    "request_id": ws_msg.request_id,
                                    "success": result.is_ok(),
                                    "result": result.as_ref().ok(),
                                    "error": result.as_ref().err().map(|e| e.to_string()),
                                })
                            }
                            Err(e) => serde_json::json!({
                                "success": false,
                                "error": format!("invalid message: {e}"),
                            }),
                        };
                        let send_start = std::time::Instant::now();
                        let send_ok = socket
                            .send(Message::Text(response.to_string().into()))
                            .await
                            .is_ok();
                        crate::services::prometheus_exporter::get_exporter()
                            .record_ws_send_duration(send_start.elapsed().as_secs_f64());
                        if !send_ok {
                            break;
                        }
                    }
                    Some(Ok(_)) => continue, // non-text frames ignored
                    Some(Err(_)) | None => break,
                }
            }

            // ── Server → Client (broadcast push) ──
            event = broadcast_rx.recv() => {
                // Update queue depth gauge on every receive — the depth
                // after draining this message reflects current backlog.
                crate::services::prometheus_exporter::get_exporter()
                    .set_ws_broadcast_queue_depth(manager.queue_depth() as f64);
                match event {
                    Ok(event) => {
                        if should_forward_event(manager, &session_id, &tenant_ctx, &event).await {
                            let msg = WsMessage {
                                msg_type: WsMessageType::Event,
                                request_id: None,
                                payload: WsPayload::Event(event),
                            };
                            let send_start = std::time::Instant::now();
                            let send_ok = socket
                                .send(Message::Text(
                                    serde_json::to_string(&msg).unwrap_or_default().into(),
                                ))
                                .await
                                .is_ok();
                            crate::services::prometheus_exporter::get_exporter()
                                .record_ws_send_duration(send_start.elapsed().as_secs_f64());
                            if !send_ok {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(
                            session_id = %session_id,
                            skipped = n,
                            "WS client lagged, dropping events"
                        );
                        crate::services::prometheus_exporter::get_exporter()
                            .inc_ws_lagged_drops();
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    manager.remove_session(&session_id).await;
    crate::services::prometheus_exporter::get_exporter()
        .set_ws_connections_active(manager.connection_count().await as f64);
}

async fn handle_ws_message(
    msg: &WsMessage,
    tenant_ctx: &crate::tenant::RequestTenantContext,
    session_id: &str,
    manager: &WsConnectionManager,
) -> Result<serde_json::Value, String> {
    use crate::services::memory_search::MemorySearchService;
    use crate::services::memory_storage::MemoryStorageService;

    match &msg.payload {
        WsPayload::Subscribe(req) => {
            let sub_id = manager
                .subscribe(session_id, req.event_type.clone())
                .await
                .ok_or_else(|| "session not found".to_string())?;
            Ok(serde_json::json!({ "subscription_id": sub_id, "event_type": req.event_type }))
        }
        WsPayload::Unsubscribe(req) => {
            let ok = manager.unsubscribe(session_id, &req.subscription_id).await;
            Ok(serde_json::json!({ "unsubscribed": req.subscription_id, "ok": ok }))
        }
        WsPayload::Search(req) => {
            let query = req.query.as_deref().unwrap_or("");
            let results = MemorySearchService::search_ltm_for_tenant(
                &tenant_ctx.tenant_id,
                query,
                req.limit.unwrap_or(10),
                None,
                None,
            )
            .await
            .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(results).unwrap_or_default())
        }
        WsPayload::Store(req) => {
            let content_str = serde_json::to_string(&req.content).unwrap_or_default();
            let result = MemoryStorageService::store_ltm_for_tenant(
                &tenant_ctx.tenant_id,
                &format!("t:{}:ws", tenant_ctx.tenant_id),
                "user_input",
                &content_str,
                None,
            )
            .await
            .map_err(|e| format!("{e}"))?;
            Ok(serde_json::to_value(result).unwrap_or_default())
        }
        _ => Err("unsupported operation".to_string()),
    }
}

/// Whether a broadcast `event` should be forwarded to `session_id`:
/// tenant must match AND (no subscriptions = receive-all, OR subscribed to
/// this event type). Slow-consumer / closed-connection handling lives in the
/// caller's `select!` loop (send-failure breaks the loop).
async fn should_forward_event(
    manager: &WsConnectionManager,
    session_id: &str,
    tenant_ctx: &crate::tenant::RequestTenantContext,
    event: &EventResponse,
) -> bool {
    // 1. Tenant check — every EventData variant carries tenant_id.
    let event_tenant: &str = match &event.data {
        EventData::MemoryAdded(p) => &p.tenant_id,
        EventData::MemoryUpdated { payload, .. } => &payload.tenant_id,
        EventData::MemoryDeleted { tenant_id, .. } => tenant_id,
        EventData::MemoryEvicted { tenant_id, .. } => tenant_id,
        EventData::LayerFull { tenant_id, .. } => tenant_id,
    };
    if event_tenant != tenant_ctx.tenant_id.as_str() {
        return false;
    }

    // 2. Subscription check — empty subscriptions = receive all.
    let Some(conn) = manager.get_connection(session_id).await else {
        return false;
    };
    conn.subscriptions.is_empty()
        || conn
            .subscriptions
            .iter()
            .any(|sub| sub.event_type == event.event_type)
}

impl Default for WsConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for WebSocket messages
pub struct WsMessageBuilder {
    msg_type: WsMessageType,
    request_id: Option<String>,
    payload: WsPayload,
}

impl WsMessageBuilder {
    pub fn new(msg_type: WsMessageType) -> Self {
        Self {
            msg_type,
            request_id: None,
            payload: WsPayload::Pong,
        }
    }

    pub fn request_id(mut self, id: String) -> Self {
        self.request_id = Some(id);
        self
    }

    pub fn payload<T: Into<WsPayload>>(mut self, payload: T) -> Self {
        self.payload = payload.into();
        self
    }

    pub fn build(self) -> WsMessage {
        WsMessage {
            msg_type: self.msg_type,
            request_id: self.request_id,
            payload: self.payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_manager() {
        let manager = WsConnectionManager::new();

        // Create session with a mock tenant context
        let tenant_ctx = crate::tenant::RequestTenantContext::new("test-tenant");
        let (session_id, _rx) = manager.create_session(tenant_ctx).await;
        assert!(manager.has_session(&session_id).await);

        // Get connection
        let conn = manager.get_connection(&session_id).await;
        assert!(conn.is_some());

        // Subscribe
        let sub_id = manager.subscribe(&session_id, EventType::MemoryAdded).await;
        assert!(sub_id.is_some());

        // Unsubscribe
        let result = manager.unsubscribe(&session_id, &sub_id.unwrap()).await;
        assert!(result);

        // Remove session
        let removed = manager.remove_session(&session_id).await;
        assert!(removed);
        assert!(!manager.has_session(&session_id).await);
    }

    #[tokio::test]
    async fn test_connection_count() {
        let manager = WsConnectionManager::new();

        let tenant_ctx1 = crate::tenant::RequestTenantContext::new("test-tenant-1");
        let tenant_ctx2 = crate::tenant::RequestTenantContext::new("test-tenant-2");
        let (session1, _) = manager.create_session(tenant_ctx1).await;
        let (session2, _) = manager.create_session(tenant_ctx2).await;

        assert_eq!(manager.connection_count().await, 2);

        manager.remove_session(&session1).await;
        assert_eq!(manager.connection_count().await, 1);

        manager.remove_session(&session2).await;
        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn broadcast_delivers_to_subscribed_receiver() {
        let manager = WsConnectionManager::new();
        let tenant_ctx = crate::tenant::RequestTenantContext::new("tenant-a");
        let (_session_id, mut rx) = manager.create_session(tenant_ctx).await;

        let event = EventResponse {
            event_type: EventType::MemoryAdded,
            data: EventData::MemoryAdded(MemoryEventPayload {
                id: "id-1".to_string(),
                tenant_id: "tenant-a".to_string(),
                layer: LayerType::Ltm,
                source_type: None,
                summary: Some("s".to_string()),
                metadata: None,
            }),
        };
        // One receiver subscribed → broadcast returns 1 and the receiver gets it.
        assert_eq!(manager.broadcast_event(event.clone()), 1);
        let recv = rx.recv().await.unwrap();
        assert_eq!(recv.event_type, EventType::MemoryAdded);
    }

    #[tokio::test]
    async fn should_forward_filters_by_tenant_and_subscription() {
        let manager = WsConnectionManager::new();
        let tenant_a = crate::tenant::RequestTenantContext::new("tenant-a");
        let (session_id, _rx) = manager.create_session(tenant_a.clone()).await;

        let deleted = |tenant: &str| EventResponse {
            event_type: EventType::MemoryDeleted,
            data: EventData::MemoryDeleted {
                id: "id".to_string(),
                layer: LayerType::Ltm,
                tenant_id: tenant.to_string(),
            },
        };
        let added = |tenant: &str| EventResponse {
            event_type: EventType::MemoryAdded,
            data: EventData::MemoryAdded(MemoryEventPayload {
                id: "id".to_string(),
                tenant_id: tenant.to_string(),
                layer: LayerType::Ltm,
                source_type: None,
                summary: None,
                metadata: None,
            }),
        };

        // No subscriptions → receive all (same tenant).
        assert!(should_forward_event(&manager, &session_id, &tenant_a, &deleted("tenant-a")).await);
        // Cross-tenant → never forwarded.
        assert!(!should_forward_event(&manager, &session_id, &tenant_a, &deleted("tenant-b")).await);

        // Subscribe to MemoryDeleted only.
        manager.subscribe(&session_id, EventType::MemoryDeleted).await;
        // Matching type + tenant → forwarded.
        assert!(should_forward_event(&manager, &session_id, &tenant_a, &deleted("tenant-a")).await);
        // Non-matching type (MemoryAdded) with an active subscription → filtered out.
        assert!(!should_forward_event(&manager, &session_id, &tenant_a, &added("tenant-a")).await);
    }

    #[tokio::test]
    async fn send_to_session_returns_differentiated_result() {
        let manager = WsConnectionManager::new();
        let tenant_a = crate::tenant::RequestTenantContext::new("tenant-a");
        let (session_id, _rx) = manager.create_session(tenant_a).await;

        let event = EventResponse {
            event_type: EventType::MemoryAdded,
            data: EventData::MemoryAdded(MemoryEventPayload {
                id: "id".to_string(),
                tenant_id: "tenant-a".to_string(),
                layer: LayerType::Ltm,
                source_type: None,
                summary: None,
                metadata: None,
            }),
        };

        // No subscriptions → receive all → Delivered.
        assert_eq!(
            manager.send_to_session(&session_id, event.clone()).await,
            SendResult::Delivered
        );

        // Non-existent session → SessionNotFound.
        assert_eq!(
            manager.send_to_session("dead-session", event.clone()).await,
            SendResult::SessionNotFound
        );

        // Subscribe to MemoryDeleted only → NotSubscribed for MemoryAdded.
        manager.subscribe(&session_id, EventType::MemoryDeleted).await;
        assert_eq!(
            manager.send_to_session(&session_id, event).await,
            SendResult::NotSubscribed
        );
    }

    #[tokio::test]
    async fn queue_depth_reflects_buffered_messages() {
        let manager = WsConnectionManager::new();
        let tenant_ctx = crate::tenant::RequestTenantContext::new("tenant-a");
        let (_session_id, _rx) = manager.create_session(tenant_ctx).await;

        // Initially empty.
        assert_eq!(manager.queue_depth(), 0);

        // Broadcast a few events — they should be buffered (no one draining _rx).
        for i in 0..5 {
            manager.broadcast_event(EventResponse {
                event_type: EventType::MemoryAdded,
                data: EventData::MemoryAdded(MemoryEventPayload {
                    id: format!("id-{i}"),
                    tenant_id: "tenant-a".to_string(),
                    layer: LayerType::Ltm,
                    source_type: None,
                    summary: None,
                    metadata: None,
                }),
            });
        }
        assert!(manager.queue_depth() > 0, "queue should have buffered messages");
    }
}
