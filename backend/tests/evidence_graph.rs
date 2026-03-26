use std::collections::BTreeMap;

use backend::models::{
    WorkflowEvidenceEdge, WorkflowEvidenceExport, WorkflowEvidenceNode, WorkflowEvidenceResponse,
    WorkflowEvidenceRun, WorkflowEvidenceToolInvocation, WorkflowEvidenceVerification,
};
use serde_json::json;

fn sample_tool_invocation() -> WorkflowEvidenceToolInvocation {
    let mut inputs = BTreeMap::new();
    inputs.insert("query".to_string(), json!("memory policy"));

    let mut outputs = BTreeMap::new();
    outputs.insert("status".to_string(), json!("ok"));

    WorkflowEvidenceToolInvocation {
        tool_name: "policy_lookup".to_string(),
        invocation_id: Some("tool-1".to_string()),
        inputs,
        outputs,
    }
}

fn sample_run() -> WorkflowEvidenceRun {
    let mut context_snapshot = BTreeMap::new();
    context_snapshot.insert("active_task".to_string(), json!("task-1"));
    context_snapshot.insert("memory_budget_mb".to_string(), json!(512));

    let mut metadata = BTreeMap::new();
    metadata.insert("environment".to_string(), json!("test"));

    WorkflowEvidenceRun {
        run_id: "run-1".to_string(),
        workflow_id: "workflow-1".to_string(),
        task_id: "task-1".to_string(),
        attempt_id: "attempt-1".to_string(),
        timestamp: "2026-03-26T00:00:00Z".to_string(),
        sequence_number: 0,
        prev_hash: None,
        node_hash: "run-hash-1".to_string(),
        tool_invocations: vec![sample_tool_invocation()],
        context_snapshot,
        metadata,
    }
}

fn sample_node(sequence_number: i64, prev_hash: Option<&str>, node_hash: &str) -> WorkflowEvidenceNode {
    let mut context_snapshot = BTreeMap::new();
    context_snapshot.insert("step".to_string(), json!(sequence_number));
    context_snapshot.insert("window".to_string(), json!("planner"));

    let mut metadata = BTreeMap::new();
    metadata.insert("confidence".to_string(), json!(0.9));

    WorkflowEvidenceNode {
        node_id: format!("node-{sequence_number}"),
        run_id: "run-1".to_string(),
        workflow_id: "workflow-1".to_string(),
        task_id: "task-1".to_string(),
        attempt_id: "attempt-1".to_string(),
        sequence_number,
        node_kind: "decision".to_string(),
        timestamp: format!("2026-03-26T00:00:0{sequence_number}Z"),
        llm_input_hash: format!("input-hash-{sequence_number}"),
        llm_output_hash: format!("output-hash-{sequence_number}"),
        tool_invocations: vec![sample_tool_invocation()],
        context_snapshot,
        metadata,
        prev_hash: prev_hash.map(str::to_string),
        node_hash: node_hash.to_string(),
    }
}

fn sample_edge(
    sequence_number: i64,
    source_node_id: &str,
    target_node_id: &str,
    prev_hash: Option<&str>,
    node_hash: &str,
) -> WorkflowEvidenceEdge {
    let mut context_snapshot = BTreeMap::new();
    context_snapshot.insert("transition".to_string(), json!("selected"));

    let mut metadata = BTreeMap::new();
    metadata.insert("reason".to_string(), json!("append-only"));

    WorkflowEvidenceEdge {
        edge_id: format!("edge-{sequence_number}"),
        run_id: "run-1".to_string(),
        workflow_id: "workflow-1".to_string(),
        task_id: "task-1".to_string(),
        attempt_id: "attempt-1".to_string(),
        sequence_number,
        source_node_id: source_node_id.to_string(),
        target_node_id: target_node_id.to_string(),
        edge_kind: "follows".to_string(),
        timestamp: format!("2026-03-26T00:01:0{sequence_number}Z"),
        tool_invocations: vec![sample_tool_invocation()],
        context_snapshot,
        metadata,
        prev_hash: prev_hash.map(str::to_string),
        node_hash: node_hash.to_string(),
    }
}

#[test]
fn workflow_evidence_response_round_trips_through_json() {
    let run = sample_run();
    let nodes = vec![
        sample_node(0, None, "node-hash-0"),
        sample_node(1, Some("node-hash-0"), "node-hash-1"),
    ];
    let edges = vec![
        sample_edge(0, "node-0", "node-1", None, "edge-hash-0"),
        sample_edge(1, "node-1", "node-2", Some("edge-hash-0"), "edge-hash-1"),
    ];
    let verification = WorkflowEvidenceVerification {
        workflow_id: "workflow-1".to_string(),
        run_id: "run-1".to_string(),
        checked_at: "2026-03-26T00:02:00Z".to_string(),
        verified: true,
        expected_node_count: nodes.len(),
        verified_node_count: nodes.len(),
        root_hash: Some("node-hash-1".to_string()),
        violations: Vec::new(),
        metadata: BTreeMap::new(),
    };
    let response = WorkflowEvidenceResponse {
        run,
        nodes,
        edges,
        verification,
    };
    let export = WorkflowEvidenceExport {
        schema_version: "2026-03-26".to_string(),
        hash_algorithm: "sha256".to_string(),
        exported_at: "2026-03-26T00:03:00Z".to_string(),
        response,
        metadata: BTreeMap::new(),
    };

    let value = serde_json::to_value(&export).expect("serialize export");
    assert_eq!(value["response"]["run"]["attempt_id"], "attempt-1");
    assert!(value["response"]["nodes"][0].get("context_snapshot").is_some());
    assert!(value["response"]["edges"][0].get("node_hash").is_some());

    let round_trip: WorkflowEvidenceExport =
        serde_json::from_value(value).expect("deserialize export");
    assert_eq!(round_trip.response.nodes[0].llm_input_hash, "input-hash-0");
    assert_eq!(
        round_trip.response.edges[1].prev_hash.as_deref(),
        Some("edge-hash-0")
    );
}

#[test]
fn workflow_evidence_sequences_are_append_only_and_monotonic() {
    let nodes = vec![
        sample_node(0, None, "node-hash-0"),
        sample_node(1, Some("node-hash-0"), "node-hash-1"),
        sample_node(2, Some("node-hash-1"), "node-hash-2"),
    ];
    let edges = vec![
        sample_edge(0, "node-0", "node-1", None, "edge-hash-0"),
        sample_edge(1, "node-1", "node-2", Some("edge-hash-0"), "edge-hash-1"),
    ];

    assert!(nodes.windows(2).all(|pair| {
        pair[1].sequence_number == pair[0].sequence_number + 1
            && pair[1].attempt_id == pair[0].attempt_id
    }));
    assert!(edges.windows(2).all(|pair| {
        pair[1].sequence_number == pair[0].sequence_number + 1
            && pair[1].prev_hash.as_deref() == Some(pair[0].node_hash.as_str())
    }));
    assert!(nodes
        .iter()
        .all(|node| node.context_snapshot.contains_key("step") && !node.attempt_id.is_empty()));
}

#[test]
#[ignore = "Database-backed append-only repository coverage lands in plan 01-02."]
fn repository_append_only_behavior_is_reserved_for_next_plan() {
    let run = sample_run();
    assert_eq!(run.sequence_number, 0);
}
