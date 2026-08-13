use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtomType {
    Persona,
    Episodic,
    Instruction,
}

impl AtomType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AtomType::Persona => "persona",
            AtomType::Episodic => "episodic",
            AtomType::Instruction => "instruction",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "persona" => Some(AtomType::Persona),
            "episodic" => Some(AtomType::Episodic),
            "instruction" => Some(AtomType::Instruction),
            _ => None,
        }
    }
}

impl std::fmt::Display for AtomType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct L1Atom {
    pub id: String,
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub atom_type: String,
    pub scene_name: String,
    pub content: String,
    pub priority: f32,
    pub source_session_id: String,
    pub source_message_ids: serde_json::Value,
    pub metadata: serde_json::Value,
    pub embedding_model: Option<String>,
    pub embedding_dimension: Option<i32>,
    pub is_active: bool,
    pub superseded_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAtomRequest {
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub atom_type: AtomType,
    pub scene_name: String,
    pub content: String,
    pub priority: f32,
    pub source_session_id: String,
    pub source_message_ids: Vec<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct L2Scene {
    pub id: String,
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub scene_name: String,
    pub title: String,
    pub content: String,
    pub atom_ids: serde_json::Value,
    pub version: i32,
    pub token_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct L3Persona {
    pub id: String,
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub profile_content: String,
    pub scene_ids: serde_json::Value,
    pub version: i32,
    pub token_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistillationJobType {
    L0ToL1,
    L1ToL2,
    L2ToL3,
    SkillExtract,
}

impl DistillationJobType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DistillationJobType::L0ToL1 => "l0_to_l1",
            DistillationJobType::L1ToL2 => "l1_to_l2",
            DistillationJobType::L2ToL3 => "l2_to_l3",
            DistillationJobType::SkillExtract => "skill_extract",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "l0_to_l1" => Some(DistillationJobType::L0ToL1),
            "l1_to_l2" => Some(DistillationJobType::L1ToL2),
            "l2_to_l3" => Some(DistillationJobType::L2ToL3),
            "skill_extract" => Some(DistillationJobType::SkillExtract),
            _ => None,
        }
    }
}

impl std::fmt::Display for DistillationJobType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
        }
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DistillationJob {
    pub id: String,
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub session_id: String,
    pub job_type: String,
    pub status: String,
    pub error_message: Option<String>,
    pub atoms_created: Option<i32>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
}
