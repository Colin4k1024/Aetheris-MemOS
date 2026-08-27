//! Belief lifecycle row models (#127).
//!
//! These are the durable shapes behind [`crate::models::belief`] (the pure
//! semantics contract): governed predicate policies materialized into PG, the
//! bitemporal SPO belief edges, guard candidates with their verdicts,
//! provenance evidence, and per-agent memory contracts.
//!
//! Timestamps arrive as `::text` casts (repo convention — see `memory_enums`
//! and the TIMESTAMPTZ gotcha documented in db/distillation.rs history).

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// One row of `memory_predicate_policies` (the #125 catalog in PG form).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PredicatePolicyRow {
    pub name: String,
    pub cardinality: String,
    pub mutability: String,
    /// JSON array of allowed source strings.
    pub allowed_sources: String,
    /// TTL policy discriminant (`no_ttl` / `stale_scan` / ...).
    pub ttl_policy: String,
    pub reconfirm_days: Option<i32>,
    pub risk: String,
    pub description: String,
}

/// A current or historical SPO edge: "subject P object", valid during
/// `[valid_from, valid_to)`, as believed by the system since `recorded_at`.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MemoryBelief {
    pub id: String,
    pub tenant_id: String,
    pub principal_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub status: String,
    pub source: String,
    pub trust: f32,
    pub risk: String,
    pub valid_from: String,
    pub valid_to: Option<String>,
    pub recorded_at: String,
    pub supersedes_id: Option<String>,
    pub superseded_by_id: Option<String>,
    pub needs_confirm: bool,
    pub metadata_json: String,
}

impl MemoryBelief {
    /// An open edge is one the retrieval layer may consider under
    /// `as_of=now()`: active (current truth) or needs_confirm (parked for a
    /// human; still occupies the single-edge slot but never drives actions).
    pub fn is_open(&self) -> bool {
        self.valid_to.is_none() && matches!(self.status.as_str(), "active" | "needs_confirm")
    }

    /// Whether this edge may be presented to consumers as CURRENT truth. High
    /// risk + unconfirmed = parked: the status machine forbids it from driving
    /// actions until a human confirms (#127 acceptance 6).
    pub fn drives_actions(&self) -> bool {
        self.status == "active" && !self.needs_confirm && self.valid_to.is_none()
    }
}

/// A pre-commit proposition awaiting/holding the gate's verdict.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MemoryBeliefCandidate {
    pub id: String,
    pub tenant_id: String,
    pub principal_id: String,
    pub session_id: Option<String>,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub source: String,
    pub trust: f32,
    pub origin: String,
    pub decision: Option<String>,
    pub status: String,
    pub outcome_belief_id: Option<String>,
    pub rejection_reason: Option<String>,
    pub payload_json: String,
    pub idempotency_key: Option<String>,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

/// Provenance binding a committed belief or a parked candidate back to the
/// immutable event stream.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MemoryBeliefEvidence {
    pub id: String,
    pub tenant_id: String,
    pub belief_id: Option<String>,
    pub candidate_id: Option<String>,
    pub event_id: Option<String>,
    pub kind: String,
    pub content_hash: String,
    pub created_at: String,
}

// ============================================================================
// Gate input / output
// ============================================================================

/// How this claim reached the gate (mechanical origin, orthogonal to trust).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimOrigin {
    Manual,
    Distillation,
    External,
    Api,
}

impl ClaimOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            ClaimOrigin::Manual => "manual",
            ClaimOrigin::Distillation => "distillation",
            ClaimOrigin::External => "external",
            ClaimOrigin::Api => "api",
        }
    }
}

/// A proposition submitted to the write gate. Everything the decision needs;
/// the gate performs NO LLM work and trusts no caller-side classification —
/// sources, risks and allowlist membership are enforced here.
#[derive(Debug, Clone)]
pub struct BeliefClaim {
    pub principal_id: String,
    pub session_id: Option<String>,
    pub subject: String,
    /// Must be in the #125/#127 allowlist or the claim is rejected outright.
    pub predicate: String,
    pub object: String,
    pub source: crate::models::belief::BeliefSource,
    /// Evidence anchors into memory_events. At least ONE is required for any
    /// claim that could become active — an unevidenced belief cannot exist.
    pub evidence_event_ids: Vec<String>,
    /// Free-form JSON preserved on the candidate row for audit.
    pub payload_json: serde_json::Value,
    pub origin: ClaimOrigin,
    /// Replay guard over the CLAIM level (distinct from event idempotency).
    pub idempotency_key: Option<String>,
}

impl BeliefClaim {
    pub fn new(
        principal_id: impl Into<String>,
        subject: impl Into<String>,
        predicate: impl Into<String>,
        object: impl Into<String>,
        source: crate::models::belief::BeliefSource,
    ) -> Self {
        Self {
            principal_id: principal_id.into(),
            session_id: None,
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            source,
            evidence_event_ids: Vec::new(),
            payload_json: serde_json::json!({}),
            origin: ClaimOrigin::Api,
            idempotency_key: None,
        }
    }

    pub fn session(mut self, v: impl Into<String>) -> Self {
        self.session_id = Some(v.into());
        self
    }

    pub fn evidence(mut self, event_ids: Vec<String>) -> Self {
        self.evidence_event_ids = event_ids;
        self
    }

    pub fn payload(mut self, v: serde_json::Value) -> Self {
        self.payload_json = v;
        self
    }

    pub fn origin(mut self, o: ClaimOrigin) -> Self {
        self.origin = o;
        self
    }

    pub fn idempotency_key(mut self, v: impl Into<String>) -> Self {
        self.idempotency_key = Some(v.into());
        self
    }

    /// Canonical text handed to the injection probe (subjects are ids; objects
    /// may carry attacker-influenced text, so probe reads object + payload).
    pub fn probe_text(&self) -> String {
        format!("{} {}", self.object, self.payload_json)
    }
}

/// The gate's terminal verdict for one submitted claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateOutcome {
    /// New open edge written (status may be `needs_confirm` for high-risk or
    /// weak-source medium-risk claims). Carries the belief + candidate ids.
    Committed {
        candidate_id: String,
        belief_id: String,
        /// True when human confirmation is still required before the edge can
        /// drive actions.
        needs_confirm: bool,
    },
    /// Existing equivalent open edge found; nothing written (idempotent replay
    /// of an identical fact never creates another version).
    Noop {
        candidate_id: String,
        belief_id: String,
    },
    /// Old edge closed + new edge opened (history retained via supersedes).
    Superseded {
        candidate_id: String,
        new_belief_id: String,
        superseded_belief_id: String,
        needs_confirm: bool,
    },
    /// The claim conflicts with a strictly STRONGER open edge; both facts are
    /// kept (edge stays put, candidate parks for review). Never auto-resolves.
    Conflict {
        candidate_id: String,
        existing_belief_id: String,
    },
    /// Injection probe quarantined the claim's text. Persisted as a
    /// quarantined candidate ONLY — it can never reach an active edge without
    /// explicit human promotion through the state machine.
    Quarantined {
        candidate_id: String,
        reason: String,
    },
    /// Rejected before evaluation (not in allowlist / disallowed source /
    /// missing evidence). Candidate row keeps the reason for audit.
    Rejected {
        candidate_id: String,
        reason: String,
    },
}

impl GateOutcome {
    pub fn candidate_id(&self) -> &str {
        match self {
            GateOutcome::Committed { candidate_id, .. }
            | GateOutcome::Noop { candidate_id, .. }
            | GateOutcome::Superseded { candidate_id, .. }
            | GateOutcome::Conflict { candidate_id, .. }
            | GateOutcome::Quarantined { candidate_id, .. }
            | GateOutcome::Rejected { candidate_id, .. } => candidate_id,
        }
    }
}
