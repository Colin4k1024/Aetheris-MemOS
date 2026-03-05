//! Neo4j Graph Database Integration
//!
//! This module provides Neo4j integration for knowledge graph storage.

use std::sync::Arc;
use async_trait::async_trait;
use neo4rs::{Graph, Node, Relation, Row};
use tokio::sync::RwLock;
use tracing::{info, error, warn};
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
    pub async fn execute(&self, query: &str) -> Result<Vec<Row>, AppError> {
        self.graph.execute(query)
            .await
            .map_err(|e| {
                error!("Neo4j query failed: {}", e);
                AppError::Internal(format!("Neo4j query failed: {}", e))
            })?
            .collect::<Result<Vec<_>, _>>()
            .await
            .map_err(|e| {
                error!("Failed to collect Neo4j results: {}", e);
                AppError::Internal(format!("Neo4j result collection failed: {}", e))
            })
    }

    /// Execute a query with parameters
    pub async fn execute_with_params(
        &self,
        query: &str,
        params: impl Into<neo4rs::QueryParameters>,
    ) -> Result<Vec<Row>, AppError> {
        let query = neo4rs::query(query).with(params);
        self.graph.execute(query)
            .await
            .map_err(|e| {
                error!("Neo4j query failed: {}", e);
                AppError::Internal(format!("Neo4j query failed: {}", e))
            })?
            .collect::<Result<Vec<_>, _>>()
            .await
            .map_err(|e| {
                error!("Failed to collect Neo4j results: {}", e);
                AppError::Internal(format!("Neo4j result collection failed: {}", e))
            })
    }

    /// Create a node
    pub async fn create_node(
        &self,
        label: &str,
        properties: impl Into<neo4rs::QueryParameters>,
    ) -> Result<Node, AppError> {
        let query = format!(
            "CREATE (n:{} $props) RETURN n",
            label
        );
        let mut rows = self.execute_with_params(&query, properties).await?;

        rows.pop()
            .and_then(|row| row.get::<Node>("n"))
            .ok_or_else(|| {
                AppError::Internal("Failed to create node".to_string())
            })
    }

    /// Match a node by label and properties
    pub async fn match_node(
        &self,
        label: &str,
        property: &str,
        value: &str,
    ) -> Result<Option<Node>, AppError> {
        let query = format!(
            "MATCH (n:{} {{{}: $value}}) RETURN n",
            label, property
        );
        let mut rows = self.execute_with_params(&query, ("value", value)).await?;

        Ok(rows.pop()
            .and_then(|row| row.get::<Node>("n")))
    }

    /// Create a relationship between two nodes
    pub async fn create_relation(
        &self,
        from_id: &str,
        to_id: &str,
        rel_type: &str,
        properties: impl Into<neo4rs::QueryParameters>,
    ) -> Result<Relation, AppError> {
        let query = format!(
            "MATCH (a), (b) WHERE id(a) = $from_id AND id(b) = $to_id CREATE (a)-[r:{} $props]->(b) RETURN r",
            rel_type
        );
        let mut props = properties.into();
        props.insert("from_id".to_string(), neo4rs::Value::Int(from_id.parse().unwrap_or(0)));
        props.insert("to_id".to_string(), neo4rs::Value::Int(to_id.parse().unwrap_or(0)));

        let mut rows = self.execute_with_params(&query, props).await?;

        rows.pop()
            .and_then(|row| row.get::<Relation>("r"))
            .ok_or_else(|| {
                AppError::Internal("Failed to create relationship".to_string())
            })
    }

    /// Find related nodes
    pub async fn find_related(
        &self,
        node_id: &str,
        rel_type: Option<&str>,
        depth: usize,
    ) -> Result<Vec<Node>, AppError> {
        let rel_pattern = rel_type
            .map(|r| format!("[r:{}]", r))
            .unwrap_or_else(|| "[r]".to_string());

        let query = format!(
            "MATCH (a)-{}->(b) WHERE id(a) = $node_id RETURN b",
            rel_pattern
        );

        let mut rows = self.execute_with_params(
            &query,
            ("node_id", node_id.parse::<i64>().unwrap_or(0)),
        ).await?;

        let mut nodes = Vec::new();
        for row in rows {
            if let Some(node) = row.get::<Node>("b") {
                nodes.push(node);
            }
        }
        Ok(nodes)
    }
}

/// Global Neo4j connection
static NEO4J_MANAGER: std::sync::OnceLock<Arc<Neo4jManager>> = std::sync::OnceLock::new();

/// Initialize Neo4j connection
pub async fn init(config: &Neo4jConfig) -> Result<(), AppError> {
    // Check if password is set
    if config.password == "REPLACE_WITH_YOUR_NEO4J_PASSWORD" || config.password.is_empty() {
        warn!("Neo4j password not configured, skipping Neo4j initialization");
        return Ok(());
    }

    match Neo4jManager::new(config).await {
        Ok(manager) => {
            let _ = NEO4J_MANAGER.set(Arc::new(manager));
            info!("Neo4j connection initialized successfully");
        }
        Err(e) => {
            warn!("Failed to initialize Neo4j: {}, continuing without Neo4j", e);
        }
    }
    Ok(())
}

/// Initialize Neo4j indexes and constraints
pub async fn init_neo4j_indexes() -> Result<(), AppError> {
    let manager = get_manager()?;

    // Create index on entity name
    let queries = vec![
        "CREATE INDEX entity_name_index IF NOT EXISTS FOR (n:Entity) ON (n.name)",
        "CREATE INDEX entity_type_index IF NOT EXISTS FOR (n:Entity) ON (n.entity_type)",
        "CREATE CONSTRAINT entity_id_unique IF NOT EXISTS FOR (n:Entity) REQUIRE n.id IS UNIQUE",
    ];

    for query in queries {
        match manager.execute(query).await {
            Ok(_) => info!("Created Neo4j index: {}", query),
            Err(e) => warn!("Failed to create index (may already exist): {}", e),
        }
    }

    Ok(())
}

/// Get Neo4j manager instance
pub fn get_manager() -> Result<Arc<Neo4jManager>, AppError> {
    NEO4J_MANAGER
        .get()
        .cloned()
        .ok_or_else(|| AppError::Internal("Neo4j not initialized".to_string()))
}

/// Check if Neo4j is available
pub fn is_available() -> bool {
    NEO4J_MANAGER.get().is_some()
}

// ========== Entity Operations ==========

/// Entity structure for Neo4j
#[derive(Debug, Clone)]
pub struct Neo4jEntity {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub description: Option<String>,
}

impl From<Node> for Neo4jEntity {
    fn from(node: Node) -> Self {
        Self {
            id: node.id().to_string(),
            name: node.get("name").unwrap_or_default(),
            entity_type: node.get("entity_type").unwrap_or_default(),
            description: node.get("description").ok(),
        }
    }
}

/// Create entity in Neo4j
pub async fn create_entity(entity: &Neo4jEntity) -> Result<(), AppError> {
    let manager = get_manager()?;

    let props = vec![
        ("id".to_string(), neo4rs::Value::String(entity.id.clone())),
        ("name".to_string(), neo4rs::Value::String(entity.name.clone())),
        ("entity_type".to_string(), neo4rs::Value::String(entity.entity_type.clone())),
    ];

    if let Some(desc) = &entity.description {
        props.push(("description".to_string(), neo4rs::Value::String(desc.clone())));
    }

    manager.create_node("Entity", props).await?;

    Ok(())
}

/// Create relationship in Neo4j
pub async fn create_relationship(
    from_id: &str,
    to_id: &str,
    rel_type: &str,
    properties: Option<serde_json::Value>,
) -> Result<(), AppError> {
    let manager = get_manager()?;

    let mut props = vec![
        ("type".to_string(), neo4rs::Value::String(rel_type.to_string())),
    ];

    if let Some(props_json) = properties {
        props.push(("properties".to_string(), neo4rs::Value::String(props_json.to_string())));
    }

    manager.create_relation(from_id, to_id, rel_type, props).await?;

    Ok(())
}

/// Get entity by name
pub async fn get_entity_by_name(name: &str) -> Result<Option<Neo4jEntity>, AppError> {
    let manager = get_manager()?;

    let node = manager.match_node("Entity", "name", name).await?;

    Ok(node.map(Neo4jEntity::from))
}

/// Find related entities
pub async fn find_related_entities(
    entity_id: &str,
    rel_type: Option<&str>,
) -> Result<Vec<Neo4jEntity>, AppError> {
    let manager = get_manager()?;

    let nodes = manager.find_related(entity_id, rel_type, 1).await?;

    Ok(nodes.into_iter().map(Neo4jEntity::from).collect())
}
