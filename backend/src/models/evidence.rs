use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type WorkflowEvidenceMap = BTreeMap<String, Value>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowEvidenceToolInvocation {
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation_id: Option<String>,
    #[serde(default)]
    pub inputs: WorkflowEvidenceMap,
    #[serde(default)]
    pub outputs: WorkflowEvidenceMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowEvidenceRun {
    pub run_id: String,
    pub workflow_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub timestamp: String,
    pub sequence_number: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
    pub node_hash: String,
    #[serde(default)]
    pub tool_invocations: Vec<WorkflowEvidenceToolInvocation>,
    #[serde(default)]
    pub context_snapshot: WorkflowEvidenceMap,
    #[serde(default)]
    pub metadata: WorkflowEvidenceMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowEvidenceNode {
    pub node_id: String,
    pub run_id: String,
    pub workflow_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub sequence_number: i64,
    pub node_kind: String,
    pub timestamp: String,
    pub llm_input_hash: String,
    pub llm_output_hash: String,
    #[serde(default)]
    pub tool_invocations: Vec<WorkflowEvidenceToolInvocation>,
    #[serde(default)]
    pub context_snapshot: WorkflowEvidenceMap,
    #[serde(default)]
    pub metadata: WorkflowEvidenceMap,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
    pub node_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowEvidenceEdge {
    pub edge_id: String,
    pub run_id: String,
    pub workflow_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub sequence_number: i64,
    pub source_node_id: String,
    pub target_node_id: String,
    pub edge_kind: String,
    pub timestamp: String,
    #[serde(default)]
    pub tool_invocations: Vec<WorkflowEvidenceToolInvocation>,
    #[serde(default)]
    pub context_snapshot: WorkflowEvidenceMap,
    #[serde(default)]
    pub metadata: WorkflowEvidenceMap,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
    pub node_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowEvidenceVerification {
    pub workflow_id: String,
    pub run_id: String,
    pub checked_at: String,
    pub verified: bool,
    pub expected_node_count: usize,
    pub verified_node_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_hash: Option<String>,
    #[serde(default)]
    pub violations: Vec<String>,
    #[serde(default)]
    pub metadata: WorkflowEvidenceMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowEvidenceResponse {
    pub run: WorkflowEvidenceRun,
    #[serde(default)]
    pub nodes: Vec<WorkflowEvidenceNode>,
    #[serde(default)]
    pub edges: Vec<WorkflowEvidenceEdge>,
    pub verification: WorkflowEvidenceVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowEvidenceExport {
    pub schema_version: String,
    pub hash_algorithm: String,
    pub workflow_id: String,
    pub attempt_id: String,
    pub root_hash: String,
    pub chain_verified: bool,
    #[serde(default)]
    pub nodes: Vec<WorkflowEvidenceNode>,
    #[serde(default)]
    pub edges: Vec<WorkflowEvidenceEdge>,
    pub exported_at: String,
}
