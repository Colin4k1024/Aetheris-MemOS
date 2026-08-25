use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ============================================================================
// Asset Type
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipAssetType {
    Skill,
    L1Memory,
    L2Scene,
    L3Persona,
}

impl EquipAssetType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EquipAssetType::Skill => "skill",
            EquipAssetType::L1Memory => "l1_memory",
            EquipAssetType::L2Scene => "l2_scene",
            EquipAssetType::L3Persona => "l3_persona",
        }
    }
}

impl std::fmt::Display for EquipAssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Binding Type
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingType {
    Fixed,
    Dynamic,
    Conditional,
}

impl BindingType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BindingType::Fixed => "fixed",
            BindingType::Dynamic => "dynamic",
            BindingType::Conditional => "conditional",
        }
    }
}

impl std::fmt::Display for BindingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Visibility (#89)
// ============================================================================

/// Controls who can discover and use an asset binding.
///
/// Application-layer visibility is enforced by the service layer in addition
/// to database RLS (which already enforces tenant isolation). The two layers
/// are complementary: RLS prevents cross-tenant reads, visibility prevents
/// intra-tenant information leaks (e.g., a team member seeing private agent
/// configurations).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Only the owning agent can see and use this binding.
    Private,
    /// Members of the same team can see this binding.
    Team,
    /// All agents in the tenant can see this binding (restricted to the
    /// same tenant by RLS).
    Tenant,
    /// Explicitly shared with a specific agent (non-owner).
    Agent,
    /// Available to any agent running the specified task.
    Task,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Visibility::Private => "private",
            Visibility::Team => "team",
            Visibility::Tenant => "tenant",
            Visibility::Agent => "agent",
            Visibility::Task => "task",
        }
    }
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Private
    }
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Equipment Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentEquipment {
    pub id: String,
    pub tenant_id: String,
    pub agent_id: String,
    pub asset_type: String,
    pub asset_id: String,
    pub binding_type: String,
    pub visibility: String,
    pub condition: Option<serde_json::Value>,
    pub priority: i32,
    pub created_at: String,
}

/// Create an equipment binding. `agent_id` comes from the URL path
/// (`/agents/{agent_id}/equipment`), not the body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEquipmentRequest {
    pub asset_type: EquipAssetType,
    pub asset_id: String,
    pub binding_type: BindingType,
    #[serde(default)]
    pub visibility: Visibility,
    pub condition: Option<serde_json::Value>,
    pub priority: i32,
}

/// Partial update of an equipment binding. All fields optional; `None` means
/// "leave unchanged" (cannot currently clear a field back to NULL — follow-up).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEquipmentRequest {
    pub binding_type: Option<BindingType>,
    pub visibility: Option<Visibility>,
    pub condition: Option<serde_json::Value>,
    pub priority: Option<i32>,
}

/// Atomic snapshot of all assets bound to an agent, grouped by asset type
/// (#89). A first increment: the binding rows grouped; resolving the actual
/// asset bodies (skill/scene/persona content) by `asset_id` is a follow-up —
/// callers can fetch each asset by id from its type-specific endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loadout {
    pub agent_id: String,
    pub skills: Vec<AgentEquipment>,
    pub l1_memories: Vec<AgentEquipment>,
    pub l2_scenes: Vec<AgentEquipment>,
    pub l3_personas: Vec<AgentEquipment>,
}

// ============================================================================
// ACL Rule (#89)
// ============================================================================

/// Canonical ACL rule for an asset binding.
///
/// Derived from the equipment's `visibility` and `binding_type` at loadout
/// resolution time. The service layer enforces these rules; the database RLS
/// provides a second layer of defense.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRule {
    pub equipment_id: String,
    pub asset_type: String,
    pub asset_id: String,
    pub visibility: String,
    /// Agent IDs that are explicitly allowed (empty = visibility-based).
    pub allowed_agents: Vec<String>,
    /// Team IDs that can access (empty = no team-level access).
    pub allowed_teams: Vec<String>,
    /// Task IDs that can access (empty = no task-level access).
    pub allowed_tasks: Vec<String>,
}
