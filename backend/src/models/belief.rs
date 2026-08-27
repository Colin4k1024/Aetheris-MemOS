//! Belief lifecycle semantics - the shared vocabulary for the memory epic
//! (#124-#130).
//!
//! This module is the **single source of truth** in code for:
//!
//! - [`BeliefSource`] - where a belief claim came from (trust tier + precedence)
//! - [`BeliefStatus`] - the lifecycle state machine (validated transitions)
//! - [`WriteDecision`] - the guard's verdict when a candidate meets current edges
//! - [`PREDICATE_CATALOG`] - the first batch of governed predicates with
//!   cardinality / mutability / source policy / TTL / risk definitions
//!
//! The prose contract (state diagram, rationale, and the ADR numbering) lives in
//! `docs/adr/ADR-0011-belief-lifecycle-state-machine.md`; the two must stay in
//! lockstep. No storage is attached here on purpose: tables arrive with #127
//! (belief model + write gate). Until then this module is consumed by tests,
//! the ADR, and the extraction/guard work that follows.
//!
//! Terminology follows ADR-0010 (revised): the semantic lifecycle is
//! Event / Belief / Persona-Scenario / Procedure, orthogonal to the physical
//! storage layers (STM / LTM / KG / Qdrant / MM).

use serde::{Deserialize, Serialize};

/// Build the caller-facing message for an invalid enum value.
///
/// Lists every valid value so a caller can fix the input from the error alone -
/// same contract as [`crate::models::memory_enums`].
fn invalid_value_message(field: &str, got: &str, valid: &[&str]) -> String {
    format!(
        "invalid {field}: '{got}' is not a valid value; valid values are: {}",
        valid.join(", ")
    )
}

// ============================================================================
// Source enum
// ============================================================================

/// Where a belief claim originated.
///
/// Source determines the baseline trust score, the precedence when two claims
/// conflict, and whether the claim may drive high-stakes actions. Strong sources
/// beat weak sources on conflict (`Epic #124`: "聊天抽取是弱源，系统记录是强源。
/// 冲突时强源赢。").
///
/// Canonical values (ADR-0011): `user_stated | tool | system_of_record | web | inferred`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeliefSource {
    /// The user said it in conversation (strong for self-facts, weak for org facts).
    UserStated,
    /// A tool / agent invocation returned it (structured, machine-shaped).
    Tool,
    /// An authoritative system of record asserted it: CRM, HR, ticketing
    /// (strongest; reconciliation target for the consolidation job, #129).
    SystemOfRecord,
    /// Observed on a web page / untrusted email (weakest; trust decays after 48h
    /// and must never drive authorization-class beliefs).
    Web,
    /// The model inferred it from evidence rather than anyone asserting it.
    Inferred,
}

impl BeliefSource {
    /// All valid values, ordered strongest-first (this IS the precedence order).
    pub const ALL: &'static [&'static str] =
        &["system_of_record", "user_stated", "tool", "inferred", "web"];

    /// Canonical string persisted to the DB and used on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            BeliefSource::UserStated => "user_stated",
            BeliefSource::Tool => "tool",
            BeliefSource::SystemOfRecord => "system_of_record",
            BeliefSource::Web => "web",
            BeliefSource::Inferred => "inferred",
        }
    }

    /// Exact-match parse; `Err` is a caller-facing message listing valid values.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "user_stated" => Ok(BeliefSource::UserStated),
            "tool" => Ok(BeliefSource::Tool),
            "system_of_record" => Ok(BeliefSource::SystemOfRecord),
            "web" => Ok(BeliefSource::Web),
            "inferred" => Ok(BeliefSource::Inferred),
            other => Err(invalid_value_message("belief source", other, Self::ALL)),
        }
    }

    /// Precedence rank, lower = stronger. On conflict the stronger source wins;
    /// equal ranks fall through to recency + confidence.
    pub fn precedence_rank(self) -> u8 {
        match self {
            BeliefSource::SystemOfRecord => 0,
            BeliefSource::UserStated => 1,
            BeliefSource::Tool => 2,
            BeliefSource::Inferred => 3,
            BeliefSource::Web => 4,
        }
    }

    /// Baseline trust score before freshness/predicate adjustment.
    ///
    /// Anchored to `Epic #124`: system-of-record is the reconciliation truth,
    /// a user statement is trusted but fallible, and a web observation starts
    /// low and decays below the action threshold within 48h (decay itself is a
    /// #129 consolidation job, not this constant).
    pub fn base_trust(self) -> f64 {
        match self {
            BeliefSource::SystemOfRecord => 0.95,
            BeliefSource::UserStated => 0.85,
            BeliefSource::Tool => 0.70,
            BeliefSource::Inferred => 0.50,
            BeliefSource::Web => 0.30,
        }
    }

    /// True when a source of this class may assert the given predicate at all.
    pub fn may_assert(self, predicate: &PredicateSpec) -> bool {
        predicate.allowed_sources.contains(&self)
    }
}

// ============================================================================
// Status enum + state machine
// ============================================================================

/// Lifecycle state of a belief.
///
/// State machine (ADR-0011) - arrows are the ONLY legal transitions:
///
/// ```text
///            ┌────────────┐   review: false positive
///            │ quarantined│──────────────┐
///            └─────┬──────┘              ▼
///        guard pass│              ┌───────────┐
///                  ▼               │  rejected │ (terminal)
///            ┌───────────┐  NOOP   └───────────┘
///     ┌─────►│ candidate │─────────► (not persisted)
///     │      └─────┬─────┘
///     │            │ ADD / SUPERSEDE
///     │            ├────────────────────┐
///     │   conflict ▼                    ▼
///     │      ┌──────────────┐    ┌────────┐ supersede (new edge)
///     └──────│ needs_confirm│    │ active │──────────────┐
///            └──────┬───────┘    └───┬────┘              ▼
///            confirm│ deny           │ stale scan  ┌───────────┐
///                   ▼         confirm│ (90d)       │ superseded │
///              ┌────────┐   reconfirm │             └─────┬─────┘
///              │ active │◄────────────┘                   │ consolidate
///              └───┬────┘                                 ▼
///      consolidate │                                ┌──────────┐
///                  ▼                                │ archived │ (terminal)
///              ┌──────────┐                          └──────────┘
///              │  stale   │── archive ──────────────►
///              └──────────┘
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeliefStatus {
    /// Intercepted by the write guard (low trust / instruction-shaped injection).
    /// Parked for review; never enters retrieval (#127's quarantine area).
    Quarantined,
    /// Extracted proposition not yet committed - transient, compared against
    /// current edges to produce a [`WriteDecision`].
    Candidate,
    /// Current truth. The default `as_of=now()` retrieval surface (#128).
    Active,
    /// High-risk or conflicting; a human must confirm before it becomes active.
    NeedsConfirm,
    /// Was true, probably no longer: mutable fact past its reconfirmation
    /// window, or an authority no longer vouches for it (#129).
    Stale,
    /// Replaced by a newer edge (`supersedes`); `valid_to` closed, history kept.
    Superseded,
    /// Removed from retrieval by the consolidation job (low trust + low recall
    /// + expired). Kept for audit, invisible to working-memory assembly.
    Archived,
    /// Reviewed and discarded. Terminal.
    Rejected,
}

impl BeliefStatus {
    /// All valid values.
    pub const ALL: &'static [&'static str] = &[
        "quarantined",
        "candidate",
        "active",
        "needs_confirm",
        "stale",
        "superseded",
        "archived",
        "rejected",
    ];

    /// Canonical string persisted to the DB and used on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            BeliefStatus::Quarantined => "quarantined",
            BeliefStatus::Candidate => "candidate",
            BeliefStatus::Active => "active",
            BeliefStatus::NeedsConfirm => "needs_confirm",
            BeliefStatus::Stale => "stale",
            BeliefStatus::Superseded => "superseded",
            BeliefStatus::Archived => "archived",
            BeliefStatus::Rejected => "rejected",
        }
    }

    /// Exact-match parse; `Err` is a caller-facing message listing valid values.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "quarantined" => Ok(BeliefStatus::Quarantined),
            "candidate" => Ok(BeliefStatus::Candidate),
            "active" => Ok(BeliefStatus::Active),
            "needs_confirm" => Ok(BeliefStatus::NeedsConfirm),
            "stale" => Ok(BeliefStatus::Stale),
            "superseded" => Ok(BeliefStatus::Superseded),
            "archived" => Ok(BeliefStatus::Archived),
            "rejected" => Ok(BeliefStatus::Rejected),
            other => Err(invalid_value_message("belief status", other, Self::ALL)),
        }
    }

    /// The legal successor states, i.e. the whole state machine in table form.
    pub fn allowed_transitions_from(self) -> &'static [BeliefStatus] {
        match self {
            BeliefStatus::Quarantined => &[
                BeliefStatus::Candidate, // review: false positive, resume pipeline
                BeliefStatus::Rejected,  // review: poison, discard
            ],
            BeliefStatus::Candidate => &[
                BeliefStatus::Active,       // ADD: low risk, trusted source
                BeliefStatus::NeedsConfirm, // ADD: high risk / weak source
                BeliefStatus::Rejected,     // NOOP: duplicate of current edge
            ],
            BeliefStatus::NeedsConfirm => &[
                BeliefStatus::Active,   // human confirmed
                BeliefStatus::Rejected, // human denied
            ],
            BeliefStatus::Active => &[
                BeliefStatus::Superseded,   // newer edge for same (s,p)
                BeliefStatus::Stale,        // reconfirmation window elapsed
                BeliefStatus::NeedsConfirm, // consolidation found a conflict
                BeliefStatus::Archived,     // consolidate: low value
            ],
            BeliefStatus::Stale => &[
                BeliefStatus::Active,     // authority reconfirmed
                BeliefStatus::Superseded, // replacement arrived while stale
                BeliefStatus::Archived,   // consolidation gives up on it
            ],
            BeliefStatus::Superseded => &[BeliefStatus::Archived],
            BeliefStatus::Archived => &[], // terminal
            BeliefStatus::Rejected => &[], // terminal
        }
    }

    /// Whether `to` is a legal successor of `self`.
    pub fn can_transition_to(self, to: BeliefStatus) -> bool {
        self.allowed_transitions_from().contains(&to)
    }

    /// Terminal states never transition again.
    pub fn is_terminal(self) -> bool {
        self.allowed_transitions_from().is_empty()
    }

    /// Whether a belief in this state is visible to default (`as_of=now()`)
    /// working-memory assembly (#128). Only current truth is; stale and
    /// quarantined items are explicitly out of the default surface.
    pub fn is_retrievable(self) -> bool {
        matches!(self, BeliefStatus::Active)
    }
}

// ============================================================================
// Write decision
// ============================================================================

/// The guard's verdict when a candidate proposition is compared against the
/// current edge(s) for the same `(subject, predicate)` pair.
///
/// `SUPERSEDE` closes the old edge's `valid_to` and opens the new one - history
/// is retained, never overwritten in place (`Epic #124`: "取代，不覆盖").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteDecision {
    /// No current edge conflicts - insert as new truth.
    Add,
    /// A current edge exists with a different object - close it (`valid_to`)
    /// and insert the replacement, linking `supersedes`.
    Supersede,
    /// An equivalent current edge already exists - nothing to write.
    Noop,
    /// The candidate and current edges disagree and neither source outranks the
    /// other - park both in `needs_confirm` for human resolution.
    Conflict,
}

impl WriteDecision {
    /// All valid values.
    pub const ALL: &'static [&'static str] = &["add", "supersede", "noop", "conflict"];

    /// Canonical string form.
    pub fn as_str(self) -> &'static str {
        match self {
            WriteDecision::Add => "add",
            WriteDecision::Supersede => "supersede",
            WriteDecision::Noop => "noop",
            WriteDecision::Conflict => "conflict",
        }
    }

    /// Exact-match parse; `Err` is a caller-facing message listing valid values.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "add" => Ok(WriteDecision::Add),
            "supersede" => Ok(WriteDecision::Supersede),
            "noop" => Ok(WriteDecision::Noop),
            "conflict" => Ok(WriteDecision::Conflict),
            other => Err(invalid_value_message("write decision", other, Self::ALL)),
        }
    }

    /// Whether this decision persists a new belief row at all.
    pub fn writes(self) -> bool {
        !matches!(self, WriteDecision::Noop)
    }
}

// ============================================================================
// Predicate catalog
// ============================================================================

/// How many current values a predicate may hold for one subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredicateCardinality {
    /// At most one current edge per subject (e.g. one current employer).
    /// A second distinct value triggers `SUPERSEDE`, never coexistence.
    Single,
    /// Many current edges may coexist (e.g. a person prefers many things).
    /// A contradictory value only supersedes the matching edge, if any.
    Multi,
}

/// How the predicate's truth changes over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredicateMutability {
    /// May change; new evidence supersedes the old edge. Subject to stale scans.
    Mutable,
    /// Recorded once and never rewritten (contract numbers, birthplace).
    /// Correction means a new evidence-backed edge with human confirmation,
    /// not a time-based overwrite.
    Immutable,
    /// True only until a due date carried by the object (`promised`). On
    /// expiry the belief leaves the current set and survives as episodic
    /// history ("到期后变成情节，不再当现行事实").
    TimeBounded,
}

/// Risk class of a predicate, driving the confirmation requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskTier {
    /// No gating; written straight to `active`.
    Low,
    /// Requires confirmation when asserted by a weak source (web / inferred).
    Medium,
    /// Authorization-, payment- or approval-adjacent. Requires either a
    /// system-of-record source or explicit human confirmation before `active`.
    High,
}

impl RiskTier {
    /// Canonical string persisted to the DB.
    pub fn as_str(self) -> &'static str {
        match self {
            RiskTier::Low => "low",
            RiskTier::Medium => "medium",
            RiskTier::High => "high",
        }
    }
}

impl RiskTier {
    /// Whether a claim of this risk class from `source` must stop in
    /// `needs_confirm` for a human before becoming `active`.
    ///
    /// This encodes the memory-contract rule from `Epic #124`: a high-stakes
    /// action may only be driven by `system_of_record` (or admin-confirmed)
    /// beliefs - a user saying "I own account X" must not grant ownership.
    pub fn confirmation_required_for(self, source: BeliefSource) -> bool {
        match self {
            RiskTier::Low => false,
            RiskTier::Medium => matches!(source, BeliefSource::Web | BeliefSource::Inferred),
            RiskTier::High => source != BeliefSource::SystemOfRecord,
        }
    }
}

/// How a belief's expiry / staleness is determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtlPolicy {
    /// No time-based expiry. Only a supersedes edge or explicit archival retires it.
    NoTtl,
    /// Mutable fact: the consolidation job (#129) marks it `stale` after
    /// `reconfirm_days` without reconfirmation from any allowed source.
    StaleScan { reconfirm_days: u32 },
    /// Retired by system-of-record reconciliation, not by time ("权威系统更新时
    /// 失效，不靠时间"). The nightly SoR sweep (#129) closes or flags the edge.
    SorDriven,
    /// Valid until the due date on the object; expiry converts the belief to
    /// episodic history rather than stale.
    ExpiresAtDueDate,
}

/// One governed predicate: the contract every extractor and the write gate
/// (#127) must honour when touching this predicate.
#[derive(Debug, Clone, Copy)]
pub struct PredicateSpec {
    /// Predicate name as it appears in extraction output and the belief graph.
    pub name: &'static str,
    /// Single- or multi-valued for one subject.
    pub cardinality: PredicateCardinality,
    /// Mutability class.
    pub mutability: PredicateMutability,
    /// Source policy: the only sources allowed to assert this predicate.
    /// A claim from anything else is rejected at the gate (not quarantined -
    /// it is not even a candidate).
    pub allowed_sources: &'static [BeliefSource],
    /// TTL / staleness policy.
    pub ttl: TtlPolicy,
    /// Risk class and confirmation requirement.
    pub risk: RiskTier,
    /// One-line semantics for humans; part of the extractor prompt contract.
    pub description: &'static str,
}

impl PredicateSpec {
    /// Whether `source` may assert this predicate.
    pub fn allows_source(&self, source: BeliefSource) -> bool {
        source.may_assert(self)
    }

    /// Whether a claim of this predicate from `source` needs human confirmation
    /// before activation.
    pub fn confirmation_required_from(&self, source: BeliefSource) -> bool {
        self.risk.confirmation_required_for(source)
    }
}

/// The first batch of governed predicates (ADR-0011 §predicate catalog).
///
/// Deliberately tiny: the extraction quality contract is "only these predicates,
/// only from these sources" - everything else stays an unstructured event.
/// Extending the catalog is an ADR-level decision, not a code change.
pub const PREDICATE_CATALOG: &[PredicateSpec] = &[
    PredicateSpec {
        name: "works_at",
        cardinality: PredicateCardinality::Single,
        mutability: PredicateMutability::Mutable,
        allowed_sources: &[
            BeliefSource::SystemOfRecord,
            BeliefSource::UserStated,
            BeliefSource::Tool,
        ],
        ttl: TtlPolicy::StaleScan { reconfirm_days: 90 },
        risk: RiskTier::Medium,
        description: "subject person currently works at object organization",
    },
    PredicateSpec {
        name: "reports_to",
        cardinality: PredicateCardinality::Single,
        mutability: PredicateMutability::Mutable,
        allowed_sources: &[
            BeliefSource::SystemOfRecord,
            BeliefSource::UserStated,
            BeliefSource::Tool,
        ],
        ttl: TtlPolicy::StaleScan { reconfirm_days: 90 },
        risk: RiskTier::High,
        description: "subject person currently reports to object person",
    },
    PredicateSpec {
        name: "lives_in",
        cardinality: PredicateCardinality::Single,
        mutability: PredicateMutability::Mutable,
        allowed_sources: &[
            BeliefSource::UserStated,
            BeliefSource::Tool,
            BeliefSource::Inferred,
        ],
        ttl: TtlPolicy::StaleScan { reconfirm_days: 90 },
        risk: RiskTier::Low,
        description: "subject person currently lives in object location",
    },
    PredicateSpec {
        name: "prefers",
        cardinality: PredicateCardinality::Multi,
        mutability: PredicateMutability::Mutable,
        allowed_sources: &[
            BeliefSource::UserStated,
            BeliefSource::Tool,
            BeliefSource::Inferred,
        ],
        ttl: TtlPolicy::StaleScan {
            reconfirm_days: 365,
        },
        risk: RiskTier::Low,
        description:
            "subject prefers object (durable preference; session-scoped variants are not beliefs)",
    },
    PredicateSpec {
        name: "member_of",
        cardinality: PredicateCardinality::Multi,
        mutability: PredicateMutability::Mutable,
        allowed_sources: &[
            BeliefSource::SystemOfRecord,
            BeliefSource::UserStated,
            BeliefSource::Tool,
        ],
        ttl: TtlPolicy::StaleScan { reconfirm_days: 90 },
        risk: RiskTier::Low,
        description: "subject person is currently a member of object team/org",
    },
    PredicateSpec {
        name: "owner_of",
        cardinality: PredicateCardinality::Multi,
        mutability: PredicateMutability::Mutable,
        // Authorization class: ONLY a system of record may assert ownership.
        // "I own account X" from a user or a web page must not grant it.
        allowed_sources: &[BeliefSource::SystemOfRecord],
        ttl: TtlPolicy::SorDriven,
        risk: RiskTier::High,
        description: "subject person currently owns object account/project (authorization)",
    },
    PredicateSpec {
        name: "project_status",
        cardinality: PredicateCardinality::Single,
        mutability: PredicateMutability::Mutable,
        allowed_sources: &[
            BeliefSource::SystemOfRecord,
            BeliefSource::UserStated,
            BeliefSource::Tool,
        ],
        ttl: TtlPolicy::StaleScan { reconfirm_days: 30 },
        risk: RiskTier::Medium,
        description: "subject project currently has object status",
    },
    PredicateSpec {
        name: "promised",
        cardinality: PredicateCardinality::Multi,
        mutability: PredicateMutability::TimeBounded,
        allowed_sources: &[BeliefSource::UserStated, BeliefSource::Tool],
        ttl: TtlPolicy::ExpiresAtDueDate,
        risk: RiskTier::Medium,
        description: "subject committed to object; true until the stated due date",
    },
    PredicateSpec {
        name: "budget_owner",
        cardinality: PredicateCardinality::Single,
        mutability: PredicateMutability::Mutable,
        allowed_sources: &[BeliefSource::SystemOfRecord],
        ttl: TtlPolicy::SorDriven,
        risk: RiskTier::High,
        description: "subject budget is currently owned by object person (finance SoR)",
    },
    PredicateSpec {
        name: "contract_number",
        cardinality: PredicateCardinality::Single,
        mutability: PredicateMutability::Immutable,
        allowed_sources: &[BeliefSource::SystemOfRecord, BeliefSource::Tool],
        ttl: TtlPolicy::NoTtl,
        risk: RiskTier::Medium,
        description: "subject agreement is identified by object contract number",
    },
];

/// Look up a predicate spec by name (the shape the extractor and gate use).
pub fn find_predicate(name: &str) -> Option<&'static PredicateSpec> {
    PREDICATE_CATALOG.iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // --- Source enum ------------------------------------------------------- //

    #[test]
    fn belief_source_parse_accepts_all_valid_values() {
        for v in BeliefSource::ALL {
            assert_eq!(BeliefSource::parse(v).unwrap().as_str(), *v);
        }
    }

    #[test]
    fn belief_source_rejects_invalid_and_is_exact_match() {
        assert!(BeliefSource::parse("User_Stated").is_err());
        assert!(BeliefSource::parse(" system_of_record ").is_err());
        assert!(BeliefSource::parse("").is_err());
        let err = BeliefSource::parse("crm").unwrap_err();
        assert!(err.contains("crm"), "must echo the bad value: {err}");
        for v in BeliefSource::ALL {
            assert!(err.contains(v), "must list valid value '{v}': {err}");
        }
    }

    #[test]
    fn belief_source_precedence_order_matches_trust_order() {
        // system_of_record > user_stated > tool > inferred > web, and the
        // baseline trust must be monotonically non-increasing along that order.
        let ordered = [
            BeliefSource::SystemOfRecord,
            BeliefSource::UserStated,
            BeliefSource::Tool,
            BeliefSource::Inferred,
            BeliefSource::Web,
        ];
        for pair in ordered.windows(2) {
            assert!(
                pair[0].precedence_rank() < pair[1].precedence_rank(),
                "{:?} must outrank {:?}",
                pair[0],
                pair[1]
            );
            assert!(
                pair[0].base_trust() > pair[1].base_trust(),
                "{:?} must be more trusted than {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    // --- Status state machine ---------------------------------------------- //

    #[test]
    fn belief_status_parse_accepts_all_valid_values() {
        for v in BeliefStatus::ALL {
            assert_eq!(BeliefStatus::parse(v).unwrap().as_str(), *v);
        }
        assert!(BeliefStatus::parse("ACTIVE").is_err());
        assert!(BeliefStatus::parse("needs-confirm").is_err());
    }

    #[test]
    fn no_state_transitions_to_itself() {
        for from in all_statuses() {
            for to in from.allowed_transitions_from() {
                assert_ne!(from, *to, "{from:?} -> itself is not a transition");
            }
        }
    }

    #[test]
    fn archived_and_rejected_are_the_only_terminal_states() {
        for s in all_statuses() {
            assert_eq!(
                s.is_terminal(),
                matches!(s, BeliefStatus::Archived | BeliefStatus::Rejected),
                "{s:?} terminal flag wrong"
            );
        }
    }

    #[test]
    fn every_non_terminal_state_can_eventually_reach_a_terminal_state() {
        // Walks the transition graph from every state and asserts it can never
        // loop forever: a belief must always be able to leave the system.
        for start in all_statuses() {
            let mut reachable = vec![start];
            let mut i = 0;
            while i < reachable.len() {
                let current = reachable[i];
                if current.is_terminal() {
                    i += 1;
                    continue;
                }
                for next in current.allowed_transitions_from() {
                    if !reachable.contains(next) {
                        reachable.push(*next);
                    }
                }
                i += 1;
            }
            assert!(
                reachable.iter().any(|s| s.is_terminal()),
                "no terminal state reachable from {start:?}"
            );
        }
    }

    #[test]
    fn only_active_is_retrievable() {
        for s in all_statuses() {
            assert_eq!(
                s.is_retrievable(),
                s == BeliefStatus::Active,
                "{s:?} retrievable flag wrong: quarantined/stale/superseded/\
                 archived must never enter default working-memory assembly"
            );
        }
    }

    #[test]
    fn supersede_and_stale_paths_exist_from_active() {
        assert!(BeliefStatus::Active.can_transition_to(BeliefStatus::Superseded));
        assert!(BeliefStatus::Active.can_transition_to(BeliefStatus::Stale));
        // The replaced edge must never resurrect itself as active.
        assert!(!BeliefStatus::Superseded.can_transition_to(BeliefStatus::Active));
        // Stale may be reconfirmed (SoR re-vouches) but not silently.
        assert!(BeliefStatus::Stale.can_transition_to(BeliefStatus::Active));
    }

    #[test]
    fn quarantined_can_only_leave_via_review() {
        // The two exits from quarantine are review outcomes; a quarantined
        // belief must never reach active in one hop.
        assert!(!BeliefStatus::Quarantined.can_transition_to(BeliefStatus::Active));
        assert!(BeliefStatus::Quarantined.can_transition_to(BeliefStatus::Rejected));
        assert!(BeliefStatus::Quarantined.can_transition_to(BeliefStatus::Candidate));
    }

    // --- Write decision ---------------------------------------------------- //

    #[test]
    fn write_decision_round_trips() {
        for v in WriteDecision::ALL {
            assert_eq!(WriteDecision::parse(v).unwrap().as_str(), *v);
        }
        assert!(WriteDecision::parse("update").is_err());
        assert!(!WriteDecision::Noop.writes());
        assert!(WriteDecision::Add.writes());
        assert!(WriteDecision::Supersede.writes());
        assert!(WriteDecision::Conflict.writes());
    }

    // --- Predicate catalog ------------------------------------------------- //

    fn all_statuses() -> Vec<BeliefStatus> {
        BeliefStatus::ALL
            .iter()
            .map(|v| BeliefStatus::parse(v).unwrap())
            .collect()
    }

    #[test]
    fn catalog_has_batch_one_size_and_unique_names() {
        assert!(
            (8..=10).contains(&PREDICATE_CATALOG.len()),
            "issue #125 asks for a first batch of 8-10 predicates, got {}",
            PREDICATE_CATALOG.len()
        );
        let names: HashSet<_> = PREDICATE_CATALOG.iter().map(|p| p.name).collect();
        assert_eq!(names.len(), PREDICATE_CATALOG.len(), "duplicate names");
    }

    #[test]
    fn catalog_contains_the_required_predicates() {
        for required in [
            "works_at",
            "reports_to",
            "prefers",
            "owner_of",
            "project_status",
            "promised",
        ] {
            assert!(
                find_predicate(required).is_some(),
                "required predicate '{required}' missing from catalog"
            );
        }
    }

    #[test]
    fn every_predicate_defines_all_policy_axes() {
        // The #125 acceptance criterion: cardinality, mutability, source
        // policy, TTL/stale policy and risk must ALL be defined per predicate.
        for p in PREDICATE_CATALOG {
            assert!(
                !p.allowed_sources.is_empty(),
                "{} must declare a source policy",
                p.name
            );
            assert!(
                !p.description.is_empty(),
                "{} must carry a one-line semantics description",
                p.name
            );
            match p.ttl {
                TtlPolicy::StaleScan { reconfirm_days } => {
                    assert!(reconfirm_days > 0, "{} has a zero-day stale window", p.name);
                }
                TtlPolicy::NoTtl | TtlPolicy::SorDriven | TtlPolicy::ExpiresAtDueDate => {}
            }
        }
    }

    #[test]
    fn immutable_predicates_are_never_time_scanned() {
        // An immutable fact ("contract_number", birthplace) has no time-based
        // truth: a stale scan would be a category error, so the two policies
        // must not combine.
        for p in PREDICATE_CATALOG {
            if p.mutability == PredicateMutability::Immutable {
                assert_ne!(
                    p.ttl,
                    TtlPolicy::StaleScan { reconfirm_days: 90 },
                    "{} is immutable but time-scanned",
                    p.name
                );
                assert!(
                    matches!(p.ttl, TtlPolicy::NoTtl),
                    "immutable predicate {} must use NoTtl",
                    p.name
                );
            }
        }
    }

    #[test]
    fn time_bounded_predicates_expire_at_due_date() {
        for p in PREDICATE_CATALOG {
            match p.mutability {
                PredicateMutability::TimeBounded => {
                    assert_eq!(p.ttl, TtlPolicy::ExpiresAtDueDate, "{}", p.name);
                }
                PredicateMutability::Mutable | PredicateMutability::Immutable => {
                    assert_ne!(
                        p.ttl,
                        TtlPolicy::ExpiresAtDueDate,
                        "{} without TimeBounded mutability must not expire at a due date",
                        p.name
                    );
                }
            }
        }
    }

    #[test]
    fn web_is_not_an_allowed_source_for_any_batch_one_predicate() {
        // Conservative batch-one default (Epic #124): web observations are
        // quarantined on arrival and never assert a governed predicate. When a
        // predicate later needs web input it must be argued in the ADR first.
        for p in PREDICATE_CATALOG {
            assert!(
                !p.allows_source(BeliefSource::Web),
                "{} must not accept web assertions in batch one",
                p.name
            );
        }
    }

    #[test]
    fn authorization_predicates_are_high_risk_and_sor_only() {
        for name in ["owner_of", "budget_owner", "reports_to"] {
            let p = find_predicate(name).expect(name);
            assert_eq!(p.risk, RiskTier::High, "{name} must be high risk");
            assert!(
                p.confirmation_required_from(BeliefSource::UserStated),
                "{name} from a user statement must need confirmation"
            );
        }
        // Only system_of_record bypasses confirmation for high-risk beliefs.
        for p in PREDICATE_CATALOG {
            if p.risk == RiskTier::High {
                assert!(!p.confirmation_required_from(BeliefSource::SystemOfRecord));
            }
        }
        // Ownership and budget control are SoR-exclusive: not even a user
        // statement may assert them.
        for name in ["owner_of", "budget_owner"] {
            let p = find_predicate(name).expect(name);
            assert!(
                !p.allows_source(BeliefSource::UserStated),
                "{name} must be system-of-record exclusive"
            );
            assert_eq!(p.ttl, TtlPolicy::SorDriven);
        }
    }

    #[test]
    fn source_policy_uses_the_right_precedence_examples() {
        // works_at: HR (SoR) wins over the user's own claim - the canonical
        // "HR 说离职" example from Epic #124.
        let works_at = find_predicate("works_at").unwrap();
        assert!(works_at.allows_source(BeliefSource::SystemOfRecord));
        assert!(works_at.allows_source(BeliefSource::UserStated));
        assert!(
            BeliefSource::SystemOfRecord.precedence_rank()
                < BeliefSource::UserStated.precedence_rank()
        );

        // prefers: a preference is only ever user-shaped or inferred.
        let prefers = find_predicate("prefers").unwrap();
        assert!(prefers.allows_source(BeliefSource::UserStated));
        assert!(prefers.allows_source(BeliefSource::Inferred));
        assert!(!prefers.allows_source(BeliefSource::SystemOfRecord));
    }

    #[test]
    fn low_risk_predicates_from_user_stated_activate_without_confirmation() {
        for name in ["lives_in", "prefers", "member_of"] {
            let p = find_predicate(name).expect(name);
            assert!(
                !p.confirmation_required_from(BeliefSource::UserStated),
                "{name} from user_stated must not need confirmation"
            );
        }
        // Medium risk still gates weak sources.
        let project_status = find_predicate("project_status").unwrap();
        assert!(project_status.confirmation_required_from(BeliefSource::Inferred));
        assert!(!project_status.confirmation_required_from(BeliefSource::Tool));
    }
}
