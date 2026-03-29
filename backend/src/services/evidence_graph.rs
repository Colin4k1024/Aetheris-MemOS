use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use ulid::Ulid;

use crate::db::evidence_graph::{EvidenceGraphRepository, StoredWorkflowEvidence};
use crate::models::{
    WorkflowEvidenceEdge, WorkflowEvidenceExport, WorkflowEvidenceMap, WorkflowEvidenceNode,
    WorkflowEvidenceResponse, WorkflowEvidenceRun, WorkflowEvidenceToolInvocation,
    WorkflowEvidenceVerification,
};
use crate::services::scheduler::DecisionTrace;
use crate::AppError;

pub const WORKFLOW_EVIDENCE_EXPORT_SCHEMA_VERSION: &str = "workflow-evidence-export.v1";
pub const WORKFLOW_EVIDENCE_HASH_ALGORITHM: &str = "sha256";

pub fn build_evidence_nodes(trace: &DecisionTrace) -> Result<Vec<WorkflowEvidenceNode>, AppError> {
    let run_id = Ulid::new().to_string();
    let attempt_id = Ulid::new().to_string();
    build_evidence_nodes_for_run(trace, &run_id, &trace.task_id, &attempt_id, Utc::now())
}

pub async fn record_decision_trace_as_evidence(
    trace: &DecisionTrace,
) -> Result<WorkflowEvidenceResponse, AppError> {
    let workflow_id = trace.task_id.clone();
    let run_id = Ulid::new().to_string();
    let attempt_id = Ulid::new().to_string();
    let started_at = Utc::now();
    let nodes =
        build_evidence_nodes_for_run(trace, &run_id, &workflow_id, &attempt_id, started_at)?;
    let edges = build_evidence_edges_for_run(&nodes, &attempt_id, started_at)?;
    let run = build_run(
        trace,
        &run_id,
        &workflow_id,
        &attempt_id,
        &nodes,
        &edges,
        started_at,
    )?;
    let run = EvidenceGraphRepository::create_run(run).await?;
    EvidenceGraphRepository::append_nodes(&nodes).await?;
    EvidenceGraphRepository::append_edges(&edges).await?;
    let verification = verify_chain(&workflow_id, &run.run_id, &nodes)?;

    Ok(WorkflowEvidenceResponse {
        run,
        nodes,
        edges,
        verification,
    })
}

pub async fn list_workflow_evidence(
    workflow_id: &str,
) -> Result<WorkflowEvidenceResponse, AppError> {
    let StoredWorkflowEvidence { run, nodes, edges } =
        EvidenceGraphRepository::list_workflow_evidence(workflow_id).await?;
    let verification = verify_chain(&run.workflow_id, &run.run_id, &nodes)?;

    Ok(WorkflowEvidenceResponse {
        run,
        nodes,
        edges,
        verification,
    })
}

pub fn verify_chain(
    workflow_id: &str,
    run_id: &str,
    nodes: &[WorkflowEvidenceNode],
) -> Result<WorkflowEvidenceVerification, AppError> {
    let checked_at = Utc::now().to_rfc3339();
    let mut violations = Vec::new();
    let mut last_hash: Option<String> = None;

    for (index, node) in nodes.iter().enumerate() {
        violations.extend(locked_field_violations(node));

        if node.sequence_number != index as i64 {
            violations.push(format!(
                "node {} has non-monotonic sequence_number {}",
                node.node_id, node.sequence_number
            ));
        }

        if node.prev_hash != last_hash {
            violations.push(format!(
                "node {} has broken prev_hash linkage",
                node.node_id
            ));
        }

        let expected_hash = compute_node_hash(node)?;
        if node.node_hash != expected_hash {
            violations.push(format!(
                "node {} canonical payload does not match stored node_hash",
                node.node_id
            ));
        }

        last_hash = Some(node.node_hash.clone());
    }

    Ok(WorkflowEvidenceVerification {
        workflow_id: workflow_id.to_string(),
        run_id: run_id.to_string(),
        checked_at,
        verified: violations.is_empty(),
        expected_node_count: nodes.len(),
        verified_node_count: nodes.len().saturating_sub(violations.len()),
        root_hash: nodes.last().map(|node| node.node_hash.clone()),
        violations,
        metadata: BTreeMap::from([
            (
                "hash_algorithm".to_string(),
                json!(WORKFLOW_EVIDENCE_HASH_ALGORITHM),
            ),
            ("verification_mode".to_string(), json!("canonical-replay")),
        ]),
    })
}

pub fn build_workflow_evidence_export(
    response: &WorkflowEvidenceResponse,
    exported_at: chrono::DateTime<Utc>,
) -> WorkflowEvidenceExport {
    WorkflowEvidenceExport {
        schema_version: WORKFLOW_EVIDENCE_EXPORT_SCHEMA_VERSION.to_string(),
        hash_algorithm: WORKFLOW_EVIDENCE_HASH_ALGORITHM.to_string(),
        workflow_id: response.run.workflow_id.clone(),
        attempt_id: response.run.attempt_id.clone(),
        root_hash: response
            .verification
            .root_hash
            .clone()
            .unwrap_or_else(|| response.run.node_hash.clone()),
        chain_verified: response.verification.verified,
        nodes: response.nodes.clone(),
        edges: response.edges.clone(),
        exported_at: exported_at.to_rfc3339(),
    }
}

pub fn canonical_export_body_bytes(export: &WorkflowEvidenceExport) -> Result<Vec<u8>, AppError> {
    let body = CanonicalWorkflowEvidenceExport {
        schema_version: &export.schema_version,
        hash_algorithm: &export.hash_algorithm,
        workflow_id: &export.workflow_id,
        attempt_id: &export.attempt_id,
        root_hash: &export.root_hash,
        chain_verified: export.chain_verified,
        nodes: &export.nodes,
        edges: &export.edges,
    };
    serde_json::to_vec(&body)
        .map_err(|err| AppError::Serialization(format!("serialize canonical export body: {err}")))
}

pub fn hash_workflow_evidence_export_body(
    export: &WorkflowEvidenceExport,
) -> Result<String, AppError> {
    let bytes = canonical_export_body_bytes(export)?;
    Ok(sha256_hex(&bytes))
}

fn build_evidence_nodes_for_run(
    trace: &DecisionTrace,
    run_id: &str,
    workflow_id: &str,
    attempt_id: &str,
    started_at: chrono::DateTime<Utc>,
) -> Result<Vec<WorkflowEvidenceNode>, AppError> {
    let stages = vec![
        stage_spec(
            "analyzer",
            json!({
                "task_id": trace.task_id,
                "initial_memory_config": trace.initial_memory_config,
            }),
            serde_json::to_value(&trace.analyzer).map_err(|err| {
                AppError::Serialization(format!("serialize analyzer stage: {err}"))
            })?,
        ),
        stage_spec(
            "predictor",
            serde_json::to_value(&trace.analyzer).map_err(|err| {
                AppError::Serialization(format!("serialize analyzer stage input: {err}"))
            })?,
            serde_json::to_value(&trace.predictor).map_err(|err| {
                AppError::Serialization(format!("serialize predictor stage: {err}"))
            })?,
        ),
        stage_spec(
            "weight_adjustment",
            json!({
                "cost_benefit_ratio": trace.cost_benefit_ratio,
                "predictor": trace.predictor,
            }),
            serde_json::to_value(&trace.weight_adjustment).map_err(|err| {
                AppError::Serialization(format!("serialize weight adjustment stage: {err}"))
            })?,
        ),
        stage_spec(
            "final_result",
            json!({
                "weight_adjustment": trace.weight_adjustment,
                "memory_contributions": trace.memory_contributions,
            }),
            serde_json::to_value(&trace.final_result).map_err(|err| {
                AppError::Serialization(format!("serialize final result stage: {err}"))
            })?,
        ),
    ];

    let mut nodes = Vec::with_capacity(stages.len());
    let mut previous_hash = None;

    for (index, stage) in stages.into_iter().enumerate() {
        let timestamp = (started_at + Duration::seconds(index as i64)).to_rfc3339();
        let tool_invocation = build_tool_invocation(
            &format!("scheduler_{}", stage.node_kind),
            attempt_id,
            index as i64,
            &stage.input,
            &stage.output,
        )?;
        let mut metadata = BTreeMap::new();
        metadata.insert("trace_kind".to_string(), json!("scheduler_decision"));
        metadata.insert("locked_fields_version".to_string(), json!("2026-03-26"));
        metadata.insert("stage".to_string(), json!(stage.node_kind));

        let mut context_snapshot = BTreeMap::new();
        context_snapshot.insert("task_id".to_string(), json!(trace.task_id));
        context_snapshot.insert("stage".to_string(), json!(stage.node_kind));
        context_snapshot.insert("input".to_string(), stage.input.clone());
        context_snapshot.insert("output".to_string(), stage.output.clone());

        let mut node = WorkflowEvidenceNode {
            node_id: Ulid::new().to_string(),
            run_id: run_id.to_string(),
            workflow_id: workflow_id.to_string(),
            task_id: trace.task_id.clone(),
            attempt_id: attempt_id.to_string(),
            sequence_number: index as i64,
            node_kind: stage.node_kind.to_string(),
            timestamp,
            llm_input_hash: hash_json(&stage.input)?,
            llm_output_hash: hash_json(&stage.output)?,
            tool_invocations: vec![tool_invocation],
            context_snapshot,
            metadata,
            prev_hash: previous_hash.clone(),
            node_hash: String::new(),
        };
        ensure_locked_fields(&node)?;
        node.node_hash = compute_node_hash(&node)?;
        previous_hash = Some(node.node_hash.clone());
        nodes.push(node);
    }

    Ok(nodes)
}

fn build_evidence_edges_for_run(
    nodes: &[WorkflowEvidenceNode],
    attempt_id: &str,
    started_at: chrono::DateTime<Utc>,
) -> Result<Vec<WorkflowEvidenceEdge>, AppError> {
    let mut edges = Vec::new();
    let mut previous_hash = None;

    for (index, pair) in nodes.windows(2).enumerate() {
        let source = &pair[0];
        let target = &pair[1];
        let timestamp = (started_at + Duration::seconds(index as i64)).to_rfc3339();
        let transition = json!({
            "source_node_id": source.node_id,
            "target_node_id": target.node_id,
            "source_kind": source.node_kind,
            "target_kind": target.node_kind,
        });
        let tool_invocation = build_tool_invocation(
            "scheduler_transition",
            attempt_id,
            index as i64,
            &transition,
            &json!({"edge_kind": "follows"}),
        )?;
        let mut context_snapshot = BTreeMap::new();
        context_snapshot.insert("transition".to_string(), transition);

        let mut metadata = BTreeMap::new();
        metadata.insert("source_kind".to_string(), json!(source.node_kind));
        metadata.insert("target_kind".to_string(), json!(target.node_kind));

        let mut edge = WorkflowEvidenceEdge {
            edge_id: Ulid::new().to_string(),
            run_id: source.run_id.clone(),
            workflow_id: source.workflow_id.clone(),
            task_id: source.task_id.clone(),
            attempt_id: attempt_id.to_string(),
            sequence_number: index as i64,
            source_node_id: source.node_id.clone(),
            target_node_id: target.node_id.clone(),
            edge_kind: "follows".to_string(),
            timestamp,
            tool_invocations: vec![tool_invocation],
            context_snapshot,
            metadata,
            prev_hash: previous_hash.clone(),
            node_hash: String::new(),
        };
        edge.node_hash = compute_edge_hash(&edge)?;
        previous_hash = Some(edge.node_hash.clone());
        edges.push(edge);
    }

    Ok(edges)
}

fn build_run(
    trace: &DecisionTrace,
    run_id: &str,
    workflow_id: &str,
    attempt_id: &str,
    nodes: &[WorkflowEvidenceNode],
    edges: &[WorkflowEvidenceEdge],
    started_at: chrono::DateTime<Utc>,
) -> Result<WorkflowEvidenceRun, AppError> {
    let mut context_snapshot = BTreeMap::new();
    context_snapshot.insert(
        "resource_status".to_string(),
        canonicalize_json(serde_json::to_value(&trace.resource_status).map_err(|err| {
            AppError::Serialization(format!("serialize workflow resource status: {err}"))
        })?),
    );
    context_snapshot.insert(
        "final_result".to_string(),
        canonicalize_json(serde_json::to_value(&trace.final_result).map_err(|err| {
            AppError::Serialization(format!("serialize workflow final result: {err}"))
        })?),
    );
    context_snapshot.insert(
        "memory_contributions".to_string(),
        canonicalize_json(
            serde_json::to_value(&trace.memory_contributions).map_err(|err| {
                AppError::Serialization(format!("serialize memory contributions: {err}"))
            })?,
        ),
    );

    let mut metadata = BTreeMap::new();
    metadata.insert("node_count".to_string(), json!(nodes.len()));
    metadata.insert("edge_count".to_string(), json!(edges.len()));
    metadata.insert("verification_status".to_string(), json!("pending"));

    Ok(WorkflowEvidenceRun {
        run_id: run_id.to_string(),
        workflow_id: workflow_id.to_string(),
        task_id: trace.task_id.clone(),
        attempt_id: attempt_id.to_string(),
        timestamp: started_at.to_rfc3339(),
        sequence_number: 0,
        prev_hash: None,
        node_hash: nodes
            .last()
            .map(|node| node.node_hash.clone())
            .unwrap_or_else(|| sha256_hex(run_id.as_bytes())),
        tool_invocations: vec![build_tool_invocation(
            "scheduler_pipeline",
            attempt_id,
            0,
            &json!({"task_id": trace.task_id, "workflow_id": workflow_id}),
            &json!({"node_count": nodes.len(), "edge_count": edges.len()}),
        )?],
        context_snapshot,
        metadata,
    })
}

fn build_tool_invocation(
    tool_name: &str,
    attempt_id: &str,
    sequence_number: i64,
    input: &Value,
    output: &Value,
) -> Result<WorkflowEvidenceToolInvocation, AppError> {
    Ok(WorkflowEvidenceToolInvocation {
        tool_name: tool_name.to_string(),
        invocation_id: Some(format!("{attempt_id}-{sequence_number}")),
        inputs: value_to_map(input.clone())?,
        outputs: value_to_map(output.clone())?,
    })
}

fn stage_spec(node_kind: &'static str, input: Value, output: Value) -> StageSpec {
    StageSpec {
        node_kind,
        input: canonicalize_json(input),
        output: canonicalize_json(output),
    }
}

fn ensure_locked_fields(node: &WorkflowEvidenceNode) -> Result<(), AppError> {
    let violations = locked_field_violations(node);
    if violations.is_empty() {
        Ok(())
    } else {
        Err(AppError::BadRequest(violations.join("; ")))
    }
}

fn locked_field_violations(node: &WorkflowEvidenceNode) -> Vec<String> {
    let mut violations = Vec::new();
    if node.timestamp.is_empty() {
        violations.push(format!("node {} missing timestamp", node.node_id));
    }
    if node.attempt_id.is_empty() {
        violations.push(format!("node {} missing attempt_id", node.node_id));
    }
    if node.llm_input_hash.is_empty() {
        violations.push(format!("node {} missing llm_input_hash", node.node_id));
    }
    if node.llm_output_hash.is_empty() {
        violations.push(format!("node {} missing llm_output_hash", node.node_id));
    }
    if node.tool_invocations.is_empty() {
        violations.push(format!("node {} missing tool_invocations", node.node_id));
    }
    if node.context_snapshot.is_empty() {
        violations.push(format!("node {} missing context_snapshot", node.node_id));
    }
    violations
}

fn hash_json(value: &Value) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(&canonicalize_json(value.clone()))
        .map_err(|err| AppError::Serialization(format!("serialize canonical json: {err}")))?;
    Ok(sha256_hex(&bytes))
}

fn compute_node_hash(node: &WorkflowEvidenceNode) -> Result<String, AppError> {
    let payload = CanonicalNodeHash {
        run_id: &node.run_id,
        workflow_id: &node.workflow_id,
        task_id: &node.task_id,
        attempt_id: &node.attempt_id,
        sequence_number: node.sequence_number,
        node_kind: &node.node_kind,
        timestamp: &node.timestamp,
        llm_input_hash: &node.llm_input_hash,
        llm_output_hash: &node.llm_output_hash,
        tool_invocations: &node.tool_invocations,
        context_snapshot: &node.context_snapshot,
        metadata: &node.metadata,
        prev_hash: &node.prev_hash,
    };
    let bytes = serde_json::to_vec(&payload).map_err(|err| {
        AppError::Serialization(format!("serialize canonical node payload: {err}"))
    })?;
    Ok(sha256_hex(&bytes))
}

fn compute_edge_hash(edge: &WorkflowEvidenceEdge) -> Result<String, AppError> {
    let payload = CanonicalEdgeHash {
        run_id: &edge.run_id,
        workflow_id: &edge.workflow_id,
        task_id: &edge.task_id,
        attempt_id: &edge.attempt_id,
        sequence_number: edge.sequence_number,
        source_node_id: &edge.source_node_id,
        target_node_id: &edge.target_node_id,
        edge_kind: &edge.edge_kind,
        timestamp: &edge.timestamp,
        tool_invocations: &edge.tool_invocations,
        context_snapshot: &edge.context_snapshot,
        metadata: &edge.metadata,
        prev_hash: &edge.prev_hash,
    };
    let bytes = serde_json::to_vec(&payload).map_err(|err| {
        AppError::Serialization(format!("serialize canonical edge payload: {err}"))
    })?;
    Ok(sha256_hex(&bytes))
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        Value::Object(map) => {
            let ordered = map
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect::<BTreeMap<_, _>>();
            let mut json_map = serde_json::Map::new();
            for (key, value) in ordered {
                json_map.insert(key, value);
            }
            Value::Object(json_map)
        }
        other => other,
    }
}

fn value_to_map(value: Value) -> Result<WorkflowEvidenceMap, AppError> {
    match canonicalize_json(value) {
        Value::Object(map) => Ok(map.into_iter().collect()),
        other => Ok(BTreeMap::from([("value".to_string(), other)])),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

struct StageSpec {
    node_kind: &'static str,
    input: Value,
    output: Value,
}

#[derive(Serialize)]
struct CanonicalNodeHash<'a> {
    run_id: &'a str,
    workflow_id: &'a str,
    task_id: &'a str,
    attempt_id: &'a str,
    sequence_number: i64,
    node_kind: &'a str,
    timestamp: &'a str,
    llm_input_hash: &'a str,
    llm_output_hash: &'a str,
    tool_invocations: &'a [WorkflowEvidenceToolInvocation],
    context_snapshot: &'a WorkflowEvidenceMap,
    metadata: &'a WorkflowEvidenceMap,
    prev_hash: &'a Option<String>,
}

#[derive(Serialize)]
struct CanonicalEdgeHash<'a> {
    run_id: &'a str,
    workflow_id: &'a str,
    task_id: &'a str,
    attempt_id: &'a str,
    sequence_number: i64,
    source_node_id: &'a str,
    target_node_id: &'a str,
    edge_kind: &'a str,
    timestamp: &'a str,
    tool_invocations: &'a [WorkflowEvidenceToolInvocation],
    context_snapshot: &'a WorkflowEvidenceMap,
    metadata: &'a WorkflowEvidenceMap,
    prev_hash: &'a Option<String>,
}

#[derive(Serialize)]
struct CanonicalWorkflowEvidenceExport<'a> {
    schema_version: &'a str,
    hash_algorithm: &'a str,
    workflow_id: &'a str,
    attempt_id: &'a str,
    root_hash: &'a str,
    chain_verified: bool,
    nodes: &'a [WorkflowEvidenceNode],
    edges: &'a [WorkflowEvidenceEdge],
}
