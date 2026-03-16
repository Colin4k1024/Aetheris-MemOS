use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ============================================================================
// Agent Identity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentIdentity {
    pub agent_id: String,
    pub agent_name: String,
    pub agent_type: String,
    pub version: String,
    pub capabilities: serde_json::Value,
    pub description: Option<String>,
    pub personality_traits: Option<serde_json::Value>,
    pub system_prompt: Option<String>,
    pub core_directives: Option<serde_json::Value>,
    pub memory_config: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    pub status: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateAgentIdentity {
    pub agent_id: Option<String>,
    pub agent_name: String,
    pub agent_type: Option<String>,
    pub version: Option<String>,
    pub capabilities: Option<serde_json::Value>,
    pub description: Option<String>,
    pub personality_traits: Option<serde_json::Value>,
    pub system_prompt: Option<String>,
    pub core_directives: Option<serde_json::Value>,
    pub memory_config: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateAgentIdentity {
    pub agent_name: Option<String>,
    pub capabilities: Option<serde_json::Value>,
    pub description: Option<String>,
    pub personality_traits: Option<serde_json::Value>,
    pub system_prompt: Option<String>,
    pub core_directives: Option<serde_json::Value>,
    pub memory_config: Option<serde_json::Value>,
    pub status: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

// ============================================================================
// Agent Capability
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentCapability {
    pub capability_id: String,
    pub agent_id: String,
    pub capability_name: String,
    pub capability_type: String,
    pub description: Option<String>,
    pub implementation_type: Option<String>,
    pub implementation_ref: Option<String>,
    pub success_rate: f64,
    pub avg_latency_ms: Option<i32>,
    pub times_invoked: i32,
    pub max_tokens: Option<i32>,
    pub timeout_ms: Option<i32>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateAgentCapability {
    pub capability_id: Option<String>,
    pub capability_name: String,
    pub capability_type: String,
    pub description: Option<String>,
    pub implementation_type: Option<String>,
    pub implementation_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateAgentCapability {
    pub capability_name: Option<String>,
    pub description: Option<String>,
    pub implementation_type: Option<String>,
    pub implementation_ref: Option<String>,
    pub success_rate: Option<f64>,
    pub avg_latency_ms: Option<i32>,
    pub times_invoked: Option<i32>,
    pub max_tokens: Option<i32>,
    pub timeout_ms: Option<i32>,
    pub enabled: Option<bool>,
}

// ============================================================================
// Agent Behavior Profile
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentBehaviorProfile {
    pub profile_id: String,
    pub agent_id: String,
    pub behavior_type: String,
    pub pattern_description: String,
    pub pattern_embedding: Option<Vec<f32>>,
    pub times_applied: i32,
    pub success_rate: f64,
    pub avg_outcome_score: f64,
    pub effective_contexts: serde_json::Value,
    pub confidence: f64,
    pub status: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateAgentBehaviorProfile {
    pub profile_id: Option<String>,
    pub behavior_type: String,
    pub pattern_description: String,
    pub pattern_embedding: Option<Vec<f32>>,
    pub effective_contexts: Option<serde_json::Value>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateAgentBehaviorProfile {
    pub pattern_description: Option<String>,
    pub pattern_embedding: Option<Vec<f32>>,
    pub times_applied: Option<i32>,
    pub success_rate: Option<f64>,
    pub avg_outcome_score: Option<f64>,
    pub effective_contexts: Option<serde_json::Value>,
    pub confidence: Option<f64>,
    pub status: Option<String>,
}

// ============================================================================
// Agent Episode (for self-reflection)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentEpisode {
    pub episode_id: String,
    pub agent_id: String,
    pub episode_type: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub situation: String,
    pub actions_taken: serde_json::Value,
    pub outcome: String,
    pub outcome_score: Option<f64>,
    pub success: Option<bool>,
    pub what_went_well: Option<String>,
    pub what_could_improve: Option<String>,
    pub lessons_learned: Option<String>,
    pub related_episode_ids: Vec<String>,
    pub relevant_knowledge_ids: Vec<String>,
    pub reflection_level: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateAgentEpisode {
    pub episode_id: Option<String>,
    pub episode_type: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub situation: String,
    pub actions_taken: Option<serde_json::Value>,
    pub outcome: String,
    pub outcome_score: Option<f64>,
    pub success: Option<bool>,
    pub what_went_well: Option<String>,
    pub what_could_improve: Option<String>,
    pub lessons_learned: Option<String>,
    pub related_episode_ids: Option<Vec<String>>,
    pub relevant_knowledge_ids: Option<Vec<String>>,
    pub reflection_level: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateAgentEpisode {
    pub end_time: Option<String>,
    pub outcome: Option<String>,
    pub outcome_score: Option<f64>,
    pub success: Option<bool>,
    pub what_went_well: Option<String>,
    pub what_could_improve: Option<String>,
    pub lessons_learned: Option<String>,
    pub related_episode_ids: Option<Vec<String>>,
    pub relevant_knowledge_ids: Option<Vec<String>>,
    pub reflection_level: Option<i32>,
}

// ============================================================================
// Agent Self-Model (Meta-Memory)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentSelfModel {
    pub model_id: String,
    pub agent_id: String,
    pub identity_beliefs: serde_json::Value,
    pub strengths: serde_json::Value,
    pub weaknesses: serde_json::Value,
    pub learned_skills: serde_json::Value,
    pub preferences: serde_json::Value,
    pub relationships: serde_json::Value,
    pub computed_traits: serde_json::Value,
    pub confidence_score: f64,
    pub consistency_score: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateAgentSelfModel {
    pub model_id: Option<String>,
    pub identity_beliefs: Option<serde_json::Value>,
    pub strengths: Option<serde_json::Value>,
    pub weaknesses: Option<serde_json::Value>,
    pub learned_skills: Option<serde_json::Value>,
    pub preferences: Option<serde_json::Value>,
    pub relationships: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateAgentSelfModel {
    pub identity_beliefs: Option<serde_json::Value>,
    pub strengths: Option<serde_json::Value>,
    pub weaknesses: Option<serde_json::Value>,
    pub learned_skills: Option<serde_json::Value>,
    pub preferences: Option<serde_json::Value>,
    pub relationships: Option<serde_json::Value>,
    pub computed_traits: Option<serde_json::Value>,
    pub confidence_score: Option<f64>,
    pub consistency_score: Option<f64>,
}

// ============================================================================
// Combined Responses
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentWithSelfModel {
    pub identity: AgentIdentity,
    pub self_model: Option<AgentSelfModel>,
    pub capabilities: Vec<AgentCapability>,
    pub behavior_profiles: Vec<AgentBehaviorProfile>,
    pub recent_episodes: Vec<AgentEpisode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentListResponse {
    pub agents: Vec<AgentIdentity>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EpisodeListResponse {
    pub episodes: Vec<AgentEpisode>,
    pub total: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BehaviorProfileListResponse {
    pub profiles: Vec<AgentBehaviorProfile>,
    pub total: i64,
}

// ============================================================================
// Agent Types
// ============================================================================

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
    General,
    Chat,
    Task,
    Reasoning,
    Research,
    Assistant,
}

impl Default for AgentType {
    fn default() -> Self {
        AgentType::General
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityType {
    Reasoning,
    Memory,
    Perception,
    Action,
    Learning,
    Communication,
}

impl Default for CapabilityType {
    fn default() -> Self {
        CapabilityType::Reasoning
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum EpisodeType {
    Task,
    Conversation,
    Observation,
    Reflection,
    Error,
}

impl Default for EpisodeType {
    fn default() -> Self {
        EpisodeType::Task
    }
}
