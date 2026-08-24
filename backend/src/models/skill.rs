use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Private,
    Team,
    Public,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Visibility::Private => "private",
            Visibility::Team => "team",
            Visibility::Public => "public",
        }
    }
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillStatus {
    Draft,
    Active,
    Deprecated,
}

impl SkillStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkillStatus::Draft => "draft",
            SkillStatus::Active => "active",
            SkillStatus::Deprecated => "deprecated",
        }
    }
}

impl std::fmt::Display for SkillStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerCondition {
    pub condition_type: String,
    pub value: String,
    pub confidence_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub order: i32,
    pub action: String,
    pub description: String,
    pub tool_call: Option<String>,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub rule_type: String,
    pub expected: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Skill {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: String,
    pub version: i32,
    pub trigger_conditions: serde_json::Value,
    pub execution_steps: serde_json::Value,
    pub validation_rules: serde_json::Value,
    pub source_session_ids: serde_json::Value,
    pub owner_agent_id: String,
    pub visibility: String,
    pub status: String,
    pub embedding_model: Option<String>,
    pub embedding_dimension: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSkillRequest {
    pub name: String,
    pub description: String,
    pub trigger_conditions: Vec<TriggerCondition>,
    pub execution_steps: Vec<ExecutionStep>,
    pub validation_rules: Vec<ValidationRule>,
    pub owner_agent_id: String,
    pub visibility: Visibility,
}

/// Partial update of a skill. All fields optional; `None` = unchanged.
/// `status` transitions (draft→active→deprecated) go through here. Publishing
/// a new `version` revision is a follow-up.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSkillRequest {
    pub description: Option<String>,
    pub visibility: Option<Visibility>,
    pub status: Option<SkillStatus>,
}
