use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemorySkill {
    MemorySearch,
    MemoryStore,
    MemoryFusion,
    MemoryStatus,
    KnowledgeGraph,
}

impl MemorySkill {
    /// Every skill, for exhaustive iteration.
    ///
    /// Exists so `handler::detect_skill` can list the accepted ids in a rejection
    /// message without hardcoding a second copy of them, and so the anti-drift
    /// test below can cross-check this set against the agent card.
    pub const ALL: [MemorySkill; 5] = [
        Self::MemorySearch,
        Self::MemoryStore,
        Self::MemoryFusion,
        Self::MemoryStatus,
        Self::KnowledgeGraph,
    ];

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "memory_search" => Some(Self::MemorySearch),
            "memory_store" => Some(Self::MemoryStore),
            "memory_fusion" => Some(Self::MemoryFusion),
            "memory_status" => Some(Self::MemoryStatus),
            "knowledge_graph" => Some(Self::KnowledgeGraph),
            _ => None,
        }
    }

    pub fn id(&self) -> &'static str {
        match self {
            Self::MemorySearch => "memory_search",
            Self::MemoryStore => "memory_store",
            Self::MemoryFusion => "memory_fusion",
            Self::MemoryStatus => "memory_status",
            Self::KnowledgeGraph => "knowledge_graph",
        }
    }
}

#[cfg(test)]
mod skill_id_tests {
    use super::*;

    #[test]
    fn every_skill_round_trips_through_its_id() {
        for skill in MemorySkill::ALL {
            assert_eq!(
                MemorySkill::from_id(skill.id()),
                Some(skill),
                "{skill:?}.id() does not resolve back through from_id"
            );
        }
    }

    /// Anti-drift guard across the three places a skill id lives: this enum's
    /// [`MemorySkill::ALL`], [`MemorySkill::from_id`], and the agent card other
    /// agents actually read.
    ///
    /// The card is the *published contract* — a caller discovers `memory_store`
    /// there and then names it in `message.metadata["skill"]`. If the card
    /// advertises a skill `from_id` cannot resolve, that caller is rejected for
    /// asking exactly what it was told to ask for; if `ALL` grows a skill the card
    /// never mentions, it is undiscoverable. Both directions are asserted against
    /// the real `create_agent_card()` value rather than by scanning source text,
    /// so the guard exercises the same object the endpoint serves.
    #[test]
    fn advertised_skill_ids_and_all_are_the_same_set() {
        let card = crate::a2a::agent_card::create_agent_card("http://localhost:8008");

        let mut advertised: Vec<&str> = card.skills.iter().map(|s| s.id.as_str()).collect();
        advertised.sort_unstable();
        let mut known: Vec<&str> = MemorySkill::ALL.iter().map(|s| s.id()).collect();
        known.sort_unstable();

        assert_eq!(
            advertised, known,
            "agent card skill ids and MemorySkill::ALL have drifted apart"
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchRequest {
    pub query: String,
    pub layer: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStoreRequest {
    pub content: String,
    pub layer: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFusionRequest {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphRequest {
    pub entity: Option<String>,
    pub query: Option<String>,
    pub limit: Option<usize>,
}
