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
     metadata_json::text AS metadata_json, single_valued, \
     last_confirmed_at::text AS last_confirmed_at";

/// Same columns as [`BELIEF_COLS`], qualified for queries that JOIN the
/// policies table (its `risk` column would otherwise be ambiguous).
const BELIEF_COLS_B: &str = "b.id, b.tenant_id, b.principal_id, b.subject, b.predicate, b.object, \
     b.status, b.source, b.trust, b.risk, b.valid_from::text AS valid_from, \
     b.valid_to::text AS valid_to, b.recorded_at::text AS recorded_at, b.supersedes_id, \
     b.superseded_by_id, b.needs_confirm, b.metadata_json::text AS metadata_json, \
     b.single_valued, b.last_confirmed_at::text AS last_confirmed_at";

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
        let row = Self::open_edge_in(&mut tx, tenant_id, subject, predicate, None).await?;
        tx.commit().await.ok();
        Ok(row)
    }

    /// `object_filter`: for SINGLE-valued predicates pass None (the one open
    /// slot); for MULTI-valued predicates pass the claim's object so the
    /// comparison targets the matching edge — distinct objects coexist.
    async fn open_edge_in(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tenant_id: &TenantId,
        subject: &str,
        predicate: &str,
        object_filter: Option<&str>,
    ) -> Result<Option<MemoryBelief>, AppError> {
        sqlx::query_as::<_, MemoryBelief>(&format!(
            r#"
            SELECT {BELIEF_COLS}
            FROM memory_beliefs
            WHERE tenant_id = $1 AND subject = $2 AND predicate = $3
              AND valid_to IS NULL
              AND status IN ('active', 'needs_confirm')
              AND ($4::text IS NULL OR object = $4)
            ORDER BY valid_from DESC
            LIMIT 1
            FOR UPDATE
            "#
        ))
        .bind(tenant_id.as_str())
        .bind(subject)
        .bind(predicate)
        .bind(object_filter)
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

    // ── Governance face (#130) ─────────────────────────────────────────────── //

    /// Belief listing for the governance surface. `include_history=false`
    /// returns only open edges; true returns the full version history.
    pub async fn list_beliefs(
        &self,
        tenant_id: &TenantId,
        subject: Option<&str>,
        predicate: Option<&str>,
        include_history: bool,
        limit: i64,
    ) -> Result<Vec<MemoryBelief>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = sqlx::query_as::<_, MemoryBelief>(&format!(
            r#"
            SELECT {BELIEF_COLS}
            FROM memory_beliefs
            WHERE tenant_id = $1
              AND ($2::text IS NULL OR subject = $2)
              AND ($3::text IS NULL OR predicate = $3)
              AND ($4 OR (valid_to IS NULL AND status IN ('active', 'needs_confirm')))
            ORDER BY subject, predicate, valid_from DESC
            LIMIT $5
            "#,
        ))
        .bind(tenant_id.as_str())
        .bind(subject)
        .bind(predicate)
        .bind(include_history)
        .bind(limit.clamp(1, 1000))
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("governance list failed: {e}")))?;
        tx.commit().await.ok();
        Ok(rows)
    }

    /// The full traceability surface for one belief: the edge, its evidence
    /// citations, and its audit chain — the "#124 acceptance 5" answer to
    /// "从错误行为定位到 belief、event、provenance".
    pub async fn belief_trace(
        &self,
        tenant_id: &TenantId,
        belief_id: &str,
    ) -> Result<
        Option<(
            MemoryBelief,
            Vec<crate::models::belief_record::MemoryBeliefEvidence>,
            Vec<crate::models::belief_record::AuditTraceRow>,
        )>,
        AppError,
    > {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let Some(belief) = sqlx::query_as::<_, MemoryBelief>(&format!(
            "SELECT {BELIEF_COLS} FROM memory_beliefs WHERE tenant_id = $1 AND id = $2"
        ))
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("trace: belief load failed: {e}")))?
        else {
            tx.commit().await.ok();
            return Ok(None);
        };

        let evidence = sqlx::query_as::<_, crate::models::belief_record::MemoryBeliefEvidence>(
            "SELECT id, tenant_id, belief_id, candidate_id, event_id, kind, content_hash, \
             created_at::text AS created_at FROM memory_belief_evidence \
             WHERE tenant_id = $1 AND belief_id = $2 ORDER BY created_at, id",
        )
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("trace: evidence load failed: {e}")))?;

        // Audit rows reference the belief via correlation_id (gate writes) or
        // resource_id (direct belief ops).
        let audit: Vec<crate::models::belief_record::AuditTraceRow> = sqlx::query_as(
            "SELECT event_id, tenant_id, actor_id, event_type, resource_type, resource_id, \
             correlation_id, metadata_json, created_at::text AS created_at \
             FROM memory_audit_events \
             WHERE tenant_id = $1 AND (correlation_id = $2 OR resource_id = $2) \
             ORDER BY created_at, event_id LIMIT 200",
        )
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("trace: audit load failed: {e}")))?;

        tx.commit().await.ok();
        Ok(Some((belief, evidence, audit)))
    }

    /// Deny a pending-confirmation belief: closed and rejected (terminal),
    /// with audit. Idempotent for already-terminal edges.
    pub async fn deny_belief(
        &self,
        tenant_id: &TenantId,
        belief_id: &str,
        actor: Option<&str>,
    ) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let updated = sqlx::query(
            "UPDATE memory_beliefs SET status = 'rejected', valid_to = NOW(), \
             needs_confirm = FALSE, updated_at = NOW() \
             WHERE tenant_id = $1 AND id = $2 AND status IN ('needs_confirm', 'active')",
        )
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("deny failed: {e}")))?;
        let changed = updated.rows_affected() > 0;
        if changed {
            let audit = AuditEvent::new("belief.denied", "memory_belief")
                .tenant(tenant_id.as_str())
                .actor(actor.unwrap_or("unknown"))
                .resource_id(belief_id);
            crate::db::audit::insert_tx(&mut tx, &audit).await?;
        }
        tx.commit().await.ok();
        Ok(changed)
    }

    /// Archive one belief (soft retirement from the current set).
    pub async fn archive_belief(
        &self,
        tenant_id: &TenantId,
        belief_id: &str,
        actor: Option<&str>,
    ) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let updated = sqlx::query(
            "UPDATE memory_beliefs SET status = 'archived', valid_to = NOW(), \
             updated_at = NOW() WHERE tenant_id = $1 AND id = $2 \
             AND status IN ('active', 'needs_confirm', 'stale') AND valid_to IS NULL",
        )
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("archive failed: {e}")))?;
        let changed = updated.rows_affected() > 0;
        if changed {
            let audit = AuditEvent::new("belief.archived", "memory_belief")
                .tenant(tenant_id.as_str())
                .actor(actor.unwrap_or("unknown"))
                .resource_id(belief_id);
            crate::db::audit::insert_tx(&mut tx, &audit).await?;
        }
        tx.commit().await.ok();
        Ok(changed)
    }

    /// #124 rollback: close the CURRENT edge and re-activate its direct
    /// predecessor — the "belief graph can be rolled back to a known-good
    /// snapshot" capability. Returns (closed_id, restored_id).
    pub async fn rollback_belief(
        &self,
        tenant_id: &TenantId,
        belief_id: &str,
        actor: Option<&str>,
    ) -> Result<(String, String), AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let current = sqlx::query_as::<_, MemoryBelief>(&format!(
            "SELECT {BELIEF_COLS} FROM memory_beliefs WHERE tenant_id = $1 AND id = $2 FOR UPDATE"
        ))
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("rollback: load failed: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("belief '{belief_id}' not found")))?;

        let Some(predecessor_id) = current.supersedes_id.clone() else {
            return Err(AppError::BadRequest(format!(
                "belief '{belief_id}' has no predecessor to roll back to"
            )));
        };
        let predecessor = sqlx::query_as::<_, MemoryBelief>(&format!(
            "SELECT {BELIEF_COLS} FROM memory_beliefs WHERE tenant_id = $1 AND id = $2 FOR UPDATE"
        ))
        .bind(tenant_id.as_str())
        .bind(&predecessor_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("rollback: predecessor load failed: {e}")))?
        .ok_or_else(|| {
            AppError::Internal(format!("rollback predecessor '{predecessor_id}' vanished"))
        })?;

        // Close the current edge as superseded-with-no-successor, reopen the
        // predecessor as the current truth. History keeps every version.
        sqlx::query(
            "UPDATE memory_beliefs SET status = 'superseded', valid_to = NOW(), \
             superseded_by_id = NULL, updated_at = NOW() \
             WHERE tenant_id = $1 AND id = $2 AND valid_to IS NULL",
        )
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("rollback: close failed: {e}")))?;
        sqlx::query(
            "UPDATE memory_beliefs SET status = 'active', valid_to = NULL, \
             superseded_by_id = NULL, needs_confirm = FALSE, last_confirmed_at = NOW(), \
             updated_at = NOW() WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id.as_str())
        .bind(&predecessor_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("rollback: restore failed: {e}")))?;

        let audit = AuditEvent::new("belief.rolled_back", "memory_belief")
            .tenant(tenant_id.as_str())
            .actor(actor.unwrap_or("unknown"))
            .resource_id(belief_id)
            .correlation_id(&predecessor_id)
            .with_metadata(&serde_json::json!({
                "closed_belief": belief_id,
                "restored_belief": predecessor_id,
                "restored_object": predecessor.object,
            }));
        crate::db::audit::insert_tx(&mut tx, &audit).await?;
        tx.commit().await.ok();
        Ok((belief_id.to_string(), predecessor_id))
    }

    /// GDPR forget: archive every open edge for a subject. History rows keep
    /// their windows closed (audit-friendly), no content is destroyed in place.
    pub async fn forget_subject(
        &self,
        tenant_id: &TenantId,
        subject: &str,
        actor: Option<&str>,
    ) -> Result<u64, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let updated = sqlx::query(
            "UPDATE memory_beliefs SET status = 'archived', valid_to = NOW(), \
             needs_confirm = FALSE, updated_at = NOW() \
             WHERE tenant_id = $1 AND subject = $2 \
             AND status IN ('active', 'needs_confirm', 'stale') AND valid_to IS NULL",
        )
        .bind(tenant_id.as_str())
        .bind(subject)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("forget failed: {e}")))?;
        let n = updated.rows_affected();
        if n > 0 {
            let audit = AuditEvent::new("belief.subject_forgotten", "memory_belief")
                .tenant(tenant_id.as_str())
                .actor(actor.unwrap_or("unknown"))
                .resource_id(subject)
                .with_metadata(&serde_json::json!({ "archived_edges": n }));
            crate::db::audit::insert_tx(&mut tx, &audit).await?;
        }
        tx.commit().await.ok();
        Ok(n)
    }

    /// Current belief volume for the observability gauge.
    pub async fn active_belief_count(&self, tenant_id: &TenantId) -> Result<i64, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM memory_beliefs \
             WHERE tenant_id = $1 AND status = 'active' AND valid_to IS NULL",
        )
        .bind(tenant_id.as_str())
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("belief count failed: {e}")))?;
        tx.commit().await.ok();
        Ok(n)
    }

    // ── Consolidation read/repair face (#129) ───────────────────────────────── //
    //
    // Every repair is idempotent: guarded UPDATEs (status/window predicates)
    // plus candidate rows keyed by a deterministic idempotency key, so a
    // crashed-and-retried consolidation run leaves the same state as one clean
    // run — never a second supersede chain link.

    /// Single-valued (subject, predicate) pairs holding MORE than one open
    /// edge — impossible under the exclusion constraint unless it was bypassed
    /// (admin surgery, restore from backup); the scan is defense-in-depth.
    /// Only groups with at least one ACTIVE edge are reported (a settled
    /// all-needs_confirm group is already parked for a human).
    pub async fn multi_active_groups(
        &self,
        tenant_id: &TenantId,
        limit: i64,
    ) -> Result<Vec<(String, String, i64)>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            r#"
            SELECT subject, predicate, COUNT(*) AS n
            FROM memory_beliefs
            WHERE tenant_id = $1
              AND single_valued
              AND valid_to IS NULL
              AND status IN ('active', 'needs_confirm')
            GROUP BY subject, predicate
            HAVING COUNT(*) > 1
               AND COUNT(*) FILTER (WHERE status = 'active') > 0
            ORDER BY subject, predicate
            LIMIT $2
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("multi_active scan failed: {e}")))?;
        tx.commit().await.ok();
        Ok(rows)
    }

    /// Repair one multi-active group: keep the newest open edge as the slot
    /// owner, close the rest as superseded pointing at it. If a closed edge's
    /// source was STRICTLY stronger than the winner's, the winner parks in
    /// needs_confirm — the conflict/confirm flow #129 prescribes.
    pub async fn repair_multi_active_group(
        &self,
        tenant_id: &TenantId,
        subject: &str,
        predicate: &str,
    ) -> Result<(String, usize, bool), AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let edges: Vec<MemoryBelief> = sqlx::query_as::<_, MemoryBelief>(&format!(
            r#"
            SELECT {BELIEF_COLS}
            FROM memory_beliefs
            WHERE tenant_id = $1 AND subject = $2 AND predicate = $3
              AND valid_to IS NULL AND status IN ('active', 'needs_confirm')
            ORDER BY valid_from DESC, id
            FOR UPDATE
            "#
        ))
        .bind(tenant_id.as_str())
        .bind(subject)
        .bind(predicate)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("multi_active fetch failed: {e}")))?;
        if edges.len() < 2 {
            tx.commit().await.ok();
            return Ok((String::new(), 0, false));
        }

        let winner = &edges[0];
        let winner_rank = BeliefSource::parse(&winner.source)
            .map(|s| s.precedence_rank())
            .unwrap_or(u8::MAX);
        let mut parked = false;
        let mut closed = 0usize;
        for loser in &edges[1..] {
            let loser_rank = BeliefSource::parse(&loser.source)
                .map(|s| s.precedence_rank())
                .unwrap_or(u8::MAX);
            sqlx::query(
                "UPDATE memory_beliefs SET valid_to = NOW(), status = 'superseded', \
                 superseded_by_id = $1, updated_at = NOW() WHERE id = $2 AND valid_to IS NULL",
            )
            .bind(&winner.id)
            .bind(&loser.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("multi_active close failed: {e}")))?;
            closed += 1;
            if loser_rank < winner_rank {
                parked = true; // a strictly stronger source lost the slot race
            }
        }
        if parked {
            sqlx::query(
                "UPDATE memory_beliefs SET status = 'needs_confirm', needs_confirm = TRUE, \
                 updated_at = NOW() WHERE id = $1 AND status = 'active'",
            )
            .bind(&winner.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("multi_active park failed: {e}")))?;
        }

        // Idempotent audit trail: one candidate per repair action.
        sqlx::query(
            r#"
            INSERT INTO memory_belief_candidates
                (id, tenant_id, principal_id, subject, predicate, object, source, trust,
                 origin, decision, status, rejection_reason, payload_json, idempotency_key, resolved_at)
            VALUES ($1,$2,$3,$4,$5,'(multi-active repair)',$6,0,'manual','conflict','pending',
                    'consolidation: multiple open single-valued edges repaired',
                    $7::jsonb, $8, NOW())
            ON CONFLICT (tenant_id, idempotency_key) WHERE idempotency_key IS NOT NULL
            DO NOTHING
            "#,
        )
        .bind(Ulid::new().to_string())
        .bind(tenant_id.as_str())
        .bind(&winner.principal_id)
        .bind(subject)
        .bind(predicate)
        .bind(&winner.source)
        .bind(serde_json::to_string(&serde_json::json!({
            "winner": winner.id, "closed": closed, "parked": parked,
        })).unwrap())
        .bind(format!("consolidation|multi_active|{subject}|{predicate}"))
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("multi_active candidate failed: {e}")))?;

        let audit = AuditEvent::new(AUDIT_BELIEF_CONFLICT, "memory_belief")
            .tenant(tenant_id.as_str())
            .resource_id(&winner.id)
            .with_metadata(&serde_json::json!({ "subject": subject, "predicate": predicate, "closed": closed }));
        crate::db::audit::insert_tx(&mut tx, &audit).await?;

        tx.commit().await.ok();
        Ok((winner.id.clone(), closed, parked))
    }

    /// Active open edges past their reconfirmation window (stale_scan
    /// policies). SoR-sourced edges never age ("权威系统更新时失效，不靠时间").
    pub async fn stale_candidates(
        &self,
        tenant_id: &TenantId,
        limit: i64,
    ) -> Result<Vec<MemoryBelief>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = sqlx::query_as::<_, MemoryBelief>(&format!(
            r#"
            SELECT {BELIEF_COLS_B}
            FROM memory_beliefs b
            JOIN memory_predicate_policies p ON p.name = b.predicate
            WHERE b.tenant_id = $1
              AND b.status = 'active' AND b.valid_to IS NULL
              AND b.source <> 'system_of_record'
              AND p.ttl_policy = 'stale_scan'
              AND b.last_confirmed_at < NOW() - (p.reconfirm_days * INTERVAL '1 day')
            ORDER BY b.last_confirmed_at ASC
            LIMIT $2
            "#,
        ))
        .bind(tenant_id.as_str())
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("stale scan failed: {e}")))?;
        tx.commit().await.ok();
        Ok(rows)
    }

    /// Mark one edge stale (guarded: only from active — replay-safe).
    /// High-engagement stale edges (feedback count >= threshold) go to the
    /// confirmation queue instead (#129: 高召回 stale 进待确认).
    pub async fn mark_stale(
        &self,
        tenant_id: &TenantId,
        belief_id: &str,
        to_confirm_queue: bool,
    ) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let updated = sqlx::query(
            "UPDATE memory_beliefs SET status = $3, needs_confirm = $4, updated_at = NOW() \
             WHERE tenant_id = $1 AND id = $2 AND status = 'active'",
        )
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .bind(if to_confirm_queue {
            "needs_confirm"
        } else {
            "stale"
        })
        .bind(to_confirm_queue)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("mark_stale failed: {e}")))?;
        let changed = updated.rows_affected() > 0;
        if changed {
            sqlx::query(
                r#"
                INSERT INTO memory_belief_candidates
                    (id, tenant_id, principal_id, subject, predicate, object, source, trust,
                     origin, decision, status, rejection_reason, payload_json, idempotency_key, resolved_at)
                SELECT $1,$2,principal_id,subject,predicate,object,source,trust,'manual',NULL,
                       $5,'consolidation: stale scan', '{}'::jsonb, $4, NOW()
                FROM memory_beliefs WHERE tenant_id=$2 AND id=$3
                ON CONFLICT (tenant_id, idempotency_key) WHERE idempotency_key IS NOT NULL
                DO NOTHING
                "#,
            )
            .bind(Ulid::new().to_string())
            .bind(tenant_id.as_str())
            .bind(belief_id)
            .bind(format!("consolidation|stale|{belief_id}"))
            .bind(if to_confirm_queue { "pending" } else { "accepted" })
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("stale candidate failed: {e}")))?;
            let audit = AuditEvent::new("belief.stale_marked", "memory_belief")
                .tenant(tenant_id.as_str())
                .resource_id(belief_id)
                .with_metadata(&serde_json::json!({ "to_confirm_queue": to_confirm_queue }));
            crate::db::audit::insert_tx(&mut tx, &audit).await?;
        }
        tx.commit().await.ok();
        Ok(changed)
    }

    /// Open time-bounded (promise) edges past due: explicit `due_date`
    /// metadata first, valid_from + 90d fallback.
    pub async fn expired_promises(
        &self,
        tenant_id: &TenantId,
        limit: i64,
    ) -> Result<Vec<MemoryBelief>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = sqlx::query_as::<_, MemoryBelief>(&format!(
            r#"
            SELECT {BELIEF_COLS_B}
            FROM memory_beliefs b
            JOIN memory_predicate_policies p ON p.name = b.predicate
            WHERE b.tenant_id = $1
              AND b.status = 'active' AND b.valid_to IS NULL
              AND p.mutability = 'time_bounded'
              AND COALESCE(
                    (b.metadata_json->>'due_date')::timestamptz,
                    b.valid_from + INTERVAL '90 days'
                  ) < NOW()
            ORDER BY b.valid_from ASC
            LIMIT $2
            "#,
        ))
        .bind(tenant_id.as_str())
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("expired promise scan failed: {e}")))?;
        tx.commit().await.ok();
        Ok(rows)
    }

    /// Retire an expired promise: close the window and archive — it leaves the
    /// current-truth set but survives as episode/history (#124 Epic).
    pub async fn retire_promise(
        &self,
        tenant_id: &TenantId,
        belief_id: &str,
    ) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let updated = sqlx::query(
            r#"
            UPDATE memory_beliefs
            SET valid_to = NOW(), status = 'archived', updated_at = NOW(),
                metadata_json = metadata_json || '{"retired_reason":"promise_expired"}'::jsonb
            WHERE tenant_id = $1 AND id = $2 AND status = 'active' AND valid_to IS NULL
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("retire_promise failed: {e}")))?;
        let changed = updated.rows_affected() > 0;
        if changed {
            sqlx::query(
                r#"
                INSERT INTO memory_belief_candidates
                    (id, tenant_id, principal_id, subject, predicate, object, source, trust,
                     origin, decision, status, rejection_reason, payload_json, idempotency_key, resolved_at)
                SELECT $1,$2,principal_id,subject,predicate,object,source,trust,'manual',NULL,
                       'accepted','consolidation: promise expired','{}'::jsonb, $4, NOW()
                FROM memory_beliefs WHERE tenant_id=$2 AND id=$3
                ON CONFLICT (tenant_id, idempotency_key) WHERE idempotency_key IS NOT NULL
                DO NOTHING
                "#,
            )
            .bind(Ulid::new().to_string())
            .bind(tenant_id.as_str())
            .bind(belief_id)
            .bind(format!("consolidation|promise_expired|{belief_id}"))
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("retire candidate failed: {e}")))?;
            let audit = AuditEvent::new("belief.promise_expired", "memory_belief")
                .tenant(tenant_id.as_str())
                .resource_id(belief_id);
            crate::db::audit::insert_tx(&mut tx, &audit).await?;
        }
        tx.commit().await.ok();
        Ok(changed)
    }

    /// Open web observations older than the decay horizon, still above the
    /// action floor — candidates for trust decay.
    pub async fn web_observations(
        &self,
        tenant_id: &TenantId,
        older_than_hours: i32,
        limit: i64,
    ) -> Result<Vec<MemoryBelief>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = sqlx::query_as::<_, MemoryBelief>(&format!(
            r#"
            SELECT {BELIEF_COLS}
            FROM memory_beliefs
            WHERE tenant_id = $1
              AND source = 'web' AND status = 'active' AND valid_to IS NULL
              AND recorded_at < NOW() - ($2 * INTERVAL '1 hour')
            ORDER BY recorded_at ASC
            LIMIT $3
            "#,
        ))
        .bind(tenant_id.as_str())
        .bind(older_than_hours)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("web scan failed: {e}")))?;
        tx.commit().await.ok();
        Ok(rows)
    }

    /// Decay a web observation's trust to a fixed plateau (idempotent: the
    /// UPDATE only fires when trust is still above the plateau).
    pub async fn decay_web_trust(
        &self,
        tenant_id: &TenantId,
        belief_id: &str,
        plateau: f32,
    ) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let updated = sqlx::query(
            "UPDATE memory_beliefs SET trust = $3, updated_at = NOW() \
             WHERE tenant_id = $1 AND id = $2 AND trust > $3",
        )
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .bind(plateau)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("web decay failed: {e}")))?;
        tx.commit().await.ok();
        Ok(updated.rows_affected() > 0)
    }

    /// Reconciliation target: the LATEST still-current-ish edge for a
    /// subject+predicate, INCLUDING stale ones (a stale edge is the thing an
    /// SoR update most often needs to re-vouch for or replace). Superseded /
    /// archived / rejected history is never a target.
    pub async fn reconcile_target(
        &self,
        tenant_id: &TenantId,
        subject: &str,
        predicate: &str,
    ) -> Result<Option<MemoryBelief>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let row = sqlx::query_as::<_, MemoryBelief>(&format!(
            r#"
            SELECT {BELIEF_COLS}
            FROM memory_beliefs
            WHERE tenant_id = $1 AND subject = $2 AND predicate = $3
              AND valid_to IS NULL
              AND status IN ('active', 'needs_confirm', 'stale')
            ORDER BY valid_from DESC
            LIMIT 1
            FOR UPDATE
            "#
        ))
        .bind(tenant_id.as_str())
        .bind(subject)
        .bind(predicate)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("reconcile_target failed: {e}")))?;
        tx.commit().await.ok();
        Ok(row)
    }

    /// Close a STALE edge without a successor link (the SoR replacement that
    /// follows opens its own edge): history preserved, current set cleaned.
    pub async fn close_stale_edge(
        &self,
        tenant_id: &TenantId,
        belief_id: &str,
    ) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let updated = sqlx::query(
            "UPDATE memory_beliefs SET valid_to = NOW(), status = 'archived', updated_at = NOW() \
             WHERE tenant_id = $1 AND id = $2 AND status = 'stale' AND valid_to IS NULL",
        )
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("close_stale_edge failed: {e}")))?;
        tx.commit().await.ok();
        Ok(updated.rows_affected() > 0)
    }

    /// SoR reconfirmation: reset the aging clock; a stale edge returns to
    /// active (the authority re-vouched for it — "stale → active" transition).
    pub async fn reconfirm_from_sor(
        &self,
        tenant_id: &TenantId,
        belief_id: &str,
    ) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let updated = sqlx::query(
            "UPDATE memory_beliefs SET last_confirmed_at = NOW(), status = 'active', \
             needs_confirm = FALSE, updated_at = NOW() \
             WHERE tenant_id = $1 AND id = $2 AND status IN ('active', 'stale') AND valid_to IS NULL",
        )
        .bind(tenant_id.as_str())
        .bind(belief_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("reconfirm failed: {e}")))?;
        tx.commit().await.ok();
        Ok(updated.rows_affected() > 0)
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

    /// Feedback signal COUNT per belief id (engagement for the #129
    /// high-recall stale routing decision).
    pub async fn feedback_counts(
        &self,
        tenant_id: &TenantId,
        belief_ids: &[String],
    ) -> Result<std::collections::HashMap<String, i64>, AppError> {
        if belief_ids.is_empty() {
            return Ok(Default::default());
        }
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT memory_id, COUNT(*) FROM memory_feedback \
             WHERE tenant_id = $1 AND memory_id = ANY($2) GROUP BY memory_id",
        )
        .bind(tenant_id.as_str())
        .bind(belief_ids)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("feedback count failed: {e}")))?;
        tx.commit().await.ok();
        Ok(rows.into_iter().collect())
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

        // 5. Compare against the open edge under lock. Single-valued: the one
        // slot. Multi-valued: the edge with the SAME object — distinct objects
        // never collide and simply coexist (#129 constraint fix).
        let is_multi = policy.policy.cardinality == "multi";
        let existing = Self::open_edge_in(
            &mut tx,
            tenant_id,
            &claim.subject,
            &claim.predicate,
            is_multi.then_some(claim.object.as_str()),
        )
        .await?;

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
                !is_multi,
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
            !is_multi,
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
        single_valued: bool,
    ) -> Result<String, AppError> {
        let id = Ulid::new().to_string();
        let payload = serde_json::to_string(&claim.payload_json)
            .map_err(|e| AppError::BadRequest(format!("payload not serializable: {e}")))?;
        sqlx::query(
            r#"
            INSERT INTO memory_beliefs
                (id, tenant_id, principal_id, subject, predicate, object, status,
                 source, trust, risk, valid_from, valid_to, recorded_at,
                 supersedes_id, needs_confirm, metadata_json, single_valued, last_confirmed_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10, NOW(), NULL, NOW(), $11, $12, $13::jsonb, $14, NOW())
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
        .bind(single_valued)
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
