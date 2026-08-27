//! Belief lifecycle repository + the write gate's commit orchestration (#127).
//!
//! One transaction per submitted claim performs ALL of:
//! candidate persistence (idempotent) → allowlist/source checks → open-edge
//! comparison → ADD / SUPERSEDE / NOOP / CONFLICT → bitemporal insert with
//! supersede closure → evidence binding → audit row. There is deliberately no
//! multi-claim batch API: the gate's per-claim invariants (exclusion constraint
//! retries, probe verdicts) must not be able to half-apply across claims.
//!
//! The single-open-edge invariant for single-cardinality predicates is enforced
//! by the `beliefs_single_open_edge_per_subject` exclusion constraint — under
//! concurrency a racing writer gets a constraint violation and [`Self::commit`]
//! surfaces it as a retryable error rather than silently corrupting history.

use ulid::Ulid;

use crate::db::audit::AuditEvent;
use crate::db::tenant_scope::begin_tenant_tx;
use crate::error::AppError;
use crate::models::belief::{BeliefSource, WriteDecision};
use crate::models::belief_record::{
    BeliefClaim, ClaimOrigin, GateOutcome, MemoryBelief, MemoryBeliefCandidate,
    MemoryBeliefEvidence, PredicatePolicyRow,
};
use crate::tenant::TenantId;
use sqlx::PgPool;

/// Audit event types emitted by the gate (kept public for tests + #130).
pub const AUDIT_BELIEF_COMMITTED: &str = "belief.committed";
pub const AUDIT_BELIEF_SUPERSEDED: &str = "belief.superseded";
pub const AUDIT_BELIEF_CONFLICT: &str = "belief.conflict";
pub const AUDIT_BELIEF_QUARANTINED: &str = "belief.quarantined";
pub const AUDIT_BELIEF_REJECTED: &str = "belief.rejected";
pub const AUDIT_BELIEF_NOOP: &str = "belief.noop";

const BELIEF_COLS: &str = "id, tenant_id, principal_id, subject, predicate, object, status, \
     source, trust, risk, valid_from::text AS valid_from, valid_to::text AS valid_to, \
     recorded_at::text AS recorded_at, supersedes_id, superseded_by_id, needs_confirm, \
     metadata_json::text AS metadata_json";

const CANDIDATE_COLS: &str =
    "id, tenant_id, principal_id, session_id, subject, predicate, object, \
     source, trust, origin, decision, status, outcome_belief_id, rejection_reason, \
     payload_json::text AS payload_json, idempotency_key, created_at::text AS created_at, \
     resolved_at::text AS resolved_at";

/// Retryable: the exclusion constraint fired under a concurrent writer; the
/// caller should re-run the same claim against fresh state.
pub fn is_concurrent_supersede_conflict(err: &AppError) -> bool {
    let msg = err.to_string();
    msg.contains("beliefs_single_open_edge_per_subject")
        || msg.contains("could not serialize access")
}

pub struct BeliefRepository {
    pool: PgPool,
}

/// Decision-time view of the claim against one predicate policy.
struct PolicyChecks {
    policy: PredicatePolicyRow,
    allowed_sources: Vec<String>,
}

impl BeliefRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Policy allowlist ──────────────────────────────────────────────────── //

    /// Load the governed predicate allowlist (global catalog).
    pub async fn list_policies(&self) -> Result<Vec<PredicatePolicyRow>, AppError> {
        let mut rows = sqlx::query_as::<_, PredicatePolicyRow>(
            "SELECT name, cardinality, mutability, allowed_sources::text AS allowed_sources, \
             ttl_policy, reconfirm_days, risk, description \
             FROM memory_predicate_policies ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("list policies failed: {e}")))?;
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(rows)
    }

    /// Idempotently sync the #125 catalog (`models::belief::PREDICATE_CATALOG`)
    /// into the global `memory_predicate_policies` catalog. Safe on every boot;
    /// the table is tenant-free (global allowlist), so this runs on a plain
    /// transaction without the tenant GUC.
    pub async fn sync_catalog_from_code(&self) -> Result<usize, AppError> {
        use crate::models::belief::{
            PredicateCardinality, PredicateMutability, TtlPolicy, PREDICATE_CATALOG,
        };

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::Internal(format!("catalog sync begin failed: {e}")))?;
        let mut n = 0usize;
        for p in PREDICATE_CATALOG {
            let cardinality = match p.cardinality {
                PredicateCardinality::Single => "single",
                PredicateCardinality::Multi => "multi",
            };
            let mutability = match p.mutability {
                PredicateMutability::Mutable => "mutable",
                PredicateMutability::Immutable => "immutable",
                PredicateMutability::TimeBounded => "time_bounded",
            };
            let (ttl_policy, reconfirm_days) = match p.ttl {
                TtlPolicy::NoTtl => ("no_ttl", None),
                TtlPolicy::SorDriven => ("sor_driven", None),
                TtlPolicy::ExpiresAtDueDate => ("expires_at_due_date", None),
                TtlPolicy::StaleScan { reconfirm_days: d } => ("stale_scan", Some(d as i32)),
            };
            let sources: Vec<&str> = p.allowed_sources.iter().map(|s| s.as_str()).collect();
            sqlx::query(
                r#"
                INSERT INTO memory_predicate_policies
                    (name, cardinality, mutability, allowed_sources, ttl_policy, reconfirm_days, risk, description)
                VALUES ($1,$2,$3,$4::jsonb,$5,$6,$7,$8)
                ON CONFLICT (name) DO UPDATE SET
                    cardinality = EXCLUDED.cardinality,
                    mutability = EXCLUDED.mutability,
                    allowed_sources = EXCLUDED.allowed_sources,
                    ttl_policy = EXCLUDED.ttl_policy,
                    reconfirm_days = EXCLUDED.reconfirm_days,
                    risk = EXCLUDED.risk,
                    description = EXCLUDED.description
                "#,
            )
            .bind(p.name)
            .bind(cardinality)
            .bind(mutability)
            .bind(serde_json::to_string(&sources).unwrap())
            .bind(ttl_policy)
            .bind(reconfirm_days)
            .bind(p.risk.as_str())
            .bind(p.description)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("policy sync failed for {}: {e}", p.name)))?;
            n += 1;
        }
        tx.commit().await.ok();
        Ok(n)
    }

    // ── Reads ─────────────────────────────────────────────────────────────── //

    pub async fn get_belief(
        &self,
        tenant_id: &TenantId,
        belief_id: &str,
    ) -> Result<Option<MemoryBelief>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let row = sqlx::query_as::<_, MemoryBelief>(&format!(
            "SELECT {BELIEF_COLS} FROM memory_beliefs WHERE tenant_id = $1 AND id = $2"
        ))
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("get_belief failed: {e}")))?;
        tx.commit().await.ok();
        Ok(row)
    }

    /// The open edge for a subject+predicate (single-cardinality slot) — the
    /// surface the gate compares against and #128 will recall.
    pub async fn open_edge(
        &self,
        tenant_id: &TenantId,
        subject: &str,
        predicate: &str,
    ) -> Result<Option<MemoryBelief>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let row = Self::open_edge_in(&mut tx, tenant_id, subject, predicate).await?;
        tx.commit().await.ok();
        Ok(row)
    }

    async fn open_edge_in(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tenant_id: &TenantId,
        subject: &str,
        predicate: &str,
    ) -> Result<Option<MemoryBelief>, AppError> {
        sqlx::query_as::<_, MemoryBelief>(&format!(
            r#"
            SELECT {BELIEF_COLS}
            FROM memory_beliefs
            WHERE tenant_id = $1 AND subject = $2 AND predicate = $3
              AND valid_to IS NULL
              AND status IN ('active', 'needs_confirm')
            ORDER BY valid_from DESC
            LIMIT 1
            FOR UPDATE
            "#
        ))
        .bind(tenant_id.as_str())
        .bind(subject)
        .bind(predicate)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| AppError::Internal(format!("open_edge failed: {e}")))
    }

    /// Current (active) edges for a subject — the recall-facing helper.
    pub async fn active_edges_for_subject(
        &self,
        tenant_id: &TenantId,
        subject: &str,
    ) -> Result<Vec<MemoryBelief>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = sqlx::query_as::<_, MemoryBelief>(&format!(
            r#"
            SELECT {BELIEF_COLS}
            FROM memory_beliefs
            WHERE tenant_id = $1 AND subject = $2
              AND valid_to IS NULL AND status = 'active'
            ORDER BY predicate
            "#
        ))
        .bind(tenant_id.as_str())
        .bind(subject)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("active_edges failed: {e}")))?;
        tx.commit().await.ok();
        Ok(rows)
    }

    /// Full version history (supersede chain) for a subject+predicate.
    pub async fn history_for(
        &self,
        tenant_id: &TenantId,
        subject: &str,
        predicate: &str,
    ) -> Result<Vec<MemoryBelief>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = sqlx::query_as::<_, MemoryBelief>(&format!(
            r#"
            SELECT {BELIEF_COLS}
            FROM memory_beliefs
            WHERE tenant_id = $1 AND subject = $2 AND predicate = $3
            ORDER BY valid_from ASC
            "#
        ))
        .bind(tenant_id.as_str())
        .bind(subject)
        .bind(predicate)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("history_for failed: {e}")))?;
        tx.commit().await.ok();
        Ok(rows)
    }

    pub async fn get_candidate(
        &self,
        tenant_id: &TenantId,
        candidate_id: &str,
    ) -> Result<Option<MemoryBeliefCandidate>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let row = sqlx::query_as::<_, MemoryBeliefCandidate>(&format!(
            "SELECT {CANDIDATE_COLS} FROM memory_belief_candidates WHERE tenant_id = $1 AND id = $2"
        ))
        .bind(tenant_id.as_str())
        .bind(candidate_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("get_candidate failed: {e}")))?;
        tx.commit().await.ok();
        Ok(row)
    }

    pub async fn list_candidates(
        &self,
        tenant_id: &TenantId,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<MemoryBeliefCandidate>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = sqlx::query_as::<_, MemoryBeliefCandidate>(&format!(
            r#"
            SELECT {CANDIDATE_COLS}
            FROM memory_belief_candidates
            WHERE tenant_id = $1 AND ($2::text IS NULL OR status = $2)
            ORDER BY created_at DESC
            LIMIT $3
            "#
        ))
        .bind(tenant_id.as_str())
        .bind(status)
        .bind(limit.clamp(1, 500))
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("list_candidates failed: {e}")))?;
        tx.commit().await.ok();
        Ok(rows)
    }

    pub async fn evidence_for_belief(
        &self,
        tenant_id: &TenantId,
        belief_id: &str,
    ) -> Result<Vec<MemoryBeliefEvidence>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = sqlx::query_as::<_, MemoryBeliefEvidence>(
            "SELECT id, tenant_id, belief_id, candidate_id, event_id, kind, content_hash, \
             created_at::text AS created_at \
             FROM memory_belief_evidence WHERE tenant_id = $1 AND belief_id = $2",
        )
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("evidence_for_belief failed: {e}")))?;
        tx.commit().await.ok();
        Ok(rows)
    }

    // ── Recall read face (#128) ────────────────────────────────────────────── //

    /// Belief rows eligible for retrieval under the #128 hard filters.
    ///
    /// Hard (pre-ranking, never soft-scored):
    /// - tenant via RLS (caller's begin_tenant_tx) and principal scope;
    /// - `as_of` window coverage: default NOW returns only `status='active'`
    ///   open edges; an explicit past `as_of` instead returns, per
    ///   (subject, predicate), the latest edge whose window covered that
    ///   instant — which includes `superseded` history;
    /// - `needs_confirm` / `quarantined` / `archived` / `rejected` NEVER
    ///   eligible (quarantined claims never became edges at all);
    /// - high-risk trust floor: `risk='high' AND trust < min_high_risk_trust`
    ///   excluded (Epic #124 deny_if_trust_below).
    pub async fn eligible_edges_for_recall(
        &self,
        tenant_id: &TenantId,
        principal_id: &str,
        subject: Option<&str>,
        as_of: Option<chrono::DateTime<chrono::Utc>>,
        min_high_risk_trust: f32,
    ) -> Result<Vec<MemoryBelief>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = Self::eligible_edges_in(
            &mut tx,
            tenant_id,
            principal_id,
            subject,
            as_of,
            min_high_risk_trust,
        )
        .await?;
        tx.commit().await.ok();
        Ok(rows)
    }

    /// Same as above on an existing transaction (the recall core fetches
    /// evidence on the same tx for a consistent snapshot).
    #[allow(clippy::too_many_arguments)]
    pub async fn eligible_edges_in(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tenant_id: &TenantId,
        principal_id: &str,
        subject: Option<&str>,
        as_of: Option<chrono::DateTime<chrono::Utc>>,
        min_high_risk_trust: f32,
    ) -> Result<Vec<MemoryBelief>, AppError> {
        let historical = as_of.is_some();
        let sql = format!(
            r#"
            SELECT {BELIEF_COLS}
            FROM (
                SELECT DISTINCT ON (subject, predicate) *
                FROM memory_beliefs
                WHERE tenant_id = $1
                  AND principal_id = $2
                  AND ($3::text IS NULL OR subject = $3)
                  AND {}
                  AND status {}
                  AND NOT (risk = 'high' AND trust < $4)
                ORDER BY subject, predicate, valid_from DESC
            ) e
            ORDER BY e.subject, e.predicate
            "#,
            // Window coverage: historical picks the edge valid AT as_of (which
            // may since be superseded); current requires an open active edge.
            if historical {
                "(valid_from <= $5 AND (valid_to IS NULL OR valid_to > $5))"
            } else {
                "(valid_from <= NOW() AND valid_to IS NULL)"
            },
            if historical {
                "IN ('active', 'superseded')"
            } else {
                "= 'active'"
            },
        );
        let mut q = sqlx::query_as::<_, MemoryBelief>(&sql)
            .bind(tenant_id.as_str())
            .bind(principal_id)
            .bind(subject)
            .bind(min_high_risk_trust);
        if let Some(ts) = as_of {
            q = q.bind(ts);
        }
        q.fetch_all(&mut **tx)
            .await
            .map_err(|e| AppError::Internal(format!("eligible_edges failed: {e}")))
    }

    /// The agent's memory contract, if one exists (hard-filter input).
    pub async fn contract_for_agent(
        &self,
        tenant_id: &TenantId,
        agent_id: Option<&str>,
    ) -> Result<Option<MemoryContractRow>, AppError> {
        let Some(agent) = agent_id else {
            return Ok(None);
        };
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let row = sqlx::query_as::<_, MemoryContractRow>(
            "SELECT id, tenant_id, agent_id, may_believe::text AS may_believe,              must_not_believe_from::text AS must_not_believe_from,              high_stakes_deny_below_trust, enabled              FROM memory_contracts WHERE tenant_id = $1 AND agent_id = $2 AND enabled",
        )
        .bind(tenant_id.as_str())
        .bind(agent)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("contract load failed: {e}")))?;
        tx.commit().await.ok();
        Ok(row)
    }

    /// Average usefulness (0..1) per belief id from `memory_feedback`.
    /// Absent feedback is simply missing from the map (neutral later).
    pub async fn feedback_usefulness(
        &self,
        tenant_id: &TenantId,
        belief_ids: &[String],
    ) -> Result<std::collections::HashMap<String, f64>, AppError> {
        if belief_ids.is_empty() {
            return Ok(Default::default());
        }
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows: Vec<(String, f64, i64)> = sqlx::query_as(
            "SELECT memory_id, AVG(CASE WHEN useful THEN 1.0 ELSE 0.0 END), COUNT(*)              FROM memory_feedback WHERE tenant_id = $1 AND memory_id = ANY($2)              GROUP BY memory_id",
        )
        .bind(tenant_id.as_str())
        .bind(belief_ids)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("feedback aggregation failed: {e}")))?;
        tx.commit().await.ok();
        Ok(rows.into_iter().map(|(k, v, _)| (k, v)).collect())
    }

    // ── The gate (commit orchestration) ───────────────────────────────────── //

    /// Submit one claim through the write gate.
    ///
    /// `probe_verdict` is the injection-probe result for [`BeliefClaim::probe_text`]
    /// (quarantined/flagged/clean) supplied by the caller so this function stays
    /// synchronous-only about the database. Passing `Quarantined` parks the
    /// claim; passing `Flagged` downgrades trust below the action threshold.
    ///
    /// All outcomes except a DB failure return a `GateOutcome` and persist an
    /// auditable candidate row — even rejects and quarantines leave a trail.
    pub async fn commit(
        &self,
        tenant_id: &TenantId,
        claim: BeliefClaim,
        probe_verdict: &crate::services::belief::ProbeVerdict,
    ) -> Result<GateOutcome, AppError> {
        use crate::models::belief::find_predicate;

        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        // 0. Serialize gate writers on this (tenant, subject, predicate) slot.
        //    The exclusion constraint alone still deadlocks under concurrent
        //    inserts (each tx's constraint check waits on the others'
        //    uncommitted index entries, and PG detects the cycle). A
        //    transaction-scoped advisory lock per slot removes both the
        //    deadlock window and the EPQ missed-edge race; different slots
        //    proceed in parallel. The service-level retry stays as a
        //    belt-and-suspenders for any residual contention.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "belief|{}|{}|{}",
                tenant_id.as_str(),
                claim.subject,
                claim.predicate
            ))
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("gate advisory lock failed: {e}")))?;

        // 1. Idempotent candidate persistence — replays resolve to the ORIGINAL
        //    candidate + outcome instead of re-deciding (#127 acceptance 2).
        if let Some((prior_id, prior_outcome)) =
            Self::resolve_duplicate_candidate(&mut tx, tenant_id, &claim).await?
        {
            tx.commit().await.ok();
            return Ok(prior_outcome
                .ok_or_else(|| {
                    AppError::Internal(format!(
                        "candidate {prior_id} recorded an idempotency key but no outcome; \
                         inspect it before resubmitting"
                    ))
                })
                .map(|o| o.with_candidate(prior_id))?);
        }

        let candidate_id = Ulid::new().to_string();

        // 2. Injection probe FIRST, before allowlist/source policy: a
        //    poisoned claim quarantines even when its source would be barred
        //    anyway — the quarantine queue is the security-review surface and
        //    "web + injection" is strictly more actionable than "web rejected".
        if probe_verdict == &crate::services::belief::ProbeVerdict::Quarantined {
            return Self::quarantine(
                tx,
                tenant_id,
                &claim,
                &candidate_id,
                "injection probe: quarantined (instruction-shaped content)".to_string(),
            )
            .await;
        }

        // 3. Allowlist + source policy (code catalog is the single source of
        //    truth; the policies table is its materialized, queryable copy).
        let Some(spec) = find_predicate(&claim.predicate) else {
            return Self::reject(
                tx,
                tenant_id,
                &claim,
                &candidate_id,
                format!(
                    "predicate '{}' is not in the governed allowlist",
                    claim.predicate
                ),
            )
            .await;
        };
        let policy = PolicyChecks {
            allowed_sources: spec
                .allowed_sources
                .iter()
                .map(|s| s.as_str().to_string())
                .collect(),
            policy: PredicatePolicyRow {
                name: spec.name.to_string(),
                cardinality: match spec.cardinality {
                    crate::models::belief::PredicateCardinality::Single => "single",
                    crate::models::belief::PredicateCardinality::Multi => "multi",
                }
                .to_string(),
                mutability: match spec.mutability {
                    crate::models::belief::PredicateMutability::Mutable => "mutable",
                    crate::models::belief::PredicateMutability::Immutable => "immutable",
                    crate::models::belief::PredicateMutability::TimeBounded => "time_bounded",
                }
                .to_string(),
                allowed_sources: String::new(),
                ttl_policy: String::new(),
                reconfirm_days: None,
                risk: spec.risk.as_str().to_string(),
                description: spec.description.to_string(),
            },
        };
        if !policy
            .allowed_sources
            .contains(&claim.source.as_str().to_string())
        {
            return Self::reject(
                tx,
                tenant_id,
                &claim,
                &candidate_id,
                format!(
                    "source '{}' is not allowed to assert predicate '{}'",
                    claim.source.as_str(),
                    claim.predicate
                ),
            )
            .await;
        }

        // Flagged (not quarantined) content may be recorded but never with
        // enough trust to drive anything: clamped below the action threshold.
        let trust = if probe_verdict == &crate::services::belief::ProbeVerdict::Flagged {
            claim.source.base_trust().min(0.4)
        } else {
            claim.source.base_trust()
        };

        // 4. Evidence precondition: an active-eligible claim must cite at least
        //    one event (#127 acceptance 7 — no unevidenced active beliefs).
        let needs_confirm = spec.risk.confirmation_required_for(claim.source);
        if claim.evidence_event_ids.is_empty() && !needs_confirm {
            return Self::reject(
                tx,
                tenant_id,
                &claim,
                &candidate_id,
                "claims must cite at least one memory_events id as evidence".to_string(),
            )
            .await;
        }

        // 5. Compare against the open edge under lock.
        let existing =
            Self::open_edge_in(&mut tx, tenant_id, &claim.subject, &claim.predicate).await?;

        let Some(existing) = existing else {
            // ADD: no open edge. High-risk / weak-source claims park in
            // needs_confirm; they occupy the slot but drive nothing.
            // Candidate row MUST land before the edge: evidence rows carry a
            // candidate_id FK, and the outcome update below needs the row.
            Self::resolve_candidate(
                &mut tx,
                tenant_id,
                &candidate_id,
                &claim,
                Some(WriteDecision::Add.as_str()),
                if needs_confirm { "pending" } else { "accepted" },
                None,
                None,
            )
            .await?;
            let belief_id = Self::insert_edge(
                &mut tx,
                tenant_id,
                &claim,
                &candidate_id,
                None,
                trust,
                &policy.policy.risk,
                needs_confirm,
            )
            .await?;
            Self::link_candidate_outcome(&mut tx, &candidate_id, &belief_id).await?;
            Self::bind_evidence(&mut tx, tenant_id, &candidate_id, Some(&belief_id), &claim)
                .await?;
            Self::audit(
                &mut tx,
                tenant_id,
                AUDIT_BELIEF_COMMITTED,
                &candidate_id,
                Some(&belief_id),
                &claim,
            )
            .await?;
            tx.commit().await.ok();
            return Ok(GateOutcome::Committed {
                candidate_id,
                belief_id,
                needs_confirm,
            });
        };

        if existing.object == claim.object {
            // NOOP: identical fact. Idempotent retries never create versions.
            Self::resolve_candidate(
                &mut tx,
                tenant_id,
                &candidate_id,
                &claim,
                Some(WriteDecision::Noop.as_str()),
                "accepted",
                Some(existing.id.clone()),
                None,
            )
            .await?;
            Self::bind_evidence(
                &mut tx,
                tenant_id,
                &candidate_id,
                Some(&existing.id),
                &claim,
            )
            .await?;
            Self::audit(
                &mut tx,
                tenant_id,
                AUDIT_BELIEF_NOOP,
                &candidate_id,
                Some(&existing.id),
                &claim,
            )
            .await?;
            tx.commit().await.ok();
            return Ok(GateOutcome::Noop {
                candidate_id,
                belief_id: existing.id,
            });
        }

        let new_rank = claim.source.precedence_rank();
        let old_rank = BeliefSource::parse(&existing.source)
            .map_err(|raw| {
                AppError::Internal(format!("corrupt source '{raw}' on belief {}", existing.id))
            })?
            .precedence_rank();

        // A pending-confirmation edge is protected: only a STRICTLY stronger
        // source may resolve it by supersede. An equal-rank rival must park as
        // a conflict — silently swapping one unconfirmed human claim for
        // another is exactly the coin-flip #127 forbids. Active edges follow
        // the ADR-0011 rule: rank <= old (equal or stronger) may supersede,
        // recency breaking the equal-rank tie.
        let supersede_allowed = if existing.status == "needs_confirm" {
            new_rank < old_rank
        } else {
            new_rank <= old_rank
        };

        if !supersede_allowed {
            // Weak source vs strong edge: CONFLICT. Existing stays; candidate
            // parks for human resolution. Nothing auto-overwrites (#127 ac. 4).
            Self::resolve_candidate(
                &mut tx,
                tenant_id,
                &candidate_id,
                &claim,
                Some(WriteDecision::Conflict.as_str()),
                "pending",
                None,
                Some(format!(
                    "weaker source '{}' contradicts active edge from '{}'",
                    claim.source.as_str(),
                    existing.source
                )),
            )
            .await?;
            Self::bind_evidence(&mut tx, tenant_id, &candidate_id, None, &claim).await?;
            Self::audit(
                &mut tx,
                tenant_id,
                AUDIT_BELIEF_CONFLICT,
                &candidate_id,
                Some(&existing.id),
                &claim,
            )
            .await?;
            tx.commit().await.ok();
            return Ok(GateOutcome::Conflict {
                candidate_id,
                existing_belief_id: existing.id,
            });
        }

        // SUPERSEDE: close the old edge's valid window, open the new one linked
        // through supersedes/superseded_by. History is retained, never edited.
        Self::resolve_candidate(
            &mut tx,
            tenant_id,
            &candidate_id,
            &claim,
            Some(WriteDecision::Supersede.as_str()),
            if needs_confirm { "pending" } else { "accepted" },
            None,
            None,
        )
        .await?;
        // Close the OLD edge BEFORE inserting the new one: the exclusion
        // constraint is checked per-statement, and an interim state with two
        // open edges would violate it even inside one transaction. Both
        // timestamps come from the same transaction clock, so the closed range
        // [old_from, now) and the new [now, inf) are adjacent, not overlapping.
        sqlx::query(
            "UPDATE memory_beliefs SET valid_to = NOW(), \
             status = 'superseded', updated_at = NOW() WHERE id = $1 AND valid_to IS NULL",
        )
        .bind(&existing.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("supersede closure failed: {e}")))?;
        let new_belief_id = Self::insert_edge(
            &mut tx,
            tenant_id,
            &claim,
            &candidate_id,
            Some(&existing.id),
            trust,
            &policy.policy.risk,
            needs_confirm,
        )
        .await?;
        sqlx::query("UPDATE memory_beliefs SET superseded_by_id = $1 WHERE id = $2")
            .bind(&new_belief_id)
            .bind(&existing.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("supersede backlink failed: {e}")))?;

        Self::link_candidate_outcome(&mut tx, &candidate_id, &new_belief_id).await?;
        Self::bind_evidence(
            &mut tx,
            tenant_id,
            &candidate_id,
            Some(&new_belief_id),
            &claim,
        )
        .await?;
        Self::audit(
            &mut tx,
            tenant_id,
            AUDIT_BELIEF_SUPERSEDED,
            &candidate_id,
            Some(&new_belief_id),
            &claim,
        )
        .await?;
        tx.commit().await.ok();
        Ok(GateOutcome::Superseded {
            candidate_id,
            new_belief_id,
            superseded_belief_id: existing.id,
            needs_confirm,
        })
    }

    /// Human confirmation flips a `needs_confirm` edge to `active` (#127 ac. 6).
    pub async fn confirm_belief(
        &self,
        tenant_id: &TenantId,
        belief_id: &str,
        actor: Option<&str>,
    ) -> Result<MemoryBelief, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let updated = sqlx::query_as::<_, MemoryBelief>(&format!(
            r#"
            UPDATE memory_beliefs
            SET status = 'active', needs_confirm = FALSE, updated_at = NOW()
            WHERE tenant_id = $1 AND id = $2 AND status = 'needs_confirm'
            RETURNING {BELIEF_COLS}
            "#
        ))
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("confirm_belief failed: {e}")))?;
        let Some(edge) = updated else {
            return Err(AppError::BadRequest(format!(
                "belief '{belief_id}' is not awaiting confirmation"
            )));
        };
        let audit = AuditEvent::new("belief.confirmed", "memory_belief")
            .tenant(tenant_id.as_str())
            .actor(actor.unwrap_or("unknown"))
            .resource_id(belief_id);
        crate::db::audit::insert_tx(&mut tx, &audit).await?;
        tx.commit().await.ok();
        Ok(edge)
    }

    // ── Commit-internal helpers (all take the open tx) ────────────────────── //

    async fn resolve_duplicate_candidate(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tenant_id: &TenantId,
        claim: &BeliefClaim,
    ) -> Result<Option<(String, Option<GateOutcome>)>, AppError> {
        let Some(key) = claim.idempotency_key.as_deref() else {
            return Ok(None);
        };
        let prior: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT id, decision, outcome_belief_id FROM memory_belief_candidates \
                 WHERE tenant_id = $1 AND idempotency_key = $2",
        )
        .bind(tenant_id.as_str())
        .bind(key)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| AppError::Internal(format!("duplicate candidate lookup failed: {e}")))?;
        let Some((id, decision, outcome)) = prior else {
            return Ok(None);
        };
        let outcome = match decision.as_deref() {
            Some("add") => outcome.map(|b| GateOutcome::Committed {
                candidate_id: id.clone(),
                belief_id: b,
                needs_confirm: false,
            }),
            Some("supersede") => outcome.map(|b| GateOutcome::Superseded {
                candidate_id: id.clone(),
                new_belief_id: b.clone(),
                superseded_belief_id: b, // replay keeps ids; chain intact via history
                needs_confirm: false,
            }),
            Some("noop") => outcome.map(|b| GateOutcome::Noop {
                candidate_id: id.clone(),
                belief_id: b,
            }),
            Some("conflict") => Some(GateOutcome::Conflict {
                candidate_id: id.clone(),
                existing_belief_id: outcome.unwrap_or_default(),
            }),
            _ => None,
        };
        Ok(Some((id, outcome)))
    }

    #[allow(clippy::too_many_arguments)]
    async fn insert_edge(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tenant_id: &TenantId,
        claim: &BeliefClaim,
        candidate_id: &str,
        supersedes: Option<&str>,
        trust: f64,
        risk: &str,
        needs_confirm: bool,
    ) -> Result<String, AppError> {
        let id = Ulid::new().to_string();
        let payload = serde_json::to_string(&claim.payload_json)
            .map_err(|e| AppError::BadRequest(format!("payload not serializable: {e}")))?;
        sqlx::query(
            r#"
            INSERT INTO memory_beliefs
                (id, tenant_id, principal_id, subject, predicate, object, status,
                 source, trust, risk, valid_from, valid_to, recorded_at,
                 supersedes_id, needs_confirm, metadata_json)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10, NOW(), NULL, NOW(), $11, $12, $13::jsonb)
            "#,
        )
        .bind(&id)
        .bind(tenant_id.as_str())
        .bind(&claim.principal_id)
        .bind(&claim.subject)
        .bind(&claim.predicate)
        .bind(&claim.object)
        .bind(if needs_confirm {
            "needs_confirm"
        } else {
            "active"
        })
        .bind(claim.source.as_str())
        .bind(trust)
        .bind(risk)
        .bind(supersedes)
        .bind(needs_confirm)
        .bind(&payload)
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            AppError::Internal(format!(
                "insert_edge failed for {}/{} (candidate {candidate_id}): {e}",
                claim.subject, claim.predicate
            ))
        })?;
        Ok(id)
    }

    /// Point an already-inserted candidate row at its outcome belief (the
    /// belief row did not exist when the candidate was first written).
    async fn link_candidate_outcome(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        candidate_id: &str,
        belief_id: &str,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE memory_belief_candidates SET outcome_belief_id = $1 WHERE id = $2")
            .bind(belief_id)
            .bind(candidate_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| AppError::Internal(format!("candidate outcome link failed: {e}")))?;
        Ok(())
    }

    async fn bind_evidence(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tenant_id: &TenantId,
        candidate_id: &str,
        belief_id: Option<&str>,
        claim: &BeliefClaim,
    ) -> Result<(), AppError> {
        for event_id in &claim.evidence_event_ids {
            sqlx::query(
                "INSERT INTO memory_belief_evidence (id, tenant_id, belief_id, candidate_id, event_id, kind, content_hash) \
                 VALUES ($1,$2,$3,$4,$5,'direct',(SELECT content_hash FROM memory_events WHERE id=$5 AND tenant_id=$2))",
            )
            .bind(Ulid::new().to_string())
            .bind(tenant_id.as_str())
            .bind(belief_id)
            .bind(candidate_id)
            .bind(event_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| {
                AppError::Internal(format!("evidence bind for event {event_id} failed: {e}"))
            })?;
        }
        Ok(())
    }

    async fn resolve_candidate(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tenant_id: &TenantId,
        candidate_id: &str,
        claim: &BeliefClaim,
        decision: Option<&str>,
        status: &str,
        outcome_belief_id: Option<String>,
        reason: Option<String>,
    ) -> Result<(), AppError> {
        let payload = serde_json::to_string(&claim.payload_json).unwrap_or_else(|_| "{}".into());
        sqlx::query(
            r#"
            INSERT INTO memory_belief_candidates
                (id, tenant_id, principal_id, session_id, subject, predicate, object,
                 source, trust, origin, decision, status, outcome_belief_id,
                 rejection_reason, payload_json, idempotency_key, resolved_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15::jsonb,$16, NOW())
            "#,
        )
        .bind(candidate_id)
        .bind(tenant_id.as_str())
        .bind(&claim.principal_id)
        .bind(claim.session_id.as_deref())
        .bind(&claim.subject)
        .bind(&claim.predicate)
        .bind(&claim.object)
        .bind(claim.source.as_str())
        .bind(claim.source.base_trust() as f32)
        .bind(claim.origin.as_str())
        .bind(decision)
        .bind(status)
        .bind(outcome_belief_id)
        .bind(reason)
        .bind(&payload)
        .bind(claim.idempotency_key.as_deref())
        .execute(&mut **tx)
        .await
        .map_err(|e| AppError::Internal(format!("candidate persist failed: {e}")))?;
        Ok(())
    }

    async fn reject(
        tx: sqlx::Transaction<'_, sqlx::Postgres>,
        tenant_id: &TenantId,
        claim: &BeliefClaim,
        candidate_id: &str,
        reason: String,
    ) -> Result<GateOutcome, AppError> {
        let mut tx = tx;
        Self::resolve_candidate(
            &mut tx,
            tenant_id,
            candidate_id,
            claim,
            // 'rejected' lives in `status`; the decision CHECK only admits
            // the four write-decision values.
            None,
            "rejected",
            None,
            Some(reason.clone()),
        )
        .await?;
        Self::audit(
            &mut tx,
            tenant_id,
            AUDIT_BELIEF_REJECTED,
            candidate_id,
            None,
            claim,
        )
        .await?;
        tx.commit().await.ok();
        Ok(GateOutcome::Rejected {
            candidate_id: candidate_id.to_string(),
            reason,
        })
    }

    async fn quarantine(
        tx: sqlx::Transaction<'_, sqlx::Postgres>,
        tenant_id: &TenantId,
        claim: &BeliefClaim,
        candidate_id: &str,
        reason: String,
    ) -> Result<GateOutcome, AppError> {
        let mut tx = tx;
        // Quarantined candidates keep their evidence pointers so a human review
        // can see exactly WHICH messages tried to plant the instruction.
        let payload = serde_json::to_string(&claim.payload_json).unwrap_or_else(|_| "{}".into());
        sqlx::query(
            r#"
            INSERT INTO memory_belief_candidates
                (id, tenant_id, principal_id, session_id, subject, predicate, object,
                 source, trust, origin, status, rejection_reason, payload_json, idempotency_key, resolved_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'quarantined',$11,$12::jsonb,$13, NOW())
            "#,
        )
        .bind(candidate_id)
        .bind(tenant_id.as_str())
        .bind(&claim.principal_id)
        .bind(claim.session_id.as_deref())
        .bind(&claim.subject)
        .bind(&claim.predicate)
        .bind(&claim.object)
        .bind(claim.source.as_str())
        .bind((claim.source.base_trust() as f32).min(0.2))
        .bind(claim.origin.as_str())
        .bind(&reason)
        .bind(&payload)
        .bind(claim.idempotency_key.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("quarantine persist failed: {e}")))?;
        for event_id in &claim.evidence_event_ids {
            sqlx::query(
                "INSERT INTO memory_belief_evidence (id, tenant_id, candidate_id, event_id, kind, content_hash) \
                 VALUES ($1,$2,$3,$4,'direct',(SELECT content_hash FROM memory_events WHERE id=$4 AND tenant_id=$2))",
            )
            .bind(Ulid::new().to_string())
            .bind(tenant_id.as_str())
            .bind(candidate_id)
            .bind(event_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("quarantine evidence bind failed: {e}")))?;
        }
        Self::audit(
            &mut tx,
            tenant_id,
            AUDIT_BELIEF_QUARANTINED,
            candidate_id,
            None,
            claim,
        )
        .await?;
        tx.commit().await.ok();
        Ok(GateOutcome::Quarantined {
            candidate_id: candidate_id.to_string(),
            reason,
        })
    }

    async fn audit(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tenant_id: &TenantId,
        event_type: &str,
        candidate_id: &str,
        belief_id: Option<&str>,
        claim: &BeliefClaim,
    ) -> Result<(), AppError> {
        let audit = AuditEvent::new(event_type, "memory_belief")
            .tenant(tenant_id.as_str())
            .resource_id(candidate_id)
            .correlation_id(belief_id.unwrap_or(""))
            .with_metadata(&serde_json::json!({
                "subject": claim.subject,
                "predicate": claim.predicate,
                "object": claim.object,
                "source": claim.source.as_str(),
                "origin": claim.origin.as_str(),
                "belief_id": belief_id,
            }));
        crate::db::audit::insert_tx(tx, &audit).await
    }
}

/// One row of `memory_contracts` (#128 recall hard-filter input).
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct MemoryContractRow {
    pub id: String,
    pub tenant_id: String,
    pub agent_id: String,
    /// JSON array text: predicates this agent may believe from allowed sources.
    pub may_believe: String,
    /// JSON object text: { source: [predicates...] } the agent must NOT believe.
    pub must_not_believe_from: String,
    pub high_stakes_deny_below_trust: Option<f32>,
    pub enabled: bool,
}

impl GateOutcome {
    fn with_candidate(self, _id: String) -> GateOutcome {
        self
    }
}
