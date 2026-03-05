//! Memory Protocol - API Definitions
//!
//! This module defines the Memory Protocol for client-server communication.

pub mod grpc;
pub mod websocket;

use serde::{Deserialize, Serialize};
use crate::kernel::types::*;
use crate::kernel::traits::{EvictionPolicy, MemoryStats};

/// Memory Protocol Methods
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMethod {
    Store,
    Retrieve,
    Update,
    Delete,
    Search,
    List,
    Stats,
    Evict,
    Compress,
    Merge,
    Forget,
}

/// Memory Protocol Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRequest {
    pub method: MemoryMethod,
    pub params: MemoryParams,
    pub context: ProtocolContext,
}

/// Memory Protocol Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryResponse {
    pub request_id: String,
    pub success: bool,
    pub result: Option<MemoryResultValue>,
    pub error: Option<ProtocolError>,
}

/// Protocol Context (includes tenant, user, session info)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolContext {
    pub tenant_id: Option<String>,
    pub user_id: String,
    pub session_id: Option<String>,
    pub request_id: String,
    pub timestamp: i64,
}

/// Memory Parameters (method-specific)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MemoryParams {
    Store(StoreParams),
    Retrieve(RetrieveParams),
    Update(UpdateParams),
    Delete(DeleteParams),
    Search(SearchParams),
    List(ListParams),
    Stats(StatsParams),
    Evict(EvictParams),
}

/// Store parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreParams {
    pub layer: LayerType,
    pub content: MemoryContent,
    pub metadata: Option<MemoryMetadata>,
}

/// Retrieve parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveParams {
    pub id: String,
}

/// Update parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateParams {
    pub id: String,
    pub content: Option<MemoryContent>,
    pub metadata: Option<MemoryMetadata>,
}

/// Delete parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteParams {
    pub id: String,
}

/// Search parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParams {
    pub layer: Option<LayerType>,
    pub query: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub filters: Option<MemoryFilters>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// List parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListParams {
    pub layer: Option<LayerType>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Stats parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsParams {
    pub layer: Option<LayerType>,
}

/// Evict parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictParams {
    pub policy: EvictionPolicy,
}

/// Memory Result Value
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MemoryResultValue {
    Store(StoreResult),
    Retrieve(MemoryEntry),
    Update(UpdateResult),
    Delete(DeleteResult),
    Search(Vec<MemoryMatch>),
    List(Vec<MemoryEntry>),
    Stats(MemoryStats),
    Evict(EvictResult),
}

/// Store result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreResult {
    pub id: String,
    pub layer: LayerType,
}

/// Update result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResult {
    pub id: String,
    pub updated: bool,
}

/// Delete result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResult {
    pub id: String,
    pub deleted: bool,
}

/// Evict result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictResult {
    pub evicted_ids: Vec<String>,
    pub count: usize,
}

/// Protocol Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolError {
    pub code: i32,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl ProtocolError {
    pub fn not_found(id: &str) -> Self {
        Self {
            code: 404,
            message: format!("Memory not found: {}", id),
            details: None,
        }
    }

    pub fn invalid_params(msg: &str) -> Self {
        Self {
            code: 400,
            message: msg.to_string(),
            details: None,
        }
    }

    pub fn internal(msg: &str) -> Self {
        Self {
            code: 500,
            message: msg.to_string(),
            details: None,
        }
    }
}
