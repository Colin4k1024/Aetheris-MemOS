//! gRPC Protocol Definition — P2 implementation (ADR-0007).
//!
//! This module defines the gRPC service types AND provides a tonic `Interceptor`
//! that extracts JWT from request metadata and delegates to the transport-agnostic
//! `hoops::jwt::authenticate()` core.
//!
//! The actual tonic server wiring (`.proto` codegen + service impl) is a separate
//! step. The interceptor is the **auth adapter** that all gRPC handlers will share.

use crate::hoops::jwt;
use crate::tenant::RequestTenantContext;
use crate::AppError;

use tonic::{Request, Status};

/// tonic Interceptor: extract `authorization: Bearer <jwt>` from metadata,
/// validate via `hoops::jwt::authenticate()`, and inject `JwtClaims` +
/// `RequestTenantContext` into the request extensions.
///
/// Returns `Status::unauthenticated` on any auth failure.
pub fn grpc_auth_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    // 1. Extract Bearer token from metadata
    let token = req
        .metadata()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| Status::unauthenticated("Missing or malformed Authorization header"))?;

    // 2. Delegate to transport-agnostic authenticator core
    let (claims, tenant_ctx) = jwt::authenticate(token).map_err(|e| match e {
        AppError::Unauthorized(msg) => Status::unauthenticated(msg),
        other => Status::internal(format!("Auth error: {other}")),
    })?;

    // 3. Inject into request extensions (same as REST auth_middleware)
    let mut req = req;
    req.extensions_mut().insert(claims);
    req.extensions_mut().insert(tenant_ctx);

    Ok(req)
}

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
