use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryAtomType {
    Persona,
    Episodic,
    Instruction,
}

impl MemoryAtomType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Persona => "persona",
            Self::Episodic => "episodic",
            Self::Instruction => "instruction",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "persona" => Some(Self::Persona),
            "episodic" => Some(Self::Episodic),
            "instruction" => Some(Self::Instruction),
            _ => None,
        }
    }
}

impl std::fmt::Display for MemoryAtomType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAtom {
    pub id: String,
    pub atom_type: MemoryAtomType,
    pub content: String,
    pub priority: i16,
    pub scene_name: String,
    pub source_message_ids: Vec<String>,
    pub session_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub tenant_id: String,
    pub metadata: serde_json::Value,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MemoryAtomRow {
    pub id: String,
    pub atom_type: String,
    pub content: String,
    pub priority: i16,
    pub scene_name: String,
    pub source_message_ids: serde_json::Value,
    pub session_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub tenant_id: String,
    pub metadata: serde_json::Value,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<MemoryAtomRow> for MemoryAtom {
    fn from(row: MemoryAtomRow) -> Self {
        let source_message_ids: Vec<String> = serde_json::from_value(row.source_message_ids)
            .unwrap_or_default();
        Self {
            id: row.id,
            atom_type: MemoryAtomType::from_str(&row.atom_type).unwrap_or(MemoryAtomType::Episodic),
            content: row.content,
            priority: row.priority,
            scene_name: row.scene_name,
            source_message_ids,
            session_id: row.session_id,
            user_id: row.user_id,
            agent_id: row.agent_id,
            tenant_id: row.tenant_id,
            metadata: row.metadata,
            version: row.version,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneBlock {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub content: String,
    pub heat: f32,
    pub atom_ids: Vec<String>,
    pub user_id: String,
    pub tenant_id: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SceneBlockRow {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub content: String,
    pub heat: f32,
    pub atom_ids: serde_json::Value,
    pub user_id: String,
    pub tenant_id: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<SceneBlockRow> for SceneBlock {
    fn from(row: SceneBlockRow) -> Self {
        let atom_ids: Vec<String> = serde_json::from_value(row.atom_ids).unwrap_or_default();
        Self {
            id: row.id,
            name: row.name,
            summary: row.summary,
            content: row.content,
            heat: row.heat,
            atom_ids,
            user_id: row.user_id,
            tenant_id: row.tenant_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub id: String,
    pub user_id: String,
    pub agent_id: Option<String>,
    pub tenant_id: String,
    pub content: String,
    pub version: i32,
    pub generated_from_scenes: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PersonaRow {
    pub id: String,
    pub user_id: String,
    pub agent_id: String,
    pub tenant_id: String,
    pub content: String,
    pub version: i32,
    pub generated_from_scenes: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

impl From<PersonaRow> for Persona {
    fn from(row: PersonaRow) -> Self {
        let generated_from_scenes: Vec<String> =
            serde_json::from_value(row.generated_from_scenes).unwrap_or_default();
        Self {
            id: row.id,
            user_id: row.user_id,
            agent_id: if row.agent_id.is_empty() { None } else { Some(row.agent_id) },
            tenant_id: row.tenant_id,
            content: row.content,
            version: row.version,
            generated_from_scenes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// LLM extraction output — a scene segment with extracted memories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedScene {
    pub scene_name: String,
    pub message_ids: Vec<String>,
    pub memories: Vec<ExtractedMemory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedMemory {
    pub content: String,
    #[serde(rename = "type")]
    pub memory_type: String,
    pub priority: i16,
    pub source_message_ids: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Dedup decision from conflict detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DedupDecision {
    Keep,
    Merge,
    Supersede,
    Discard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupResult {
    pub decision: DedupDecision,
    pub existing_id: Option<String>,
    pub merged_content: Option<String>,
}

/// Pipeline execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1ExtractionResult {
    pub success: bool,
    pub extracted_count: usize,
    pub stored_count: usize,
    pub scene_names: Vec<String>,
    pub atom_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2ConsolidationResult {
    pub success: bool,
    pub scenes_created: usize,
    pub scenes_updated: usize,
    pub atoms_processed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L3PersonaResult {
    pub success: bool,
    pub persona_id: String,
    pub version: i32,
}
