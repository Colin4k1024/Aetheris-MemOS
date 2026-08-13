use serde::{Deserialize, Serialize};
use sqlx::FromRow;

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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentEquipment {
    pub id: String,
    pub tenant_id: String,
    pub agent_id: String,
    pub asset_type: String,
    pub asset_id: String,
    pub binding_type: String,
    pub condition: Option<serde_json::Value>,
    pub priority: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEquipmentRequest {
    pub agent_id: String,
    pub asset_type: EquipAssetType,
    pub asset_id: String,
    pub binding_type: BindingType,
    pub condition: Option<serde_json::Value>,
    pub priority: i32,
}
