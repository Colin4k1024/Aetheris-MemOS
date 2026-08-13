use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillStatus {
    Draft,
    Active,
    Archived,
}

impl SkillStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Private,
    Team,
    Restricted,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Team => "team",
            Self::Restricted => "restricted",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStep {
    pub order: i32,
    pub description: String,
    #[serde(default)]
    pub tool_calls: Vec<String>,
    pub expected_output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: i32,
    pub status: SkillStatus,
    pub trigger_conditions: Vec<String>,
    pub execution_steps: Vec<SkillStep>,
    pub validation_rules: Vec<String>,
    pub owner_user_id: String,
    pub owner_agent_id: Option<String>,
    pub tenant_id: String,
    pub visibility: Visibility,
    pub tags: Vec<String>,
    pub usage_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCreateRequest {
    pub name: String,
    pub description: String,
    pub trigger_conditions: Vec<String>,
    pub execution_steps: Vec<SkillStep>,
    pub validation_rules: Vec<String>,
    pub tags: Vec<String>,
    pub visibility: Option<Visibility>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExtractRequest {
    pub session_id: String,
    pub user_id: String,
    pub agent_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMatchRequest {
    pub query: String,
    pub user_id: String,
    pub agent_id: Option<String>,
    pub tenant_id: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMatchResult {
    pub skill: Skill,
    pub relevance_score: f64,
}
