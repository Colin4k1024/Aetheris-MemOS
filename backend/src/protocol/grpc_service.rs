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
        // #128: gRPC callers identify the recall subject via the `user-id`
        // metadata key; when present, the same belief recall core every
        // transport uses appends the Working Memory block (one synthetic,
        // clearly-labelled result row — the proto has no belief field).
        let grpc_user = request
            .metadata()
            .get("user-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let req = request.into_inner();
        let limit = if req.limit > 0 { req.limit } else { 10 };

        // #128: belief recall runs FIRST and never depends on the embedding
        // backend — a governed, cited Working Memory is the guaranteed surface.
        // The legacy LTM search is additive and degrades (empty + note) when
        // its embedding dependency is unavailable, instead of failing the RPC.
        let mut wm_row: Option<SearchResult> = None;
        if let Some(user) = grpc_user {
            let wm = crate::services::recall::core::belief_working_memory(
                &tenant_ctx.tenant_id,
                Some(&user),
                &req.query,
                None,
                None,
                None,
                &[],
            )
            .await
            .map_err(|e| Status::internal(format!("belief recall failed: {e}")))?;
            if let Some(wm) = wm {
                if !wm.text.is_empty() {
                    wm_row = Some(SearchResult {
                        entry_id: "working-memory".to_string(),
                        source_layer: "belief".to_string(),
                        score: 1.0,
                        content: wm.text,
                        metadata: [
                            ("asOf".to_string(), wm.as_of.clone()),
                            ("principalId".to_string(), wm.principal_id.clone()),
                        ]
                        .into_iter()
                        .collect(),
                    });
                }
            }
        }

        let mut results: Vec<SearchResult> = match MemorySearchService::search_ltm_for_tenant(
            &tenant_ctx.tenant_id,
            &req.query,
            limit as usize,
            None,
            None,
        )
        .await
        {
            Ok(rows) => rows
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
                .collect(),
            Err(e) => {
                // Degraded legacy channel: report, keep the RPC alive on the
                // belief surface when present.
                let mut note = vec![SearchResult {
                    entry_id: "legacy-search-degraded".to_string(),
                    source_layer: "note".to_string(),
                    score: 0.0,
                    content: format!("legacy LTM search unavailable: {e}"),
                    metadata: Default::default(),
                }];
                if wm_row.is_none() {
                    return Err(Status::internal(format!("{e}")));
                }
                note
            }
        };
        if let Some(row) = wm_row {
            results.insert(0, row);
        }

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
