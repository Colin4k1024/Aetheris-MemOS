use salvo::oapi::ToSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    Conversation,
    Task,
    Query,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Modality {
    Text,
    Image,
    Audio,
    Video,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TemporalScope {
    Short,
    Medium,
    Long,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningDepth {
    Shallow,
    Medium,
    Deep,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskContext {
    #[serde(rename = "task_id")]
    pub task_id: String,
    #[serde(rename = "task_type")]
    pub task_type: TaskType,
    pub complexity: f64,
    #[serde(rename = "modality_requirements")]
    pub modality_requirements: Vec<Modality>,
    #[serde(rename = "temporal_scope")]
    pub temporal_scope: TemporalScope,
    #[serde(rename = "reasoning_depth")]
    pub reasoning_depth: ReasoningDepth,
    #[serde(rename = "context_dependency")]
    pub context_dependency: f64,
    #[serde(rename = "user_id")]
    pub user_id: String,
    #[serde(rename = "agent_id")]
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResourceConstraints {
    #[serde(rename = "max_memory_usage_mb")]
    pub max_memory_usage_mb: u64,
    #[serde(rename = "max_cpu_usage_percent")]
    pub max_cpu_usage_percent: u8,
    #[serde(rename = "max_response_time_ms")]
    pub max_response_time_ms: u64,
    #[serde(rename = "storage_quota_percent")]
    pub storage_quota_percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskPreferences {
    #[serde(rename = "prioritize_efficiency")]
    pub prioritize_efficiency: bool,
    #[serde(rename = "prioritize_coherence")]
    pub prioritize_coherence: bool,
    #[serde(rename = "enable_multimodal")]
    pub enable_multimodal: bool,
    #[serde(rename = "enable_reasoning")]
    pub enable_reasoning: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskContextInput {
    pub content: String,
    pub modality: Vec<String>,
    #[serde(rename = "context_history")]
    pub context_history: Vec<ContextHistoryItem>,
    #[serde(rename = "task_metadata")]
    pub task_metadata: Option<TaskMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContextHistoryItem {
    pub role: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskMetadata {
    pub domain: Option<String>,
    #[serde(rename = "complexity_hint")]
    pub complexity_hint: Option<String>,
    #[serde(rename = "expected_duration")]
    pub expected_duration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskCharacteristics {
    pub complexity: f64,
    #[serde(rename = "modality_count")]
    pub modality_count: usize,
    #[serde(rename = "temporal_scope")]
    pub temporal_scope: String,
    #[serde(rename = "reasoning_depth")]
    pub reasoning_depth: f64,
    #[serde(rename = "context_dependency")]
    pub context_dependency: f64,
}

