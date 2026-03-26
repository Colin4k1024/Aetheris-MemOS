use std::collections::BTreeMap;

use backend::models::{
    WorkflowEvidenceMap, WorkflowEvidenceNode, WorkflowEvidenceToolInvocation,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
struct HashableNode<'a> {
    attempt_id: &'a str,
    sequence_number: i64,
    timestamp: &'a str,
    llm_input_hash: &'a str,
    llm_output_hash: &'a str,
    tool_invocations: &'a [WorkflowEvidenceToolInvocation],
    context_snapshot: &'a WorkflowEvidenceMap,
    prev_hash: &'a Option<String>,
}

fn sample_tool_invocations() -> Vec<WorkflowEvidenceToolInvocation> {
    let mut inputs = BTreeMap::new();
    inputs.insert("attempt_id".to_string(), json!("attempt-1"));

    let mut outputs = BTreeMap::new();
    outputs.insert("status".to_string(), json!("ok"));

    vec![WorkflowEvidenceToolInvocation {
        tool_name: "evidence_writer".to_string(),
        invocation_id: Some("invocation-1".to_string()),
        inputs,
        outputs,
    }]
}

fn sample_context(label: &str) -> WorkflowEvidenceMap {
    let mut context_snapshot = BTreeMap::new();
    context_snapshot.insert("context_snapshot".to_string(), json!(label));
    context_snapshot.insert("window".to_string(), json!("scheduler"));
    context_snapshot
}

fn compute_hash(node: &WorkflowEvidenceNode) -> String {
    let payload = HashableNode {
        attempt_id: &node.attempt_id,
        sequence_number: node.sequence_number,
        timestamp: &node.timestamp,
        llm_input_hash: &node.llm_input_hash,
        llm_output_hash: &node.llm_output_hash,
        tool_invocations: &node.tool_invocations,
        context_snapshot: &node.context_snapshot,
        prev_hash: &node.prev_hash,
    };

    let bytes = serde_json::to_vec(&payload).expect("serialize hashable node");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn sample_node(
    sequence_number: i64,
    prev_hash: Option<String>,
    context_snapshot: WorkflowEvidenceMap,
) -> WorkflowEvidenceNode {
    let mut node = WorkflowEvidenceNode {
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
        tool_invocations: sample_tool_invocations(),
        context_snapshot,
        metadata: BTreeMap::new(),
        prev_hash,
        node_hash: String::new(),
    };
    node.node_hash = compute_hash(&node);
    node
}

fn verify_chain(nodes: &[WorkflowEvidenceNode]) -> Vec<String> {
    let mut violations = Vec::new();

    for (index, node) in nodes.iter().enumerate() {
        if index == 0 {
            if node.prev_hash.is_some() {
                violations.push(format!("node {} should not carry prev_hash", node.node_id));
            }
        } else {
            let previous = &nodes[index - 1];

            if node.sequence_number != previous.sequence_number + 1 {
                violations.push(format!(
                    "node {} has reordered sequence_number {} after {}",
                    node.node_id, node.sequence_number, previous.sequence_number
                ));
            }

            if node.prev_hash.as_deref() != Some(previous.node_hash.as_str()) {
                violations.push(format!("node {} has broken prev_hash link", node.node_id));
            }
        }

        let expected_hash = compute_hash(node);
        if node.node_hash != expected_hash {
            violations.push(format!(
                "node {} has hash mismatch after context_snapshot verification",
                node.node_id
            ));
        }
    }

    violations
}

#[test]
fn broken_prev_hash_link_is_rejected() {
    let first = sample_node(0, None, sample_context("initial"));
    let second = sample_node(1, Some("tampered-prev-hash".to_string()), sample_context("next"));

    let violations = verify_chain(&[first, second]);

    assert!(violations.iter().any(|entry| entry.contains("prev_hash")));
}

#[test]
fn reordered_sequence_numbers_are_rejected() {
    let first = sample_node(0, None, sample_context("initial"));
    let second = sample_node(2, Some(first.node_hash.clone()), sample_context("next"));

    let violations = verify_chain(&[first, second]);

    assert!(violations.iter().any(|entry| entry.contains("sequence_number")));
}

#[test]
fn mutated_context_snapshot_is_rejected() {
    let mut first = sample_node(0, None, sample_context("initial"));
    first
        .context_snapshot
        .insert("context_snapshot".to_string(), json!("mutated"));

    let violations = verify_chain(&[first]);

    assert!(violations
        .iter()
        .any(|entry| entry.contains("context_snapshot verification")));
}

#[test]
#[ignore = "Repository-backed hash-chain replay lands in plan 01-02."]
fn persisted_hash_chain_replay_is_reserved_for_next_plan() {
    let node = sample_node(0, None, sample_context("initial"));
    assert!(!node.node_hash.is_empty());
}
