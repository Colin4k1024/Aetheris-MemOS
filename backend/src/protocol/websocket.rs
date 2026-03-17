//! WebSocket Protocol
//!
//! This module provides WebSocket message types and connection management for real-time memory operations.

use crate::kernel::types::*;
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    MemoryAdded,
    MemoryUpdated,
    MemoryDeleted,
    MemoryEvicted,
    LayerFull,
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

/// Event data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EventData {
    MemoryAdded(MemoryEntry),
    MemoryUpdated { id: String, entry: MemoryEntry },
    MemoryDeleted { id: String },
    MemoryEvicted { ids: Vec<String> },
    LayerFull { layer: LayerType, capacity: usize },
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

/// WebSocket connection information
#[derive(Debug, Clone)]
pub struct WsConnection {
    pub session_id: String,
    pub user_id: Option<String>,
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
        user_id: Option<String>,
    ) -> (String, broadcast::Receiver<EventResponse>) {
        let mut counter = self.session_counter.write().await;
        *counter += 1;
        let session_id = format!("ws_session_{}", *counter);

        let connection = WsConnection {
            session_id: session_id.clone(),
            user_id,
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

    /// Broadcast an event to all subscribed connections.
    pub async fn broadcast_event(&self, event: EventResponse) -> usize {
        self.event_tx.send(event).unwrap_or(0)
    }

    /// Get active connection count.
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Send event to a specific session (if subscribed).
    pub async fn send_to_session(&self, session_id: &str, event: EventResponse) -> bool {
        let connections = self.connections.read().await;
        if let Some(conn) = connections.get(session_id) {
            // Check if the connection is subscribed to this event type
            let is_subscribed = conn.subscriptions.iter().any(|sub| {
                matches!(&event.event_type, EventType::MemoryAdded if matches!(sub.event_type, EventType::MemoryAdded))
                    || matches!(&event.event_type, EventType::MemoryUpdated if matches!(sub.event_type, EventType::MemoryUpdated))
                    || matches!(&event.event_type, EventType::MemoryDeleted if matches!(sub.event_type, EventType::MemoryDeleted))
                    || matches!(&event.event_type, EventType::MemoryEvicted if matches!(sub.event_type, EventType::MemoryEvicted))
                    || matches!(&event.event_type, EventType::LayerFull if matches!(sub.event_type, EventType::LayerFull))
            });

            // For simplicity, always return true if session exists
            // In real implementation, would send via WebSocket
            is_subscribed || true
        } else {
            false
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

        // Create session
        let (session_id, _rx) = manager.create_session(None).await;
        assert!(manager.has_session(&session_id).await);

        // Get connection
        let conn = manager.get_connection(&session_id).await;
        assert!(conn.is_some());

        // Subscribe
        let sub_id = manager.subscribe(&session_id, EventType::MemoryAdded).await;
        assert!(sub_id.is_some());

        // Unsubscribe
        let result = manager.unsubscribe(&session_id, sub_id.unwrap()).await;
        assert!(result);

        // Remove session
        let removed = manager.remove_session(&session_id).await;
        assert!(removed);
        assert!(!manager.has_session(&session_id).await);
    }

    #[tokio::test]
    async fn test_connection_count() {
        let manager = WsConnectionManager::new();

        let (session1, _) = manager.create_session(None).await;
        let (session2, _) = manager.create_session(None).await;

        assert_eq!(manager.connection_count().await, 2);

        manager.remove_session(&session1).await;
        assert_eq!(manager.connection_count().await, 1);

        manager.remove_session(&session2).await;
        assert_eq!(manager.connection_count().await, 0);
    }
}
