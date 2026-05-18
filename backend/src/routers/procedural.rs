//! Procedural Memory & GraphRAG Hybrid Search & Provider Health API Endpoints

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::kernel::error::MemoryResult;
use crate::kernel::hybrid::*;
use crate::kernel::provider::HealthStatus;
use crate::kernel::traits::{GraphMemory, MemoryLayer, VectorSearch};
use crate::kernel::types::*;
use crate::layers::kg_layer::KgMemoryLayer;
use crate::layers::procedural_layer::ProceduralMemoryLayer;
use crate::models::procedural::ProceduralEntry;
use crate::providers::{create_provider, ProviderConfig};
use crate::services::hybrid_search::HybridSearchService;
use crate::{json_ok, JsonResult};

const MAX_QUERY_LENGTH: usize = 1000;
const MAX_SEARCH_LIMIT: usize = 100;

// --- Procedural Memory ---

#[derive(Deserialize)]
pub struct StoreProceduralRequest {
    pub entry: ProceduralEntry,
    pub tags: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct StoreProceduralResponse {
    pub id: String,
    pub version: u32,
}

#[derive(Deserialize)]
pub struct SearchProceduralRequest {
    pub query: Option<String>,
    pub task_type: Option<String>,
    pub tags: Option<Vec<String>>,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct SearchProceduralResponse {
    pub results: Vec<ProceduralMatchResult>,
}

#[derive(Serialize)]
pub struct ProceduralMatchResult {
    pub id: String,
    pub entry: ProceduralEntry,
    pub score: f64,
}

pub async fn store_procedural(
    State(layer): State<Arc<ProceduralMemoryLayer>>,
    Json(req): Json<StoreProceduralRequest>,
) -> JsonResult<StoreProceduralResponse> {
    req.entry
        .validate()
        .map_err(|e| crate::error::AppError::BadRequest(format!("validation failed: {e}")))?;

    info!("Storing procedural memory: name={}", req.entry.name);

    let version = req.entry.version;
    let content = serde_json::to_value(&req.entry)
        .map_err(|e| crate::error::AppError::Internal(format!("serialization failed: {e}")))?;

    let mut metadata = MemoryMetadata::default();
    if let Some(tags) = req.tags {
        metadata.tags = tags;
    }
    metadata.tags.push(format!("task_type:{}", req.entry.task_type));

    let now = chrono::Utc::now().timestamp();
    let entry = MemoryEntry {
        id: MemoryId::new(),
        content: MemoryContent::Json(content),
        layer: LayerType::Procedural,
        metadata,
        created_at: now,
        updated_at: now,
    };

    let id = layer
        .store(entry)
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;

    json_ok(StoreProceduralResponse {
        id: id.as_str().to_string(),
        version,
    })
}

pub async fn search_procedural(
    State(layer): State<Arc<ProceduralMemoryLayer>>,
    Json(req): Json<SearchProceduralRequest>,
) -> JsonResult<SearchProceduralResponse> {
    if let Some(ref q) = req.query {
        if q.len() > MAX_QUERY_LENGTH {
            return Err(crate::error::AppError::BadRequest(
                format!("query exceeds max length of {MAX_QUERY_LENGTH}"),
            ));
        }
    }

    let limit = req.limit.unwrap_or(10).min(MAX_SEARCH_LIMIT);

    info!(
        "Searching procedural memory: query={:?}, task_type={:?}",
        req.query, req.task_type
    );

    let mut tags = req.tags.unwrap_or_default();
    if let Some(ref tt) = req.task_type {
        tags.push(format!("task_type:{tt}"));
    }

    let query = MemoryQuery {
        text: req.query,
        layer: Some(LayerType::Procedural),
        limit,
        offset: 0,
        embedding: None,
        filters: MemoryFilters {
            tags: if tags.is_empty() { None } else { Some(tags) },
            ..Default::default()
        },
    };

    let matches = layer
        .search(&query)
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;

    let results = matches
        .into_iter()
        .filter_map(|m| {
            let id = m.entry.id.as_str().to_string();
            if let MemoryContent::Json(ref val) = m.entry.content {
                serde_json::from_value::<ProceduralEntry>(val.clone())
                    .ok()
                    .map(|entry| ProceduralMatchResult {
                        id,
                        entry,
                        score: m.score,
                    })
            } else {
                None
            }
        })
        .collect();

    json_ok(SearchProceduralResponse { results })
}

// --- GraphRAG Hybrid Search ---

struct InMemoryVectorSearch;

#[async_trait::async_trait]
impl VectorSearch for InMemoryVectorSearch {
    async fn search_by_vector(
        &self,
        _vector: &[f32],
        _limit: usize,
        _filters: &MemoryFilters,
    ) -> MemoryResult<Vec<MemoryMatch>> {
        Ok(vec![])
    }

    async fn upsert_vectors(
        &self,
        _entries: Vec<(MemoryId, Vec<f32>, MemoryEntry)>,
    ) -> MemoryResult<()> {
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct GraphRagSearchRequest {
    pub query: String,
    pub strategy: Option<FusionStrategy>,
    pub vector_weight: Option<f64>,
    pub graph_weight: Option<f64>,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct GraphRagSearchResponse {
    pub results: Vec<HybridSearchResult>,
    pub metadata: HybridSearchMetadata,
}

pub async fn graphrag_hybrid_search(
    Json(req): Json<GraphRagSearchRequest>,
) -> JsonResult<GraphRagSearchResponse> {
    info!(
        "GraphRAG hybrid search: query_len={}, strategy={:?}",
        req.query.len(),
        req.strategy
    );

    let kg_layer = Arc::new(KgMemoryLayer::new());
    let vector_search: Arc<dyn VectorSearch> = Arc::new(InMemoryVectorSearch);
    let graph_memory: Arc<dyn GraphMemory> = kg_layer;

    let config = HybridSearchConfig::default();
    let service = HybridSearchService::new(vector_search, graph_memory, config);

    let search_req = HybridSearchRequest {
        query: req.query,
        strategy: req.strategy,
        vector_weight: req.vector_weight,
        graph_weight: req.graph_weight,
        limit: req.limit,
        filters: None,
    };

    let response = service
        .search(&search_req)
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;

    json_ok(GraphRagSearchResponse {
        results: response.results,
        metadata: response.metadata,
    })
}

// --- Provider Health ---

#[derive(Serialize)]
pub struct ProviderHealthResponse {
    pub provider: String,
    pub status: String,
    pub latency_ms: u64,
    pub message: Option<String>,
    pub capabilities: ProviderCapabilitiesDto,
}

#[derive(Serialize)]
pub struct ProviderCapabilitiesDto {
    pub supports_vector_search: bool,
    pub supports_graph: bool,
    pub supports_metadata_filter: bool,
    pub supports_eviction: bool,
}

pub async fn provider_health() -> JsonResult<ProviderHealthResponse> {
    let config = ProviderConfig::default();
    let provider = create_provider(&config);

    let health = provider.health_check().await.unwrap_or(
        crate::kernel::provider::ProviderHealth {
            status: HealthStatus::Unavailable,
            latency_ms: 0,
            message: Some("health check failed".to_string()),
        },
    );

    let caps = provider.capabilities();

    let status_str = match health.status {
        HealthStatus::Healthy => "healthy",
        HealthStatus::Degraded => "degraded",
        HealthStatus::Unavailable => "unavailable",
    };

    json_ok(ProviderHealthResponse {
        provider: provider.provider_name().to_string(),
        status: status_str.to_string(),
        latency_ms: health.latency_ms,
        message: health.message,
        capabilities: ProviderCapabilitiesDto {
            supports_vector_search: caps.supports_vector_search,
            supports_graph: caps.supports_graph,
            supports_metadata_filter: caps.supports_metadata_filter,
            supports_eviction: caps.supports_eviction,
        },
    })
}
