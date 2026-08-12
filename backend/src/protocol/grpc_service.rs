//! gRPC MemoryService implementation (ADR-0007, backlog A-5).
//!
//! Delegates to the same service layer as the REST handlers — no business logic
//! lives here, only proto↔domain type translation.

use tonic::{Request, Response, Status};

use crate::services::memory_search::MemorySearchService;
use crate::services::memory_storage::MemoryStorageService;
use crate::tenant::{get_default_tenant, RequestTenantContext};

pub mod pb {
    include!("gen/aetheris.memory.v1.rs");
}

use pb::memory_service_server::MemoryService;
use pb::*;

pub struct MemoryServiceImpl;

#[tonic::async_trait]
impl MemoryService for MemoryServiceImpl {
    async fn store_ltm(
        &self,
        request: Request<StoreLtmRequest>,
    ) -> Result<Response<StoreLtmResponse>, Status> {
        let tenant_ctx = extract_tenant(&request)?;
        let req = request.into_inner();

        let result = MemoryStorageService::store_ltm_for_tenant(
            &tenant_ctx.tenant_id,
            &req.source_id,
            &req.source_type,
            &req.content,
            req.title.as_deref(),
        )
        .await
        .map_err(|e| Status::internal(format!("{e}")))?;

        Ok(Response::new(StoreLtmResponse {
            entry_id: result.entry_id,
            index_status: result.index_status,
            summary_status: result.summary_status,
        }))
    }

    async fn search_ltm(
        &self,
        request: Request<SearchLtmRequest>,
    ) -> Result<Response<SearchLtmResponse>, Status> {
        let tenant_ctx = extract_tenant(&request)?;
        let req = request.into_inner();
        let limit = if req.limit > 0 { req.limit } else { 10 };

        let results = MemorySearchService::search_ltm_for_tenant(
            &tenant_ctx.tenant_id,
            &req.query,
            limit as usize,
            None,
            None,
        )
        .await
        .map_err(|e| Status::internal(format!("{e}")))?;

        let results = results
            .into_iter()
            .map(|r| SearchResult {
                entry_id: r.entry_id,
                source_layer: r.source_layer,
                score: r.score,
                content: r.content,
                metadata: serde_json::from_value::<std::collections::HashMap<String, String>>(
                    r.metadata,
                )
                .unwrap_or_default(),
            })
            .collect();

        Ok(Response::new(SearchLtmResponse { results }))
    }

    async fn store_stm(
        &self,
        request: Request<StoreStmRequest>,
    ) -> Result<Response<StoreStmResponse>, Status> {
        let tenant_ctx = extract_tenant(&request)?;
        let req = request.into_inner();

        let (session_id, message_id) = MemoryStorageService::store_stm_for_tenant(
            &tenant_ctx.tenant_id,
            &req.user_id,
            &req.agent_id,
            &req.session_type,
            &req.role,
            &req.content,
            req.max_context_length,
            req.retention_hours,
            req.session_id.as_deref(),
        )
        .await
        .map_err(|e| Status::internal(format!("{e}")))?;

        Ok(Response::new(StoreStmResponse {
            session_id,
            message_id,
        }))
    }

    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        Ok(Response::new(HealthCheckResponse {
            status: "ready".to_string(),
            dependencies: vec![],
        }))
    }
}

fn extract_tenant<T>(request: &Request<T>) -> Result<RequestTenantContext, Status> {
    request
        .extensions()
        .get::<RequestTenantContext>()
        .cloned()
        .ok_or_else(|| Status::unauthenticated("Missing tenant context"))
}
