//! GraphRAG Hybrid Search Service
//!
//! Orchestrates parallel vector search (Qdrant) and graph traversal (Neo4j),
//! then fuses results using configurable strategies (RRF, VectorFirst, GraphFirst).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::hybrid::*;
use crate::kernel::traits::{GraphMemory, VectorSearch};
use crate::kernel::types::*;

pub struct HybridSearchService {
    vector_search: Arc<dyn VectorSearch>,
    graph_memory: Arc<dyn GraphMemory>,
    config: HybridSearchConfig,
}

impl HybridSearchService {
    pub fn new(
        vector_search: Arc<dyn VectorSearch>,
        graph_memory: Arc<dyn GraphMemory>,
        config: HybridSearchConfig,
    ) -> Self {
        Self {
            vector_search,
            graph_memory,
            config,
        }
    }

    pub async fn search(
        &self,
        request: &HybridSearchRequest,
    ) -> MemoryResult<HybridSearchResponse> {
        let strategy = request.strategy.unwrap_or(self.config.default_strategy);
        let limit = request.limit.unwrap_or(self.config.max_results);
        let vector_weight = request.vector_weight.unwrap_or(self.config.vector_weight);
        let graph_weight = request.graph_weight.unwrap_or(self.config.graph_weight);
        let timeout = std::time::Duration::from_millis(self.config.timeout_ms);

        let filters = request.filters.clone().unwrap_or_default();

        let embedding = self.generate_query_embedding(&request.query)?;

        let start = Instant::now();

        let (vector_result, graph_result) = tokio::join!(
            self.timed_vector_search(&embedding, limit, &filters, timeout),
            self.timed_graph_search(&request.query, limit, timeout),
        );

        let total_latency_ms = start.elapsed().as_millis() as u64;
        let vector_latency_ms = total_latency_ms;
        let graph_latency_ms = total_latency_ms;

        let vector_matches = vector_result.unwrap_or_default();
        let graph_matches = graph_result.unwrap_or_default();

        let vector_count = vector_matches.len();
        let graph_count = graph_matches.len();

        let fused = match strategy {
            FusionStrategy::ReciprocalRankFusion => {
                self.rrf_fusion(&vector_matches, &graph_matches, limit)
            }
            FusionStrategy::VectorFirst => {
                self.vector_first_fusion(&vector_matches, &graph_matches, vector_weight, limit)
            }
            FusionStrategy::GraphFirst => {
                self.graph_first_fusion(&vector_matches, &graph_matches, graph_weight, limit)
            }
        };

        let fused_count = fused.len();

        Ok(HybridSearchResponse {
            results: fused,
            metadata: HybridSearchMetadata {
                strategy_used: strategy,
                vector_count,
                graph_count,
                fused_count,
                vector_latency_ms,
                graph_latency_ms,
            },
        })
    }

    fn rrf_fusion(
        &self,
        vector_matches: &[MemoryMatch],
        graph_matches: &[MemoryMatch],
        limit: usize,
    ) -> Vec<HybridSearchResult> {
        let k = self.config.rrf_k as f64;
        let mut scores: HashMap<String, (f64, Option<u32>, Option<u32>, MemoryEntry)> =
            HashMap::new();

        for (rank, m) in vector_matches.iter().enumerate() {
            let rrf_score = 1.0 / (k + rank as f64 + 1.0);
            scores
                .entry(m.entry.id.0.clone())
                .and_modify(|(s, vr, _, _)| {
                    *s += rrf_score;
                    *vr = Some(rank as u32);
                })
                .or_insert((rrf_score, Some(rank as u32), None, m.entry.clone()));
        }

        for (rank, m) in graph_matches.iter().enumerate() {
            let rrf_score = 1.0 / (k + rank as f64 + 1.0);
            scores
                .entry(m.entry.id.0.clone())
                .and_modify(|(s, _, gr, _)| {
                    *s += rrf_score;
                    *gr = Some(rank as u32);
                })
                .or_insert((rrf_score, None, Some(rank as u32), m.entry.clone()));
        }

        let mut results: Vec<HybridSearchResult> = scores
            .into_values()
            .map(|(score, vector_rank, graph_rank, entry)| {
                let provenance = match (vector_rank, graph_rank) {
                    (Some(_), Some(_)) => SearchProvenance::Both,
                    (Some(_), None) => SearchProvenance::VectorOnly,
                    (None, Some(_)) => SearchProvenance::GraphOnly,
                    (None, None) => SearchProvenance::VectorOnly,
                };
                HybridSearchResult {
                    entry,
                    score,
                    provenance,
                    vector_rank,
                    graph_rank,
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        results
    }

    fn vector_first_fusion(
        &self,
        vector_matches: &[MemoryMatch],
        graph_matches: &[MemoryMatch],
        vector_weight: f64,
        limit: usize,
    ) -> Vec<HybridSearchResult> {
        let graph_ids: HashMap<&str, (usize, f64)> = graph_matches
            .iter()
            .enumerate()
            .map(|(i, m)| (m.entry.id.0.as_str(), (i, m.score)))
            .collect();

        let mut results: Vec<HybridSearchResult> = vector_matches
            .iter()
            .enumerate()
            .map(|(rank, m)| {
                let (graph_rank, provenance) =
                    if let Some(&(gr, _)) = graph_ids.get(m.entry.id.0.as_str()) {
                        (Some(gr as u32), SearchProvenance::Both)
                    } else {
                        (None, SearchProvenance::VectorOnly)
                    };
                HybridSearchResult {
                    entry: m.entry.clone(),
                    score: m.score * vector_weight,
                    provenance,
                    vector_rank: Some(rank as u32),
                    graph_rank,
                }
            })
            .collect();

        results.truncate(limit);
        results
    }

    fn graph_first_fusion(
        &self,
        vector_matches: &[MemoryMatch],
        graph_matches: &[MemoryMatch],
        graph_weight: f64,
        limit: usize,
    ) -> Vec<HybridSearchResult> {
        let vector_ids: HashMap<&str, (usize, f64)> = vector_matches
            .iter()
            .enumerate()
            .map(|(i, m)| (m.entry.id.0.as_str(), (i, m.score)))
            .collect();

        let mut results: Vec<HybridSearchResult> = graph_matches
            .iter()
            .enumerate()
            .map(|(rank, m)| {
                let (vector_rank, provenance) =
                    if let Some(&(vr, _)) = vector_ids.get(m.entry.id.0.as_str()) {
                        (Some(vr as u32), SearchProvenance::Both)
                    } else {
                        (None, SearchProvenance::GraphOnly)
                    };
                HybridSearchResult {
                    entry: m.entry.clone(),
                    score: m.score * graph_weight,
                    provenance,
                    vector_rank,
                    graph_rank: Some(rank as u32),
                }
            })
            .collect();

        results.truncate(limit);
        results
    }

    async fn timed_vector_search(
        &self,
        embedding: &[f32],
        limit: usize,
        filters: &MemoryFilters,
        timeout: std::time::Duration,
    ) -> MemoryResult<Vec<MemoryMatch>> {
        match tokio::time::timeout(
            timeout,
            self.vector_search
                .search_by_vector(embedding, limit, filters),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                tracing::warn!("vector search timed out after {:?}", timeout);
                Ok(vec![])
            }
        }
    }

    async fn timed_graph_search(
        &self,
        query: &str,
        limit: usize,
        timeout: std::time::Duration,
    ) -> MemoryResult<Vec<MemoryMatch>> {
        let labels = vec!["Entity".to_string()];
        let properties: HashMap<String, serde_json::Value> = HashMap::from([(
            "query".to_string(),
            serde_json::Value::String(query.to_string()),
        )]);

        match tokio::time::timeout(timeout, self.graph_memory.query_nodes(&labels, &properties))
            .await
        {
            Ok(Ok(nodes)) => {
                let matches: Vec<MemoryMatch> = nodes
                    .into_iter()
                    .take(limit)
                    .enumerate()
                    .map(|(i, node)| {
                        let now = chrono::Utc::now().timestamp();
                        MemoryMatch {
                            entry: MemoryEntry {
                                id: MemoryId::from_string(&node.id),
                                layer: LayerType::Kg,
                                content: MemoryContent::Graph(GraphData {
                                    nodes: vec![node],
                                    edges: vec![],
                                }),
                                metadata: MemoryMetadata::default(),
                                created_at: now,
                                updated_at: now,
                            },
                            score: 1.0 / (i as f64 + 1.0),
                            highlights: vec![],
                        }
                    })
                    .collect();
                Ok(matches)
            }
            Ok(Err(e)) => {
                tracing::warn!("graph search failed: {e}");
                Ok(vec![])
            }
            Err(_) => {
                tracing::warn!("graph search timed out after {:?}", timeout);
                Ok(vec![])
            }
        }
    }

    fn generate_query_embedding(&self, _query: &str) -> MemoryResult<Vec<f32>> {
        // TODO: integrate with EmbeddingService for real embeddings
        // Return a placeholder embedding; callers should eventually inject an EmbeddingService
        Ok(vec![0.0; 384])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct MockVectorSearch {
        results: Vec<MemoryMatch>,
    }

    #[async_trait::async_trait]
    impl VectorSearch for MockVectorSearch {
        async fn search_by_vector(
            &self,
            _vector: &[f32],
            _limit: usize,
            _filters: &MemoryFilters,
        ) -> MemoryResult<Vec<MemoryMatch>> {
            Ok(self.results.clone())
        }

        async fn upsert_vectors(
            &self,
            _entries: Vec<(MemoryId, Vec<f32>, MemoryEntry)>,
        ) -> MemoryResult<()> {
            Ok(())
        }
    }

    struct MockGraphMemory {
        nodes: Vec<GraphNode>,
    }

    #[async_trait::async_trait]
    impl GraphMemory for MockGraphMemory {
        async fn add_node(&self, _node: GraphNode) -> MemoryResult<()> {
            Ok(())
        }
        async fn add_edge(&self, _edge: GraphEdge) -> MemoryResult<()> {
            Ok(())
        }
        async fn query_nodes(
            &self,
            _labels: &[String],
            _properties: &HashMap<String, serde_json::Value>,
        ) -> MemoryResult<Vec<GraphNode>> {
            Ok(self.nodes.clone())
        }
        async fn query_edges(
            &self,
            _from: Option<&str>,
            _to: Option<&str>,
            _relation: Option<&str>,
        ) -> MemoryResult<Vec<GraphEdge>> {
            Ok(vec![])
        }
        async fn traverse(&self, _start: &str, _depth: usize) -> MemoryResult<Vec<GraphNode>> {
            Ok(vec![])
        }
    }

    fn make_match(id: &str, score: f64) -> MemoryMatch {
        let now = chrono::Utc::now().timestamp();
        MemoryMatch {
            entry: MemoryEntry {
                id: MemoryId::from_string(id),
                layer: LayerType::Ltm,
                content: MemoryContent::Text(format!("content-{id}")),
                metadata: MemoryMetadata::default(),
                created_at: now,
                updated_at: now,
            },
            score,
            highlights: vec![],
        }
    }

    fn make_node(id: &str) -> GraphNode {
        GraphNode {
            id: id.to_string(),
            label: "Entity".to_string(),
            properties: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn rrf_fusion_combines_results() {
        let vector = Arc::new(MockVectorSearch {
            results: vec![
                make_match("a", 0.9),
                make_match("b", 0.8),
                make_match("c", 0.7),
            ],
        });
        let graph = Arc::new(MockGraphMemory {
            nodes: vec![make_node("b"), make_node("d")],
        });

        let service = HybridSearchService::new(vector, graph, HybridSearchConfig::default());
        let request = HybridSearchRequest {
            query: "test".to_string(),
            strategy: Some(FusionStrategy::ReciprocalRankFusion),
            vector_weight: None,
            graph_weight: None,
            limit: Some(10),
            filters: None,
        };

        let response = service.search(&request).await.unwrap();

        assert!(!response.results.is_empty());
        assert!(response.metadata.vector_count > 0);

        let b_result = response.results.iter().find(|r| r.entry.id.0 == "b");
        assert!(b_result.is_some());
        assert_eq!(b_result.unwrap().provenance, SearchProvenance::Both);
    }

    #[tokio::test]
    async fn rrf_score_is_correct() {
        let service = HybridSearchService::new(
            Arc::new(MockVectorSearch { results: vec![] }),
            Arc::new(MockGraphMemory { nodes: vec![] }),
            HybridSearchConfig {
                rrf_k: 60,
                ..Default::default()
            },
        );

        let vector = vec![make_match("a", 0.9), make_match("b", 0.8)];
        let graph = vec![make_match("b", 0.7), make_match("c", 0.6)];

        let results = service.rrf_fusion(&vector, &graph, 10);

        let b_result = results.iter().find(|r| r.entry.id.0 == "b").unwrap();
        let expected_b = 1.0 / (60.0 + 2.0) + 1.0 / (60.0 + 1.0);
        assert!((b_result.score - expected_b).abs() < 1e-10);
    }

    #[tokio::test]
    async fn handles_empty_results() {
        let vector = Arc::new(MockVectorSearch { results: vec![] });
        let graph = Arc::new(MockGraphMemory { nodes: vec![] });

        let service = HybridSearchService::new(vector, graph, HybridSearchConfig::default());
        let request = HybridSearchRequest {
            query: "empty".to_string(),
            strategy: None,
            vector_weight: None,
            graph_weight: None,
            limit: None,
            filters: None,
        };

        let response = service.search(&request).await.unwrap();
        assert!(response.results.is_empty());
        assert_eq!(response.metadata.fused_count, 0);
    }
}
