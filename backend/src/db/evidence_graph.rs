use sqlx::types::Json;
use tracing::error;

use crate::db::{DatabasePool, DATABASE_POOL};
use crate::models::{
    WorkflowEvidenceEdge, WorkflowEvidenceMap, WorkflowEvidenceNode, WorkflowEvidenceRun,
    WorkflowEvidenceToolInvocation,
};
use crate::AppError;

pub struct StoredWorkflowEvidence {
    pub run: WorkflowEvidenceRun,
    pub nodes: Vec<WorkflowEvidenceNode>,
    pub edges: Vec<WorkflowEvidenceEdge>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct WorkflowEvidenceRunRow {
    run_id: String,
    workflow_id: String,
    task_id: String,
    attempt_id: String,
    timestamp: String,
    sequence_number: i64,
    prev_hash: Option<String>,
    node_hash: String,
    tool_invocations: Json<Vec<WorkflowEvidenceToolInvocation>>,
    context_snapshot: Json<WorkflowEvidenceMap>,
    metadata: Json<WorkflowEvidenceMap>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct WorkflowEvidenceNodeRow {
    node_id: String,
    run_id: String,
    workflow_id: String,
    task_id: String,
    attempt_id: String,
    sequence_number: i64,
    node_kind: String,
    timestamp: String,
    llm_input_hash: String,
    llm_output_hash: String,
    tool_invocations: Json<Vec<WorkflowEvidenceToolInvocation>>,
    context_snapshot: Json<WorkflowEvidenceMap>,
    metadata: Json<WorkflowEvidenceMap>,
    prev_hash: Option<String>,
    node_hash: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct WorkflowEvidenceEdgeRow {
    edge_id: String,
    run_id: String,
    workflow_id: String,
    task_id: String,
    attempt_id: String,
    sequence_number: i64,
    source_node_id: String,
    target_node_id: String,
    edge_kind: String,
    timestamp: String,
    tool_invocations: Json<Vec<WorkflowEvidenceToolInvocation>>,
    context_snapshot: Json<WorkflowEvidenceMap>,
    metadata: Json<WorkflowEvidenceMap>,
    prev_hash: Option<String>,
    node_hash: String,
}

pub struct EvidenceGraphRepository;

impl EvidenceGraphRepository {
    // This repository supports PostgreSQL and SQLite at runtime, so shared statements use
    // `sqlx::query` / `sqlx::query_as` instead of backend-locked `sqlx::query!` / `sqlx::query_as!`.
    pub async fn create_run(mut run: WorkflowEvidenceRun) -> Result<WorkflowEvidenceRun, AppError> {
        run.sequence_number = Self::next_run_sequence(&run.workflow_id).await?;

        match DATABASE_POOL.get() {
            Some(DatabasePool::Postgres(pool)) => {
                sqlx::query(
                    r#"
                    INSERT INTO workflow_evidence_runs (
                        run_id, workflow_id, task_id, attempt_id, timestamp, sequence_number,
                        prev_hash, node_hash, tool_invocations, context_snapshot, metadata
                    ) VALUES ($1, $2, $3, $4, $5::timestamptz, $6, $7, $8, $9, $10, $11)
                    "#,
                )
                .bind(&run.run_id)
                .bind(&run.workflow_id)
                .bind(&run.task_id)
                .bind(&run.attempt_id)
                .bind(&run.timestamp)
                .bind(run.sequence_number)
                .bind(&run.prev_hash)
                .bind(&run.node_hash)
                .bind(Json(&run.tool_invocations))
                .bind(Json(&run.context_snapshot))
                .bind(Json(&run.metadata))
                .execute(pool)
                .await
                .map_err(|err| db_error("create workflow evidence run", err))?;
            }
            Some(DatabasePool::Sqlite(pool)) => {
                sqlx::query(
                    r#"
                    INSERT INTO workflow_evidence_runs (
                        run_id, workflow_id, task_id, attempt_id, timestamp, sequence_number,
                        prev_hash, node_hash, tool_invocations, context_snapshot, metadata
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                    "#,
                )
                .bind(&run.run_id)
                .bind(&run.workflow_id)
                .bind(&run.task_id)
                .bind(&run.attempt_id)
                .bind(&run.timestamp)
                .bind(run.sequence_number)
                .bind(&run.prev_hash)
                .bind(&run.node_hash)
                .bind(Json(&run.tool_invocations))
                .bind(Json(&run.context_snapshot))
                .bind(Json(&run.metadata))
                .execute(pool)
                .await
                .map_err(|err| db_error("create workflow evidence run", err))?;
            }
            None => {
                return Err(AppError::DatabaseConnection(
                    "database pool not initialized".into(),
                ))
            }
        }

        Ok(run)
    }

    pub async fn append_nodes(nodes: &[WorkflowEvidenceNode]) -> Result<(), AppError> {
        for node in nodes {
            match DATABASE_POOL.get() {
                Some(DatabasePool::Postgres(pool)) => {
                    sqlx::query(
                        r#"
                        INSERT INTO workflow_evidence_nodes (
                            node_id, run_id, workflow_id, task_id, attempt_id, sequence_number,
                            node_kind, timestamp, llm_input_hash, llm_output_hash,
                            tool_invocations, context_snapshot, metadata, prev_hash, node_hash
                        ) VALUES (
                            $1, $2, $3, $4, $5, $6,
                            $7, $8::timestamptz, $9, $10,
                            $11, $12, $13, $14, $15
                        )
                        "#,
                    )
                    .bind(&node.node_id)
                    .bind(&node.run_id)
                    .bind(&node.workflow_id)
                    .bind(&node.task_id)
                    .bind(&node.attempt_id)
                    .bind(node.sequence_number)
                    .bind(&node.node_kind)
                    .bind(&node.timestamp)
                    .bind(&node.llm_input_hash)
                    .bind(&node.llm_output_hash)
                    .bind(Json(&node.tool_invocations))
                    .bind(Json(&node.context_snapshot))
                    .bind(Json(&node.metadata))
                    .bind(&node.prev_hash)
                    .bind(&node.node_hash)
                    .execute(pool)
                    .await
                    .map_err(|err| db_error("append workflow evidence node", err))?;
                }
                Some(DatabasePool::Sqlite(pool)) => {
                    sqlx::query(
                        r#"
                        INSERT INTO workflow_evidence_nodes (
                            node_id, run_id, workflow_id, task_id, attempt_id, sequence_number,
                            node_kind, timestamp, llm_input_hash, llm_output_hash,
                            tool_invocations, context_snapshot, metadata, prev_hash, node_hash
                        ) VALUES (
                            $1, $2, $3, $4, $5, $6,
                            $7, $8, $9, $10,
                            $11, $12, $13, $14, $15
                        )
                        "#,
                    )
                    .bind(&node.node_id)
                    .bind(&node.run_id)
                    .bind(&node.workflow_id)
                    .bind(&node.task_id)
                    .bind(&node.attempt_id)
                    .bind(node.sequence_number)
                    .bind(&node.node_kind)
                    .bind(&node.timestamp)
                    .bind(&node.llm_input_hash)
                    .bind(&node.llm_output_hash)
                    .bind(Json(&node.tool_invocations))
                    .bind(Json(&node.context_snapshot))
                    .bind(Json(&node.metadata))
                    .bind(&node.prev_hash)
                    .bind(&node.node_hash)
                    .execute(pool)
                    .await
                    .map_err(|err| db_error("append workflow evidence node", err))?;
                }
                None => {
                    return Err(AppError::DatabaseConnection(
                        "database pool not initialized".into(),
                    ))
                }
            }
        }

        Ok(())
    }

    pub async fn append_edges(edges: &[WorkflowEvidenceEdge]) -> Result<(), AppError> {
        for edge in edges {
            match DATABASE_POOL.get() {
                Some(DatabasePool::Postgres(pool)) => {
                    sqlx::query(
                        r#"
                        INSERT INTO workflow_evidence_edges (
                            edge_id, run_id, workflow_id, task_id, attempt_id, sequence_number,
                            source_node_id, target_node_id, edge_kind, timestamp,
                            tool_invocations, context_snapshot, metadata, prev_hash, node_hash
                        ) VALUES (
                            $1, $2, $3, $4, $5, $6,
                            $7, $8, $9, $10::timestamptz,
                            $11, $12, $13, $14, $15
                        )
                        "#,
                    )
                    .bind(&edge.edge_id)
                    .bind(&edge.run_id)
                    .bind(&edge.workflow_id)
                    .bind(&edge.task_id)
                    .bind(&edge.attempt_id)
                    .bind(edge.sequence_number)
                    .bind(&edge.source_node_id)
                    .bind(&edge.target_node_id)
                    .bind(&edge.edge_kind)
                    .bind(&edge.timestamp)
                    .bind(Json(&edge.tool_invocations))
                    .bind(Json(&edge.context_snapshot))
                    .bind(Json(&edge.metadata))
                    .bind(&edge.prev_hash)
                    .bind(&edge.node_hash)
                    .execute(pool)
                    .await
                    .map_err(|err| db_error("append workflow evidence edge", err))?;
                }
                Some(DatabasePool::Sqlite(pool)) => {
                    sqlx::query(
                        r#"
                        INSERT INTO workflow_evidence_edges (
                            edge_id, run_id, workflow_id, task_id, attempt_id, sequence_number,
                            source_node_id, target_node_id, edge_kind, timestamp,
                            tool_invocations, context_snapshot, metadata, prev_hash, node_hash
                        ) VALUES (
                            $1, $2, $3, $4, $5, $6,
                            $7, $8, $9, $10,
                            $11, $12, $13, $14, $15
                        )
                        "#,
                    )
                    .bind(&edge.edge_id)
                    .bind(&edge.run_id)
                    .bind(&edge.workflow_id)
                    .bind(&edge.task_id)
                    .bind(&edge.attempt_id)
                    .bind(edge.sequence_number)
                    .bind(&edge.source_node_id)
                    .bind(&edge.target_node_id)
                    .bind(&edge.edge_kind)
                    .bind(&edge.timestamp)
                    .bind(Json(&edge.tool_invocations))
                    .bind(Json(&edge.context_snapshot))
                    .bind(Json(&edge.metadata))
                    .bind(&edge.prev_hash)
                    .bind(&edge.node_hash)
                    .execute(pool)
                    .await
                    .map_err(|err| db_error("append workflow evidence edge", err))?;
                }
                None => {
                    return Err(AppError::DatabaseConnection(
                        "database pool not initialized".into(),
                    ))
                }
            }
        }

        Ok(())
    }

    pub async fn list_workflow_evidence(
        workflow_id: &str,
    ) -> Result<StoredWorkflowEvidence, AppError> {
        let run = Self::latest_run(workflow_id).await?.ok_or_else(|| {
            AppError::NotFound(format!("workflow evidence not found: {workflow_id}"))
        })?;
        let nodes = Self::list_nodes(&run.run_id).await?;
        let edges = Self::list_edges(&run.run_id).await?;

        Ok(StoredWorkflowEvidence { run, nodes, edges })
    }

    async fn next_run_sequence(workflow_id: &str) -> Result<i64, AppError> {
        match DATABASE_POOL.get() {
            Some(DatabasePool::Postgres(pool)) => sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COALESCE(MAX(sequence_number), -1) + 1
                FROM workflow_evidence_runs
                WHERE workflow_id = $1
                "#,
            )
            .bind(workflow_id)
            .fetch_one(pool)
            .await
            .map_err(|err| db_error("read workflow evidence run sequence", err)),
            Some(DatabasePool::Sqlite(pool)) => sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COALESCE(MAX(sequence_number), -1) + 1
                FROM workflow_evidence_runs
                WHERE workflow_id = $1
                "#,
            )
            .bind(workflow_id)
            .fetch_one(pool)
            .await
            .map_err(|err| db_error("read workflow evidence run sequence", err)),
            None => Err(AppError::DatabaseConnection(
                "database pool not initialized".into(),
            )),
        }
    }

    async fn latest_run(workflow_id: &str) -> Result<Option<WorkflowEvidenceRun>, AppError> {
        let row = match DATABASE_POOL.get() {
            Some(DatabasePool::Postgres(pool)) => sqlx::query_as::<_, WorkflowEvidenceRunRow>(
                r#"
                    SELECT
                        run_id,
                        workflow_id,
                        task_id,
                        attempt_id,
                        timestamp::text AS timestamp,
                        sequence_number,
                        prev_hash,
                        node_hash,
                        tool_invocations,
                        context_snapshot,
                        metadata
                    FROM workflow_evidence_runs
                    WHERE workflow_id = $1
                    ORDER BY sequence_number DESC
                    LIMIT 1
                    "#,
            )
            .bind(workflow_id)
            .fetch_optional(pool)
            .await
            .map_err(|err| db_error("read workflow evidence run", err))?,
            Some(DatabasePool::Sqlite(pool)) => sqlx::query_as::<_, WorkflowEvidenceRunRow>(
                r#"
                    SELECT
                        run_id,
                        workflow_id,
                        task_id,
                        attempt_id,
                        timestamp,
                        sequence_number,
                        prev_hash,
                        node_hash,
                        tool_invocations,
                        context_snapshot,
                        metadata
                    FROM workflow_evidence_runs
                    WHERE workflow_id = $1
                    ORDER BY sequence_number DESC
                    LIMIT 1
                    "#,
            )
            .bind(workflow_id)
            .fetch_optional(pool)
            .await
            .map_err(|err| db_error("read workflow evidence run", err))?,
            None => {
                return Err(AppError::DatabaseConnection(
                    "database pool not initialized".into(),
                ))
            }
        };

        Ok(row.map(Into::into))
    }

    async fn list_nodes(run_id: &str) -> Result<Vec<WorkflowEvidenceNode>, AppError> {
        let rows = match DATABASE_POOL.get() {
            Some(DatabasePool::Postgres(pool)) => sqlx::query_as::<_, WorkflowEvidenceNodeRow>(
                r#"
                    SELECT
                        node_id,
                        run_id,
                        workflow_id,
                        task_id,
                        attempt_id,
                        sequence_number,
                        node_kind,
                        timestamp::text AS timestamp,
                        llm_input_hash,
                        llm_output_hash,
                        tool_invocations,
                        context_snapshot,
                        metadata,
                        prev_hash,
                        node_hash
                    FROM workflow_evidence_nodes
                    WHERE run_id = $1
                    ORDER BY sequence_number ASC
                    "#,
            )
            .bind(run_id)
            .fetch_all(pool)
            .await
            .map_err(|err| db_error("list workflow evidence nodes", err))?,
            Some(DatabasePool::Sqlite(pool)) => sqlx::query_as::<_, WorkflowEvidenceNodeRow>(
                r#"
                    SELECT
                        node_id,
                        run_id,
                        workflow_id,
                        task_id,
                        attempt_id,
                        sequence_number,
                        node_kind,
                        timestamp,
                        llm_input_hash,
                        llm_output_hash,
                        tool_invocations,
                        context_snapshot,
                        metadata,
                        prev_hash,
                        node_hash
                    FROM workflow_evidence_nodes
                    WHERE run_id = $1
                    ORDER BY sequence_number ASC
                    "#,
            )
            .bind(run_id)
            .fetch_all(pool)
            .await
            .map_err(|err| db_error("list workflow evidence nodes", err))?,
            None => {
                return Err(AppError::DatabaseConnection(
                    "database pool not initialized".into(),
                ))
            }
        };

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_edges(run_id: &str) -> Result<Vec<WorkflowEvidenceEdge>, AppError> {
        let rows = match DATABASE_POOL.get() {
            Some(DatabasePool::Postgres(pool)) => sqlx::query_as::<_, WorkflowEvidenceEdgeRow>(
                r#"
                    SELECT
                        edge_id,
                        run_id,
                        workflow_id,
                        task_id,
                        attempt_id,
                        sequence_number,
                        source_node_id,
                        target_node_id,
                        edge_kind,
                        timestamp::text AS timestamp,
                        tool_invocations,
                        context_snapshot,
                        metadata,
                        prev_hash,
                        node_hash
                    FROM workflow_evidence_edges
                    WHERE run_id = $1
                    ORDER BY sequence_number ASC
                    "#,
            )
            .bind(run_id)
            .fetch_all(pool)
            .await
            .map_err(|err| db_error("list workflow evidence edges", err))?,
            Some(DatabasePool::Sqlite(pool)) => sqlx::query_as::<_, WorkflowEvidenceEdgeRow>(
                r#"
                    SELECT
                        edge_id,
                        run_id,
                        workflow_id,
                        task_id,
                        attempt_id,
                        sequence_number,
                        source_node_id,
                        target_node_id,
                        edge_kind,
                        timestamp,
                        tool_invocations,
                        context_snapshot,
                        metadata,
                        prev_hash,
                        node_hash
                    FROM workflow_evidence_edges
                    WHERE run_id = $1
                    ORDER BY sequence_number ASC
                    "#,
            )
            .bind(run_id)
            .fetch_all(pool)
            .await
            .map_err(|err| db_error("list workflow evidence edges", err))?,
            None => {
                return Err(AppError::DatabaseConnection(
                    "database pool not initialized".into(),
                ))
            }
        };

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

impl From<WorkflowEvidenceRunRow> for WorkflowEvidenceRun {
    fn from(row: WorkflowEvidenceRunRow) -> Self {
        Self {
            run_id: row.run_id,
            workflow_id: row.workflow_id,
            task_id: row.task_id,
            attempt_id: row.attempt_id,
            timestamp: row.timestamp,
            sequence_number: row.sequence_number,
            prev_hash: row.prev_hash,
            node_hash: row.node_hash,
            tool_invocations: row.tool_invocations.0,
            context_snapshot: row.context_snapshot.0,
            metadata: row.metadata.0,
        }
    }
}

impl From<WorkflowEvidenceNodeRow> for WorkflowEvidenceNode {
    fn from(row: WorkflowEvidenceNodeRow) -> Self {
        Self {
            node_id: row.node_id,
            run_id: row.run_id,
            workflow_id: row.workflow_id,
            task_id: row.task_id,
            attempt_id: row.attempt_id,
            sequence_number: row.sequence_number,
            node_kind: row.node_kind,
            timestamp: row.timestamp,
            llm_input_hash: row.llm_input_hash,
            llm_output_hash: row.llm_output_hash,
            tool_invocations: row.tool_invocations.0,
            context_snapshot: row.context_snapshot.0,
            metadata: row.metadata.0,
            prev_hash: row.prev_hash,
            node_hash: row.node_hash,
        }
    }
}

impl From<WorkflowEvidenceEdgeRow> for WorkflowEvidenceEdge {
    fn from(row: WorkflowEvidenceEdgeRow) -> Self {
        Self {
            edge_id: row.edge_id,
            run_id: row.run_id,
            workflow_id: row.workflow_id,
            task_id: row.task_id,
            attempt_id: row.attempt_id,
            sequence_number: row.sequence_number,
            source_node_id: row.source_node_id,
            target_node_id: row.target_node_id,
            edge_kind: row.edge_kind,
            timestamp: row.timestamp,
            tool_invocations: row.tool_invocations.0,
            context_snapshot: row.context_snapshot.0,
            metadata: row.metadata.0,
            prev_hash: row.prev_hash,
            node_hash: row.node_hash,
        }
    }
}

fn db_error(context: &str, err: sqlx::Error) -> AppError {
    error!(context, error = %err, "workflow evidence repository failure");
    AppError::DatabaseQuery(format!("{context}: {err}"))
}
