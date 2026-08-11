//! Neo4j Graph Database Integration
//!
//! This module provides Neo4j integration for knowledge graph storage.

use crate::config::Neo4jConfig;
use crate::AppError;
use neo4rs::{query, Graph, Node, Relation, Row};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Neo4j connection manager
pub struct Neo4jManager {
    graph: Graph,
    database: String,
}

impl Neo4jManager {
    /// Create a new Neo4j manager.
    ///
    /// `Graph::new` only builds a lazy connection pool — it performs no I/O and
    /// so cannot detect a bad host or wrong credentials. We therefore issue a
    /// `RETURN 1` probe here so a failure surfaces at connect time (and the
    /// caller can log the real status) rather than on the first knowledge-graph
    /// write. NOTE: neo4rs retries a failed query with exponential backoff up to
    /// its internal 60s `max_elapsed_time`, so this call can block for up to ~60s
    /// on an unreachable host or wrong password. Callers MUST wrap it in a
    /// timeout and run it off the startup critical path (see [`spawn_neo4j_init`]).
    pub async fn new(config: &Neo4jConfig) -> Result<Self, AppError> {
        let uri = format!("{}:{}", config.host, config.port);
        info!("Connecting to Neo4j at {}", uri);

        let graph = Graph::new(&uri, &config.username, &config.password)
            .await
            .map_err(|e| {
                error!("Failed to connect to Neo4j: {}", e);
                AppError::Internal(format!("Neo4j connection failed: {}", e))
            })?;

        // Real connectivity probe — forces the lazy pool to open a connection and
        // complete the auth handshake, so "connected" reflects reality.
        graph.run(query("RETURN 1")).await.map_err(|e| {
            error!("Neo4j connectivity probe failed: {}", e);
            AppError::Internal(format!("Neo4j connectivity probe failed: {}", e))
        })?;

        info!("Successfully connected to Neo4j at {}", uri);

        Ok(Self {
            graph,
            database: config.database.clone(),
        })
    }

    /// Execute a query and return results
    pub async fn execute(&self, query_str: &str) -> Result<Vec<Row>, AppError> {
        let q = query(query_str);
        let mut rows = self.graph.execute(q).await.map_err(|e| {
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
    pub async fn execute_with_params(&self, query_str: &str) -> Result<Vec<Row>, AppError> {
        let q = query(query_str);
        let mut rows = self.graph.execute(q).await.map_err(|e| {
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
        let set_clauses: Vec<String> = props
            .iter()
            .map(|(k, _)| format!("n.{} = ${}", k, k))
            .collect();

        let query_str = if set_clauses.is_empty() {
            format!("CREATE (n:{}) RETURN n", label)
        } else {
            format!(
                "CREATE (n:{} {{ {} }}) RETURN n",
                label,
                set_clauses.join(", ")
            )
        };

        let mut q = query(&query_str);
        for (k, v) in props {
            q = q.param(&k, json_to_bolt(v));
        }

        let mut rows = self
            .graph
            .execute(q)
            .await
            .map_err(|e| AppError::Internal(format!("Neo4j create node failed: {}", e)))?;

        match rows.next().await {
            Ok(Some(row)) => row
                .get("n")
                .map_err(|e| AppError::Internal(format!("Failed to get node: {}", e))),
            Ok(None) => Err(AppError::Internal("No node returned".to_string())),
            Err(e) => Err(AppError::Internal(format!("Failed to create node: {}", e))),
        }
    }

    /// Get a node by ID
    pub async fn get_node(&self, node_id: i64) -> Result<Option<Node>, AppError> {
        let q = query("MATCH (n) WHERE id(n) = $id RETURN n").param("id", node_id);

        let mut rows = self
            .graph
            .execute(q)
            .await
            .map_err(|e| AppError::Internal(format!("Neo4j get node failed: {}", e)))?;

        match rows.next().await {
            Ok(Some(row)) => match row.get::<Node>("n") {
                Ok(node) => Ok(Some(node)),
                Err(_) => Ok(None),
            },
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
        let where_clauses: Vec<String> = props
            .iter()
            .map(|(k, _)| format!("n.{} = ${}", k, k))
            .collect();

        let query_str = if where_clauses.is_empty() {
            format!("MATCH (n:{}) RETURN n", label)
        } else {
            format!(
                "MATCH (n:{}) WHERE {} RETURN n",
                label,
                where_clauses.join(" AND ")
            )
        };

        let mut q = query(&query_str);
        for (k, v) in props {
            q = q.param(&k, json_to_bolt(v));
        }

        let mut rows = self
            .graph
            .execute(q)
            .await
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
        let set_clauses: Vec<String> = props
            .iter()
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

        let mut rows =
            self.graph.execute(q).await.map_err(|e| {
                AppError::Internal(format!("Neo4j create relationship failed: {}", e))
            })?;

        match rows.next().await {
            Ok(Some(row)) => row
                .get("r")
                .map_err(|e| AppError::Internal(format!("Failed to get relation: {}", e))),
            Ok(None) => Err(AppError::Internal("No relationship returned".to_string())),
            Err(e) => Err(AppError::Internal(format!(
                "Failed to create relationship: {}",
                e
            ))),
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

        let mut rows =
            self.graph.execute(q).await.map_err(|e| {
                AppError::Internal(format!("Neo4j find relationships failed: {}", e))
            })?;

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
        let q = query("MATCH (n) WHERE id(n) = $id DETACH DELETE n").param("id", node_id);

        let mut rows = self
            .graph
            .execute(q)
            .await
            .map_err(|e| AppError::Internal(format!("Neo4j delete node failed: {}", e)))?;

        // Just consume the result
        while let Ok(Some(_)) = rows.next().await {}
        Ok(true)
    }

    /// Delete a relationship
    pub async fn delete_relationship(&self, rel_id: i64) -> Result<bool, AppError> {
        let q = query("MATCH ()-[r]->() WHERE id(r) = $id DELETE r").param("id", rel_id);

        let mut rows =
            self.graph.execute(q).await.map_err(|e| {
                AppError::Internal(format!("Neo4j delete relationship failed: {}", e))
            })?;

        while let Ok(Some(_)) = rows.next().await {}
        Ok(true)
    }

    /// Count nodes by label
    pub async fn count_nodes_by_label(&self, label: &str) -> Result<i64, AppError> {
        let q = query("MATCH (n:$label) RETURN count(n) as count").param("label", label);

        let mut rows = self
            .graph
            .execute(q)
            .await
            .map_err(|e| AppError::Internal(format!("Neo4j count nodes failed: {}", e)))?;

        if let Ok(Some(row)) = rows.next().await {
            if let Ok(count) = row.get::<i64>("count") {
                return Ok(count);
            }
        }
        Ok(0)
    }

    /// Execute match query and return all nodes
    pub async fn match_nodes(
        &self,
        label: &str,
        properties: Option<std::collections::HashMap<String, serde_json::Value>>,
        limit: Option<usize>,
    ) -> Result<Vec<Node>, AppError> {
        let limit_clause = limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default();

        let query_str = match &properties {
            Some(props) if !props.is_empty() => {
                let where_clauses: Vec<String> =
                    props.keys().map(|k| format!("n.{} = ${}", k, k)).collect();
                format!(
                    "MATCH (n:{}) WHERE {} RETURN n{}",
                    label,
                    where_clauses.join(" AND "),
                    limit_clause
                )
            }
            _ => format!("MATCH (n:{}) RETURN n{}", label, limit_clause),
        };

        let mut q = query(&query_str);
        if let Some(props) = properties {
            for (k, v) in props {
                q = q.param(&k, json_to_bolt(v));
            }
        }

        let mut rows = self
            .graph
            .execute(q)
            .await
            .map_err(|e| AppError::Internal(format!("Neo4j match nodes failed: {}", e)))?;

        let mut nodes = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            if let Ok(node) = row.get::<Node>("n") {
                nodes.push(node);
            }
        }
        Ok(nodes)
    }
}

/// Type alias for Neo4j manager wrapped in Arc and RwLock
pub type Neo4jManagerHandle = Arc<RwLock<Option<Neo4jManager>>>;

/// Global Neo4j manager handle, set by `init_neo4j` at startup.
static NEO4J_HANDLE: OnceLock<Neo4jManagerHandle> = OnceLock::new();

/// Observable connection status for the optional Neo4j dependency.
///
/// Neo4j is initialised off the startup critical path, so the process can be
/// serving HTTP while the connection attempt is still in flight (or has failed).
/// This lets callers (health reporting, logs) read the *real* state instead of
/// trusting an optimistic startup log. It is deliberately NOT wired into
/// `/readyz`: Neo4j is optional, and failing readiness on its absence would
/// evict an otherwise-healthy instance from the load balancer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Neo4jStatus {
    /// Never attempted (e.g. placeholder password rejected before connecting).
    Disabled = 0,
    /// Background connect in progress.
    Connecting = 1,
    /// Connected and connectivity-probed successfully.
    Connected = 2,
    /// Connect attempt failed (unreachable, wrong credentials, or timeout).
    Failed = 3,
}

static NEO4J_STATUS: AtomicU8 = AtomicU8::new(Neo4jStatus::Disabled as u8);

fn set_status(status: Neo4jStatus) {
    NEO4J_STATUS.store(status as u8, Ordering::Relaxed);
}

/// Current, observable Neo4j connection status.
pub fn neo4j_status() -> Neo4jStatus {
    match NEO4J_STATUS.load(Ordering::Relaxed) {
        2 => Neo4jStatus::Connected,
        1 => Neo4jStatus::Connecting,
        3 => Neo4jStatus::Failed,
        _ => Neo4jStatus::Disabled,
    }
}

/// Known placeholder / example Neo4j passwords shipped in the repo or compose
/// files. Connecting with one of these is pointless (it will never authenticate)
/// and, before this guard, cost ~60s of blocking backoff per query. Unlike the
/// JWT secret check (which aborts the process), a placeholder here only *skips*
/// the Neo4j connection: Neo4j is optional, so the service must still start.
const PLACEHOLDER_NEO4J_PASSWORDS: &[&str] = &[
    "REPLACE_WITH_YOUR_NEO4J_PASSWORD",
    "your-neo4j-password",
    "password",
    "neo4j",
    "change-me",
    "changeme",
];

/// Total wall-clock budget for the background connect attempt. neo4rs retries a
/// failed query with exponential backoff up to its own 60s ceiling; we cap the
/// whole attempt so a mis-set host/password can never hold a connection slot (or
/// leak backoff work) indefinitely.
const NEO4J_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Returns true when `password` is a known placeholder that will never
/// authenticate. Trimmed and case-insensitive to catch copy-paste variants.
fn is_placeholder_password(password: &str) -> bool {
    let pw = password.trim();
    PLACEHOLDER_NEO4J_PASSWORDS
        .iter()
        .any(|p| p.eq_ignore_ascii_case(pw))
}

/// Create a new Neo4j manager handle
pub fn create_neo4j_manager() -> Neo4jManagerHandle {
    Arc::new(RwLock::new(None))
}

/// Spawn Neo4j connection + index initialisation off the startup critical path.
///
/// Returns immediately so the HTTP server can start listening without waiting on
/// Neo4j (an optional dependency). The actual connect + index creation runs in a
/// background task, bounded by [`NEO4J_CONNECT_TIMEOUT`]. Idempotent: repeat
/// calls after the first are no-ops.
///
/// A placeholder password (see [`PLACEHOLDER_NEO4J_PASSWORDS`]) is rejected up
/// front — the service continues without Neo4j and the status is left
/// [`Neo4jStatus::Disabled`], instead of burning the timeout on an auth attempt
/// that can never succeed.
pub fn spawn_neo4j_init(config: &Neo4jConfig) {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }

    if is_placeholder_password(&config.password) {
        set_status(Neo4jStatus::Disabled);
        warn!(
            host = %config.host,
            port = config.port,
            "Neo4j password is a placeholder — skipping connection. Knowledge-graph \
             features are disabled. Set a real password via APP_NEO4J_PASSWORD to enable Neo4j."
        );
        return;
    }

    set_status(Neo4jStatus::Connecting);
    let config = config.clone();
    tokio::spawn(async move {
        match tokio::time::timeout(NEO4J_CONNECT_TIMEOUT, init_neo4j(&config)).await {
            Ok(Ok(_)) => {
                set_status(Neo4jStatus::Connected);
                // Index creation is best-effort and never fatal.
                let _ = init_neo4j_indexes().await;
            }
            Ok(Err(e)) => {
                set_status(Neo4jStatus::Failed);
                warn!(
                    error = %e,
                    "Neo4j connection failed — continuing without knowledge-graph features"
                );
            }
            Err(_) => {
                set_status(Neo4jStatus::Failed);
                warn!(
                    timeout_secs = NEO4J_CONNECT_TIMEOUT.as_secs(),
                    "Neo4j connection timed out — continuing without knowledge-graph features"
                );
            }
        }
    });
}

/// Initialize Neo4j connection and indexes
pub async fn init_neo4j(config: &Neo4jConfig) -> Result<Neo4jManagerHandle, AppError> {
    let manager = Neo4jManager::new(config).await?;
    let handle: Neo4jManagerHandle = Arc::new(RwLock::new(Some(manager)));
    let _ = NEO4J_HANDLE.set(handle.clone());
    Ok(handle)
}

/// Initialize Neo4j indexes (best-effort).
///
/// Attempts to create a uniqueness constraint on Entity nodes and property
/// indexes on relations. Uses the global Neo4j manager handle set by `init_neo4j`.
/// Failures are logged but never propagated — Neo4j is an optional enhancement.
pub async fn init_neo4j_indexes() -> Result<(), AppError> {
    let handle = match NEO4J_HANDLE.get() {
        Some(h) => h,
        None => {
            tracing::warn!("Neo4j manager not initialized, skipping index creation");
            return Ok(());
        }
    };
    let guard = handle.read().await;
    let manager = match guard.as_ref() {
        Some(m) => m,
        None => {
            tracing::warn!("Neo4j connection not available, skipping index creation");
            return Ok(());
        }
    };

    let queries = [
        "CREATE CONSTRAINT entity_name_unique IF NOT EXISTS FOR (e:Entity) REQUIRE e.name IS UNIQUE",
        "CREATE INDEX entity_type_index IF NOT EXISTS FOR (e:Entity) ON (e.entity_type)",
        "CREATE INDEX relation_type_index IF NOT EXISTS FOR ()-[r:RELATES_TO]-() ON (r.relation_type)",
    ];

    for query in &queries {
        match manager.execute(query).await {
            Ok(_) => tracing::info!(query = %query, "Neo4j index/constraint created"),
            Err(e) => {
                tracing::error!(query = %query, error = %e, "Neo4j index/constraint creation failed")
            }
        }
    }

    tracing::info!("Neo4j index initialization complete");
    Ok(())
}

/// Helper function to convert serde_json::Value to a BoltType-compatible value
fn json_to_bolt(value: serde_json::Value) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_shipped_placeholder_password() {
        // The default from config/neo4j_config.rs must be treated as a placeholder.
        assert!(is_placeholder_password("REPLACE_WITH_YOUR_NEO4J_PASSWORD"));
    }

    #[test]
    fn placeholder_check_is_trimmed_and_case_insensitive() {
        assert!(is_placeholder_password("  password  "));
        assert!(is_placeholder_password("NEO4J"));
        assert!(is_placeholder_password("ChangeMe"));
    }

    #[test]
    fn accepts_a_real_looking_password() {
        assert!(!is_placeholder_password("s3cr3t-Aa1!-random-value"));
        assert!(!is_placeholder_password("passwords")); // not an exact match
    }

    #[test]
    fn status_round_trips_through_atomic() {
        // Default is Disabled before any init.
        set_status(Neo4jStatus::Connecting);
        assert_eq!(neo4j_status(), Neo4jStatus::Connecting);
        set_status(Neo4jStatus::Connected);
        assert_eq!(neo4j_status(), Neo4jStatus::Connected);
        set_status(Neo4jStatus::Failed);
        assert_eq!(neo4j_status(), Neo4jStatus::Failed);
        set_status(Neo4jStatus::Disabled);
        assert_eq!(neo4j_status(), Neo4jStatus::Disabled);
    }
}
