//! Knowledge Graph (KG) Layer Implementation
//!
//! KG provides structured, relational memory using graph database.

use std::collections::HashMap;
use tokio::sync::RwLock;
use crate::kernel::types::*;
use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::traits::{MemoryLayer, LayerStats, GraphMemory};

/// Knowledge Graph memory layer implementation.
///
/// KG stores structured data as nodes and edges in a graph.
/// Uses Neo4j for graph operations.
pub struct KgMemoryLayer {
    // In production, this would hold a Neo4j connection
    // For now, we'll use in-memory storage
    nodes: RwLock<HashMap<String, GraphNode>>,
    edges: RwLock<Vec<GraphEdge>>,
}

impl KgMemoryLayer {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(Vec::new()),
        }
    }
}

impl Default for KgMemoryLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl MemoryLayer for KgMemoryLayer {
    fn layer_type(&self) -> LayerType {
        LayerType::Kg
    }

    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId> {
        match entry.content {
            MemoryContent::Graph(graph_data) => {
                let mut nodes = self.nodes.write().await;
                let mut edges = self.edges.write().await;

                for node in graph_data.nodes {
                    nodes.insert(node.id.clone(), node);
                }

                for edge in graph_data.edges {
                    edges.push(edge);
                }

                Ok(entry.id)
            }
            _ => Err(MemoryError::InvalidOperation(
                "KG layer requires Graph content".to_string()
            ))
        }
    }

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry> {
        let nodes = self.nodes.read().await;

        let node = nodes.get(&id.0)
            .ok_or_else(|| MemoryError::NotFound(format!("Node not found: {}", id.0)))?
            .clone();

        let edges = self.edges.read().await;
        let related_edges: Vec<_> = edges.iter()
            .filter(|e| e.source == id.0 || e.target == id.0)
            .cloned()
            .collect();

        let graph_data = GraphData {
            nodes: vec![node],
            edges: related_edges,
        };
        
        Ok(MemoryEntry {
            id: id.clone(),
            layer: LayerType::Kg,
            content: MemoryContent::Graph(graph_data),
            metadata: MemoryMetadata::default(),
            created_at: 0,
            updated_at: 0,
        })
    }

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        let nodes = self.nodes.read().await;
        let mut results = Vec::new();
        
        // Search by label or properties
        if let Some(text) = &query.text {
            for node in nodes.values() {
                if node.label.contains(text) {
                    let entry = MemoryEntry {
                        id: MemoryId(node.id.clone()),
                        layer: LayerType::Kg,
                        content: MemoryContent::Graph(GraphData {
                            nodes: vec![node.clone()],
                            edges: vec![],
                        }),
                        metadata: MemoryMetadata::default(),
                        created_at: 0,
                        updated_at: 0,
                    };
                    
                    results.push(MemoryMatch {
                        entry,
                        score: 1.0,
                        highlights: vec![text.clone()],
                    });
                }
            }
        }
        
        results.truncate(query.limit);
        Ok(results)
    }

    async fn update(&self, id: &MemoryId, entry: MemoryEntry) -> MemoryResult<()> {
        let mut nodes = self.nodes.write().await;
        
        if let MemoryContent::Graph(graph_data) = entry.content {
            for node in graph_data.nodes {
                nodes.insert(node.id.clone(), node);
            }
        }
        
        Ok(())
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        let mut nodes = self.nodes.write().await;
        let mut edges = self.edges.write().await;
        
        nodes.remove(&id.0);
        edges.retain(|e| e.source != id.0 && e.target != id.0);
        
        Ok(())
    }

    async fn stats(&self) -> MemoryResult<LayerStats> {
        let nodes = self.nodes.read().await;
        let edges = self.edges.read().await;
        
        Ok(LayerStats {
            entry_count: nodes.len(),
            size_bytes: 0,
            avg_access_count: 0.0,
        })
    }
}

#[async_trait::async_trait]
impl GraphMemory for KgMemoryLayer {
    async fn add_node(&self, node: GraphNode) -> MemoryResult<()> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node.id.clone(), node);
        Ok(())
    }

    async fn add_edge(&self, edge: GraphEdge) -> MemoryResult<()> {
        let mut edges = self.edges.write().await;
        edges.push(edge);
        Ok(())
    }

    async fn query_nodes(&self, _labels: &[String], properties: &HashMap<String, serde_json::Value>) -> MemoryResult<Vec<GraphNode>> {
        let nodes = self.nodes.read().await;
        let mut results = Vec::new();
        
        for node in nodes.values() {
            let mut matches = true;
            for (key, value) in properties {
                if node.properties.get(key) != Some(value) {
                    matches = false;
                    break;
                }
            }
            if matches {
                results.push(node.clone());
            }
        }
        
        Ok(results)
    }

    async fn query_edges(&self, from: Option<&str>, to: Option<&str>, relation: Option<&str>) -> MemoryResult<Vec<GraphEdge>> {
        let edges = self.edges.read().await;
        
        let results: Vec<_> = edges.iter()
            .filter(|e| {
                let from_match = from.map(|f| e.source == f).unwrap_or(true);
                let to_match = to.map(|t| e.target == t).unwrap_or(true);
                let rel_match = relation.map(|r| e.relation == r).unwrap_or(true);
                from_match && to_match && rel_match
            })
            .cloned()
            .collect();
        
        Ok(results)
    }

    async fn traverse(&self, start: &str, depth: usize) -> MemoryResult<Vec<GraphNode>> {
        let nodes = self.nodes.read().await;
        let edges = self.edges.read().await;
        
        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![(start.to_string(), 0)];
        let mut results = Vec::new();
        
        while let Some((current, d)) = queue.pop() {
            if visited.contains(&current) || d > depth {
                continue;
            }
            
            visited.insert(current.clone());
            
            if let Some(node) = nodes.get(&current) {
                results.push(node.clone());
            }
            
            // Add neighbors to queue
            for edge in edges.iter() {
                if edge.source == current {
                    queue.push((edge.target.clone(), d + 1));
                }
            }
        }
        
        Ok(results)
    }
}
