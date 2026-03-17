//! gRPC Protocol Definition
//!
//! This module provides gRPC service definitions for the Memory Kernel.

/// gRPC service definition for Memory Kernel.
///
/// This can be used with tonic to generate gRPC servers and clients.
///
/// To regenerate:
/// ```bash
/// protoc --proto_path=proto --rust_out=src/protocol proto/memory_kernel.proto
/// ```
// Note: In production, you would use protobuf definitions.
// This module provides the Rust structures that would be generated.
use crate::kernel::types::*;

/// gRPC Store Request
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcStoreRequest {
    pub layer: i32,       // LayerType as i32
    pub content: Vec<u8>, // Serialized MemoryContent
    pub metadata: Option<GrpcMemoryMetadata>,
}

/// gRPC Store Response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcStoreResponse {
    pub id: String,
    pub layer: i32,
}

/// gRPC Retrieve Request
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcRetrieveRequest {
    pub id: String,
}

/// gRPC Retrieve Response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcRetrieveResponse {
    pub entry: Vec<u8>, // Serialized MemoryEntry
}

/// gRPC Search Request
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcSearchRequest {
    pub query: String,
    pub layer: Option<i32>,
    pub limit: u32,
}

/// gRPC Search Response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcSearchResponse {
    pub results: Vec<GrpcSearchResult>,
}

/// gRPC Search Result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcSearchResult {
    pub entry: Vec<u8>,
    pub score: f32,
}

/// gRPC Memory Metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcMemoryMetadata {
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub importance: f64,
}

/// gRPC Stats Request
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcStatsRequest {
    pub layer: Option<i32>,
}

/// gRPC Stats Response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcStatsResponse {
    pub total_entries: u64,
    pub by_layer: Vec<GrpcLayerStats>,
}

/// gRPC Layer Stats
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcLayerStats {
    pub layer: i32,
    pub entry_count: u64,
    pub size_bytes: u64,
}

/// Streaming search message
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcStreamMessage {
    pub message_type: i32,
    pub payload: Vec<u8>,
}
