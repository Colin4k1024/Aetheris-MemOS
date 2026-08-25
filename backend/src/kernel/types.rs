//! Memory Kernel - Core Data Types
//!
//! This module defines the core data types for the Memory Kernel system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ulid::Ulid;

/// Unique identifier for a memory entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryId(pub String);

impl MemoryId {
    pub fn new() -> Self {
        Self(Ulid::new().to_string())
    }

    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for MemoryId {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents the type of memory layer (physical/medium).
///
/// These describe **storage and retrieval** backends. For semantic/lifecycle
/// classification, see [`SemanticLayer`] (ADR-0010, #88).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerType {
    /// Short-term memory (ephemeral, fast)
    Stm,
    /// Long-term memory (persistent, indexed)
    Ltm,
    /// Knowledge graph (structured, relational)
    Kg,
    /// Multimodal memory (images, audio, video)
    Mm,
    /// Procedural memory (skills, steps, tool chains)
    Procedural,
}

/// Semantic memory layer — orthogonal to physical [`LayerType`] (ADR-0010, #88).
///
/// | Layer | Description                            | Mutability    |
/// |-------|----------------------------------------|---------------|
/// | L0    | Raw turn/event, full source             | Immutable     |
/// | L1    | Updatable facts, preferences, constraints | Bitemporal    |
/// | L2    | Persona / user profile, with confidence  | Versioned     |
/// | L3    | Scenario / long-term context, goals      | Snapshot      |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SemanticLayer {
    L0,
    L1,
    L2,
    L3,
}

impl std::fmt::Display for SemanticLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticLayer::L0 => write!(f, "L0"),
            SemanticLayer::L1 => write!(f, "L1"),
            SemanticLayer::L2 => write!(f, "L2"),
            SemanticLayer::L3 => write!(f, "L3"),
        }
    }
}

impl std::fmt::Display for LayerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerType::Stm => write!(f, "STM"),
            LayerType::Ltm => write!(f, "LTM"),
            LayerType::Kg => write!(f, "KG"),
            LayerType::Mm => write!(f, "MM"),
            LayerType::Procedural => write!(f, "Procedural"),
        }
    }
}

/// Memory entry content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MemoryContent {
    /// Plain text content
    Text(String),
    /// Structured JSON data
    Json(serde_json::Value),
    /// Binary data (for multimodal)
    Binary(Vec<u8>),
    /// Graph node/edge data
    Graph(GraphData),
}

impl From<String> for MemoryContent {
    fn from(s: String) -> Self {
        MemoryContent::Text(s)
    }
}

impl From<&str> for MemoryContent {
    fn from(s: &str) -> Self {
        MemoryContent::Text(s.to_string())
    }
}

/// Memory weights for layer selection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryWeights {
    pub stm: f64,
    pub ltm: f64,
    pub kg: f64,
    pub mm: f64,
}

/// Graph data for knowledge graph memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: MemoryId,
    pub layer: LayerType,
    pub content: MemoryContent,
    pub metadata: MemoryMetadata,
    pub created_at: i64,
    pub updated_at: i64,
}

impl MemoryEntry {
    pub fn new(layer: LayerType, content: MemoryContent) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: MemoryId::new(),
            layer,
            content,
            metadata: MemoryMetadata::default(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_metadata(mut self, metadata: MemoryMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Metadata associated with a memory entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryMetadata {
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub tags: Vec<String>,
    pub importance: f64,
    pub access_count: u32,
    pub last_accessed: Option<i64>,
    pub expires_at: Option<i64>,
    pub source: Option<String>,
    pub extra: HashMap<String, serde_json::Value>,
}

/// Memory query parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQuery {
    pub layer: Option<LayerType>,
    pub text: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub filters: MemoryFilters,
    pub limit: usize,
    pub offset: usize,
}

impl Default for MemoryQuery {
    fn default() -> Self {
        Self {
            layer: None,
            text: None,
            embedding: None,
            filters: MemoryFilters::default(),
            limit: 10,
            offset: 0,
        }
    }
}

/// Filters for memory queries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryFilters {
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub min_importance: Option<f64>,
    pub created_after: Option<i64>,
    pub created_before: Option<i64>,
}

/// Result of a memory search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMatch {
    pub entry: MemoryEntry,
    pub score: f64,
    pub highlights: Vec<String>,
}

/// Memory operation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryOperation {
    pub id: MemoryId,
    pub layer: LayerType,
    pub operation: OperationType,
    pub timestamp: i64,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    Store,
    Retrieve,
    Update,
    Delete,
    Search,
    Evict,
}

// ============================================================================
// L0–L3 Semantic Memory Types (ADR-0010, #88)
// ============================================================================

/// L1 Fact — an updatable piece of knowledge extracted from L0 events.
///
/// Bitemporal version control: `valid_from`/`valid_until` track real-world
/// time, while `created_at`/`updated_at` track database time. Conflicts are
/// resolved by confidence + recency; old versions are never overwritten.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1Fact {
    pub id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// Confidence score (0.0–1.0) based on evidence quality and quantity.
    pub confidence: f64,
    /// L0 event IDs that support this fact.
    pub evidence_l0_ids: Vec<String>,
    pub version: u32,
    pub valid_from: i64,
    pub valid_until: Option<i64>,
    /// ID of the fact that supersedes this one (if any).
    pub superseded_by: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// L2 Persona — a user profile trait with confidence and evidence.
///
/// Each `PersonaTrait` represents a single dimension of the user profile
/// (preference, constraint, expertise, etc.). It is versioned and linked
/// back to the L1 facts that support it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaTrait {
    pub id: String,
    pub user_id: String,
    /// Dimension: "preference", "constraint", "expertise", "communication_style", "role"
    pub trait_type: String,
    pub trait_value: String,
    /// Confidence derived from evidence quality and user feedback.
    pub confidence: f64,
    /// L1 fact IDs that support this trait.
    pub evidence_l1_ids: Vec<String>,
    pub version: u32,
    pub generated_at: i64,
    pub updated_at: i64,
}

/// L3 Scenario — a long-term context, goal, or behavioral pattern.
///
/// Scenarios aggregate L1 facts and L2 persona traits into a high-level
/// understanding of the user's current situation, project, or goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub user_id: String,
    /// "project", "support_ticket", "learning_path", "relationship"
    pub scenario_type: String,
    pub summary: String,
    pub goals: Vec<String>,
    pub constraints: Vec<String>,
    /// IDs of related L1 facts and L2 persona traits.
    pub evidence_ids: Vec<String>,
    /// "active", "paused", "closed"
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    fn semantic_layer_display() {
        assert_eq!(SemanticLayer::L0.to_string(), "L0");
        assert_eq!(SemanticLayer::L1.to_string(), "L1");
        assert_eq!(SemanticLayer::L2.to_string(), "L2");
        assert_eq!(SemanticLayer::L3.to_string(), "L3");
    }

    #[test]
    fn semantic_layer_serde_roundtrip() {
        for layer in &[SemanticLayer::L0, SemanticLayer::L1, SemanticLayer::L2, SemanticLayer::L3] {
            let json = serde_json::to_string(layer).unwrap();
            let back: SemanticLayer = serde_json::from_str(&json).unwrap();
            assert_eq!(*layer, back);
        }
    }

    #[test]
    fn l1_fact_has_evidence_backlink() {
        let fact = L1Fact {
            id: "f1".into(),
            subject: "user".into(),
            predicate: "prefers".into(),
            object: "dark mode".into(),
            confidence: 0.9,
            evidence_l0_ids: vec!["ev-1".into(), "ev-2".into()],
            version: 1,
            valid_from: 1000,
            valid_until: None,
            superseded_by: None,
            created_at: 1000,
            updated_at: 1000,
        };
        assert_eq!(fact.evidence_l0_ids.len(), 2);
        assert!(fact.valid_until.is_none());
    }

    #[test]
    fn persona_trait_has_confidence_bounds() {
        let trait_ = PersonaTrait {
            id: "p1".into(),
            user_id: "u1".into(),
            trait_type: "preference".into(),
            trait_value: "concise answers".into(),
            confidence: 0.75,
            evidence_l1_ids: vec!["f1".into()],
            version: 1,
            generated_at: 1000,
            updated_at: 1000,
        };
        assert!(trait_.confidence >= 0.0 && trait_.confidence <= 1.0);
        assert!(!trait_.evidence_l1_ids.is_empty());
    }

    #[test]
    fn scenario_has_status_lifecycle() {
        let scenario = Scenario {
            id: "s1".into(),
            user_id: "u1".into(),
            scenario_type: "project".into(),
            summary: "Building a web app".into(),
            goals: vec!["deploy MVP".into()],
            constraints: vec!["budget limited".into()],
            evidence_ids: vec!["f1".into(), "p1".into()],
            status: "active".into(),
            created_at: 1000,
            updated_at: 1000,
        };
        assert_eq!(scenario.status, "active");
        assert!(!scenario.evidence_ids.is_empty());
    }
}
