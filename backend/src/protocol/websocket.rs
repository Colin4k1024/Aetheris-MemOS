//! WebSocket Protocol
//!
//! This module provides WebSocket message types for real-time memory operations.

use serde::{Deserialize, Serialize};
use crate::kernel::types::*;

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

/// WebSocket connection manager
pub struct WsConnectionManager {
    // In production, this would manage WebSocket connections
}

impl WsConnectionManager {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for WsConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}
