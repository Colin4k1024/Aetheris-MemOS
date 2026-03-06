//! Neo4j Graph Database Integration
//!
//! This module provides Neo4j integration for knowledge graph storage.

use std::sync::Arc;
use neo4rs::{Graph, Node, Relation, Row, query};
use tokio::sync::RwLock;
use tracing::{info, error};
use crate::AppError;
use crate::config::Neo4jConfig;

/// Neo4j connection manager
pub struct Neo4jManager {
    graph: Graph,
    database: String,
}

impl Neo4jManager {
    /// Create a new Neo4j manager
    pub async fn new(config: &Neo4jConfig) -> Result<Self, AppError> {
        let uri = format!("{}:{}", config.host, config.port);
        info!("Connecting to Neo4j at {}", uri);

        let graph = Graph::new(&uri, &config.username, &config.password)
            .await
            .map_err(|e| {
                error!("Failed to connect to Neo4j: {}", e);
                AppError::Internal(format!("Neo4j connection failed: {}", e))
            })?;

        info!("Successfully connected to Neo4j");

        Ok(Self {
            graph,
            database: config.database.clone(),
        })
    }

    /// Execute a query and return results
    pub async fn execute(&self, query_str: &str) -> Result<Vec<Row>, AppError> {
        let q = query(query_str);
        let mut rows = self.graph.execute(q).await
            .map_err(|e| {
                error!("Neo4j query failed: {}", e);
                AppError::Internal(format!("Neo4j query failed: {}", e))
            })?;

        let mut results = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            results.push(row);
        }
        Ok(results)
    }

    /// Execute a query with parameters
    pub async fn execute_with_params(
        &self,
        query_str: &str,
    ) -> Result<Vec<Row>, AppError> {
        let q = query(query_str);
        let mut rows = self.graph.execute(q).await
            .map_err(|e| {
                error!("Neo4j query failed: {}", e);
                AppError::Internal(format!("Neo4j query failed: {}", e))
            })?;

        let mut results = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            results.push(row);
        }
        Ok(results)
    }

    /// Create a node
    pub async fn create_node(
        &self,
        label: &str,
        properties: std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Node, AppError> {
        let props: Vec<(String, serde_json::Value)> = properties.into_iter().collect();
        let set_clauses: Vec<String> = props.iter()
            .map(|(k, _)| format!("n.{} = ${}", k, k))
            .collect();

        let query_str = if set_clauses.is_empty() {
            format!("CREATE (n:{}) RETURN n", label)
        } else {
            format!("CREATE (n:{} {{ {} }}) RETURN n", label, set_clauses.join(", "))
        };

        let mut q = query(&query_str);
        for (k, v) in props {
            q = q.param(&k, json_to_bolt(v));
        }

        let mut rows = self.graph.execute(q).await
            .map_err(|e| AppError::Internal(format!("Neo4j create node failed: {}", e)))?;

        match rows.next().await {
            Ok(Some(row)) => row.get("n")
                .map_err(|e| AppError::Internal(format!("Failed to get node: {}", e))),
            Ok(None) => Err(AppError::Internal("No node returned".to_string())),
            Err(e) => Err(AppError::Internal(format!("Failed to create node: {}", e))),
        }
    }

    /// Get a node by ID
    pub async fn get_node(&self, node_id: i64) -> Result<Option<Node>, AppError> {
        let q = query("MATCH (n) WHERE id(n) = $id RETURN n")
            .param("id", node_id);

        let mut rows = self.graph.execute(q).await
            .map_err(|e| AppError::Internal(format!("Neo4j get node failed: {}", e)))?;

        match rows.next().await {
            Ok(Some(row)) => {
                match row.get::<Node>("n") {
                    Ok(node) => Ok(Some(node)),
                    Err(_) => Ok(None),
                }
            }
            Ok(None) => Ok(None),
            Err(e) => Err(AppError::Internal(format!("Failed to get node: {}", e))),
        }
    }

    /// Find nodes by label and properties
    pub async fn find_nodes(
        &self,
        label: &str,
        properties: std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Vec<Node>, AppError> {
        let props: Vec<(String, serde_json::Value)> = properties.into_iter().collect();
        let where_clauses: Vec<String> = props.iter()
            .map(|(k, _)| format!("n.{} = ${}", k, k))
            .collect();

        let query_str = if where_clauses.is_empty() {
            format!("MATCH (n:{}) RETURN n", label)
        } else {
            format!("MATCH (n:{}) WHERE {} RETURN n", label, where_clauses.join(" AND "))
        };

        let mut q = query(&query_str);
        for (k, v) in props {
            q = q.param(&k, json_to_bolt(v));
        }

        let mut rows = self.graph.execute(q).await
            .map_err(|e| AppError::Internal(format!("Neo4j find nodes failed: {}", e)))?;

        let mut results = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            if let Ok(node) = row.get::<Node>("n") {
                results.push(node);
            }
        }
        Ok(results)
    }

    /// Create a relationship
    pub async fn create_relationship(
        &self,
        from_id: i64,
        to_id: i64,
        rel_type: &str,
        properties: std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Relation, AppError> {
        let props: Vec<(String, serde_json::Value)> = properties.into_iter().collect();
        let set_clauses: Vec<String> = props.iter()
            .map(|(k, _)| format!("r.{} = ${}", k, k))
            .collect();

        let query_str = if set_clauses.is_empty() {
            format!(
                "MATCH (a), (b) WHERE id(a) = $from_id AND id(b) = $to_id CREATE (a)-[r:{}]->(b) RETURN r",
                rel_type
            )
        } else {
            format!(
                "MATCH (a), (b) WHERE id(a) = $from_id AND id(b) = $to_id CREATE (a)-[r:{} {{ {} }}]->(b) RETURN r",
                rel_type,
                set_clauses.join(", ")
            )
        };

        let mut q = query(&query_str)
            .param("from_id", from_id)
            .param("to_id", to_id);
        for (k, v) in props {
            q = q.param(&k, json_to_bolt(v));
        }

        let mut rows = self.graph.execute(q).await
            .map_err(|e| AppError::Internal(format!("Neo4j create relationship failed: {}", e)))?;

        match rows.next().await {
            Ok(Some(row)) => row.get("r")
                .map_err(|e| AppError::Internal(format!("Failed to get relation: {}", e))),
            Ok(None) => Err(AppError::Internal("No relationship returned".to_string())),
            Err(e) => Err(AppError::Internal(format!("Failed to create relationship: {}", e))),
        }
    }

    /// Find relationships between nodes
    pub async fn find_relationships(
        &self,
        from_id: Option<i64>,
        to_id: Option<i64>,
        rel_type: Option<&str>,
    ) -> Result<Vec<Relation>, AppError> {
        let mut conditions = Vec::new();
        let mut params: Vec<(&str, i64)> = Vec::new();

        if let Some(fid) = from_id {
            conditions.push("id(startNode(r)) = $from_id".to_string());
            params.push(("from_id", fid));
        }
        if let Some(tid) = to_id {
            conditions.push("id(endNode(r)) = $to_id".to_string());
            params.push(("to_id", tid));
        }

        let query_str = match (rel_type, conditions.is_empty()) {
            (Some(rt), true) => format!("MATCH (a)-[r:{}]->(b) RETURN r", rt),
            (Some(rt), false) => format!(
                "MATCH (a)-[r:{}]->(b) WHERE {} RETURN r",
                rt,
                conditions.join(" AND ")
            ),
            (None, true) => "MATCH (a)-[r]->(b) RETURN r".to_string(),
            (None, false) => format!(
                "MATCH (a)-[r]->(b) WHERE {} RETURN r",
                conditions.join(" AND ")
            ),
        };

        let mut q = query(&query_str);
        for (k, v) in params {
            q = q.param(k, v);
        }

        let mut rows = self.graph.execute(q).await
            .map_err(|e| AppError::Internal(format!("Neo4j find relationships failed: {}", e)))?;

        let mut results = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            if let Ok(rel) = row.get::<Relation>("r") {
                results.push(rel);
            }
        }
        Ok(results)
    }

    /// Delete a node
    pub async fn delete_node(&self, node_id: i64) -> Result<bool, AppError> {
        let q = query("MATCH (n) WHERE id(n) = $id DETACH DELETE n")
            .param("id", node_id);

        let mut rows = self.graph.execute(q).await
            .map_err(|e| AppError::Internal(format!("Neo4j delete node failed: {}", e)))?;

        // Just consume the result
        while let Ok(Some(_)) = rows.next().await {}
        Ok(true)
    }

    /// Delete a relationship
    pub async fn delete_relationship(&self, rel_id: i64) -> Result<bool, AppError> {
        let q = query("MATCH ()-[r]->() WHERE id(r) = $id DELETE r")
            .param("id", rel_id);

        let mut rows = self.graph.execute(q).await
            .map_err(|e| AppError::Internal(format!("Neo4j delete relationship failed: {}", e)))?;

        while let Ok(Some(_)) = rows.next().await {}
        Ok(true)
    }
}

/// Type alias for Neo4j manager wrapped in Arc and RwLock
pub type Neo4jManagerHandle = Arc<RwLock<Option<Neo4jManager>>>;

/// Create a new Neo4j manager handle
pub fn create_neo4j_manager() -> Neo4jManagerHandle {
    Arc::new(RwLock::new(None))
}

/// Initialize Neo4j connection and indexes
pub async fn init_neo4j(config: &Neo4jConfig) -> Result<Neo4jManagerHandle, AppError> {
    let manager = Neo4jManager::new(config).await?;
    let handle: Neo4jManagerHandle = Arc::new(RwLock::new(Some(manager)));
    Ok(handle)
}

/// Initialize Neo4j indexes
pub async fn init_neo4j_indexes() -> Result<(), AppError> {
    // Index creation can be added here if needed
    Ok(())
}

/// Helper function to convert serde_json::Value to a BoltType-compatible value
fn json_to_bolt(value: serde_json::Value) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string())
}
