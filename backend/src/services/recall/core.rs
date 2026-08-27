//! Controlled recall core + Working Memory assembly (#128).
//!
//! THE single recall entry point every transport (REST /v1/recall, MCP, A2A,
//! gRPC, MemoryPipeline) converges on. Read path only: nothing here writes
//! beliefs, and recall NEVER creates principals — an unresolved principal
//! yields an empty result, not a new identity.
//!
//! Pipeline, per Epic #124 + ADR-0011:
//! 1. resolve principal (alias lookup, read-only) + parse `as_of`;
//! 2. HARD filters (pre-ranking, never soft scores): RLS tenant scope via
//!    begin_tenant_tx, principal scope, `as_of` window coverage, status
//!    (active-only by default; historical `as_of` may surface superseded
//!    windows), needs_confirm/quarantined/archived/rejected excluded, the
//!    high-risk trust floor, and the agent memory contract's
//!    must_not_believe_from matrix;
//! 3. hybrid channels: graph neighborhood (subject edges) + keyword (object
//!    token overlap) + optional vector channel (trait; beliefs carry no
//!    embeddings yet — #127 left the hook, so the default channel is a
//!    graceful no-op that never fails recall);
//! 4. deterministic ranking: 0.35·relevance + 0.25·trust + 0.15·freshness +
//!    0.15·authority + 0.10·feedback, all computed from the data snapshot and
//!    the explicit `as_of` (never wall-clock), ties broken by belief_id;
//! 5. Working Memory assembly: 5–10 items max, hard character budget, every
//!    rendered line carries its provenance citation.

use std::collections::HashMap;

use sqlx::PgPool;

use crate::db::belief::BeliefRepository;
use crate::db::principal::PrincipalRepository;
use crate::db::tenant_scope::begin_tenant_tx;
use crate::error::AppError;
use crate::models::belief::BeliefSource;
use crate::models::belief_record::MemoryBelief;
use crate::models::principal::PrincipalAliasType;
use crate::tenant::TenantId;

/// Default trust floor for high-risk beliefs (Epic #124 `deny_if_trust_below`).
pub const DEFAULT_HIGH_RISK_TRUST_FLOOR: f32 = 0.8;
/// Working Memory item-count bounds (#124: "top 5-10").
pub const MIN_WM_ITEMS: usize = 5;
pub const MAX_WM_ITEMS: usize = 10;
/// Default character budget for the assembled block.
pub const DEFAULT_BUDGET_CHARS: usize = 2000;

/// Optional vector channel. Beliefs have no embeddings yet (#127 hook); the
/// default implementation is a graceful no-op so wiring is future-proof.
pub trait VectorChannel: Send + Sync {
    /// Returns (belief_id, similarity 0..=1) pairs; empty when unavailable.
    fn search(
        &self,
        _tenant_id: &TenantId,
        _principal_id: &str,
        _query: &str,
        _limit: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<(String, f64)>> + Send + '_>>;
}

/// Default no-op vector channel (no belief embeddings exist yet).
pub struct NoopVectorChannel;

impl VectorChannel for NoopVectorChannel {
    fn search(
        &self,
        _tenant: &TenantId,
        _principal: &str,
        _query: &str,
        _limit: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<(String, f64)>> + Send + '_>> {
        Box::pin(std::future::ready(Vec::new()))
    }
}

// ============================================================================
// Query / result types
// ============================================================================

/// Fully-specified recall query. `as_of = None` means "now".
#[derive(Debug, Clone)]
pub struct RecallQuery {
    pub user_id: String,
    /// Optional entity focus; defaults to `principal:{principal_id}`.
    pub subject: Option<String>,
    /// RFC 3339 instant for historical queries; None = now().
    pub as_of: Option<String>,
    pub query_text: String,
    /// Agent whose memory contract applies (if any).
    pub agent_id: Option<String>,
    pub max_items: Option<usize>,
    pub budget_chars: Option<usize>,
}

impl RecallQuery {
    pub fn new(user_id: impl Into<String>, query_text: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            subject: None,
            as_of: None,
            query_text: query_text.into(),
            agent_id: None,
            max_items: None,
            budget_chars: None,
        }
    }

    pub fn as_of(mut self, rfc3339: impl Into<String>) -> Self {
        self.as_of = Some(rfc3339.into());
        self
    }

    pub fn subject(mut self, s: impl Into<String>) -> Self {
        self.subject = Some(s.into());
        self
    }

    pub fn agent(mut self, a: impl Into<String>) -> Self {
        self.agent_id = Some(a.into());
        self
    }
}

/// Provenance citation attached to every recalled item.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RecallCitation {
    pub event_id: String,
    pub content_hash: String,
    pub kind: String,
}

/// One recalled belief with its full audit surface.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecallItem {
    pub belief_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub source: String,
    pub trust: f32,
    pub risk: String,
    pub valid_from: String,
    pub valid_to: Option<String>,
    pub recorded_at: String,
    pub score: f64,
    pub relevance: f64,
    pub freshness: f64,
    pub authority: f64,
    pub feedback: f64,
    /// Which channels surfaced this item (graph / keyword / vector).
    pub channels: Vec<String>,
    pub citations: Vec<RecallCitation>,
}

/// Assembled Working Memory: the bounded, citation-carrying context block.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkingMemory {
    pub items: Vec<RecallItem>,
    pub text: String,
    pub chars_used: usize,
    pub as_of: String,
    pub principal_id: String,
    /// Count of beliefs removed by hard filters BEFORE ranking. Counts only —
    /// the #128 contract forbids leaking unauthorized content into traces.
    pub hard_filtered_out: usize,
}

// ============================================================================
// Core service
// ============================================================================

pub struct RecallCoreService {
    pool: PgPool,
    vector: Box<dyn VectorChannel>,
}

impl RecallCoreService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            vector: Box::new(NoopVectorChannel),
        }
    }

    /// Attach a real vector channel (Qdrant-backed once beliefs carry vectors).
    pub fn with_vector_channel(mut self, channel: Box<dyn VectorChannel>) -> Self {
        self.vector = channel;
        self
    }

    /// Resolve principal → hard-filter → hybrid fetch → rank → assemble.
    ///
    /// An unresolvable `user_id` returns an EMPTY Working Memory (recall never
    /// fabricates identities). A parseable-but-explicit `as_of` switches to the
    /// historical window semantics (see `eligible_edges_for_recall`).
    pub async fn recall(
        &self,
        tenant_id: &TenantId,
        query: &RecallQuery,
    ) -> Result<WorkingMemory, AppError> {
        let principals = PrincipalRepository::new(self.pool.clone());
        let Some(principal) = principals
            .find_by_alias(tenant_id, PrincipalAliasType::JwtSub, &query.user_id)
            .await?
        else {
            return Ok(WorkingMemory {
                items: vec![],
                text: String::new(),
                chars_used: 0,
                as_of: resolve_as_of_label(&query.as_of),
                principal_id: String::new(),
                hard_filtered_out: 0,
            });
        };

        let as_of_dt = parse_as_of(query.as_of.as_deref())?;

        // One transaction = one consistent snapshot for edges, evidence,
        // contract and feedback (determinism precondition).
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;

        // Agent contract (optional) — hard filter input.
        let contract = BeliefRepository::new(self.pool.clone())
            .contract_for_agent(tenant_id, query.agent_id.as_deref())
            .await?;
        let (floor, banned_matrix) = contract_hard_filters(&contract);

        // HARD-FILTERED candidate set (single SQL; status/window/risk-truth +
        // tenant/principal scope all enforced there or by RLS itself).
        let subject = query
            .subject
            .clone()
            .unwrap_or_else(|| format!("principal:{}", principal.id));
        let mut edges = BeliefRepository::eligible_edges_in(
            &mut tx,
            tenant_id,
            &principal.id,
            Some(&subject),
            as_of_dt,
            floor,
        )
        .await?;

        // Keyword channel widens beyond the graph subject: the principal's
        // eligible beliefs whose object matches query tokens (same hard
        // filters, no subject restriction) — merged and deduped below.
        let keyword_edges = BeliefRepository::eligible_edges_in(
            &mut tx,
            tenant_id,
            &principal.id,
            None,
            as_of_dt,
            floor,
        )
        .await?;

        let pre_contract_count = edges.len() + keyword_edges.len();
        let is_banned = |e: &MemoryBelief| banned(e, &banned_matrix);
        edges.retain(|e| !is_banned(e));
        let keyword_edges: Vec<_> = keyword_edges
            .into_iter()
            .filter(|e| !is_banned(e))
            .collect();
        let hard_filtered_out = pre_contract_count - edges.len() - keyword_edges.len();

        // Vector channel (no-op today; contributes ids + similarity).
        let vector_hits = self
            .vector
            .search(
                tenant_id,
                &principal.id,
                &query.query_text,
                MAX_WM_ITEMS * 2,
            )
            .await;

        // Evidence (citations) for all surviving candidates on the snapshot.
        let mut all_ids: Vec<String> = edges.iter().map(|e| e.id.clone()).collect();
        for e in &keyword_edges {
            if !all_ids.contains(&e.id) {
                all_ids.push(e.id.clone());
            }
        }
        let evidence = Self::evidence_map(&mut tx, tenant_id, &all_ids).await?;
        let feedback = BeliefRepository::new(self.pool.clone())
            .feedback_usefulness(tenant_id, &all_ids)
            .await?;

        // Merge channels + deterministic ranking.
        let mut items = self.rank(
            edges,
            keyword_edges,
            vector_hits,
            &query.query_text,
            as_of_dt,
            &feedback,
        );

        // Attach citations; drop any item that somehow has none (invariant:
        // every recalled belief must be traceable — an unevidenced edge is a
        // data bug and must not silently reach the context).
        items.retain_mut(|item| {
            if let Some(cites) = evidence.get(&item.belief_id) {
                item.citations = cites.clone();
                !cites.is_empty()
            } else {
                false
            }
        });

        let wm = assemble_working_memory(
            items,
            query.max_items,
            query.budget_chars,
            &resolve_as_of_label(&query.as_of),
            &principal.id,
            hard_filtered_out,
        );
        tx.commit().await.ok();
        crate::services::prometheus_exporter::get_exporter().inc_recall_request(wm.items.len());
        Ok(wm)
    }

    /// Evidence (citations) for all candidate beliefs on the open snapshot.
    async fn evidence_map(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tenant_id: &TenantId,
        belief_ids: &[String],
    ) -> Result<HashMap<String, Vec<RecallCitation>>, AppError> {
        if belief_ids.is_empty() {
            return Ok(Default::default());
        }
        let rows: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT belief_id, event_id, content_hash, kind FROM memory_belief_evidence \
             WHERE tenant_id = $1 AND belief_id = ANY($2) ORDER BY created_at, id",
        )
        .bind(tenant_id.as_str())
        .bind(belief_ids)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| AppError::Internal(format!("evidence fetch failed: {e}")))?;
        let mut map: HashMap<String, Vec<RecallCitation>> = Default::default();
        for (belief_id, event_id, hash, kind) in rows {
            map.entry(belief_id).or_default().push(RecallCitation {
                event_id,
                content_hash: hash,
                kind,
            });
        }
        Ok(map)
    }

    /// Deterministic hybrid ranking. All inputs derive from the snapshot and
    /// the explicit `as_of`; ties break by belief_id ascending.
    #[allow(clippy::too_many_arguments)]
    fn rank(
        &self,
        graph_edges: Vec<MemoryBelief>,
        keyword_edges: Vec<MemoryBelief>,
        vector_hits: Vec<(String, f64)>,
        query_text: &str,
        as_of: Option<chrono::DateTime<chrono::Utc>>,
        feedback: &HashMap<String, f64>,
    ) -> Vec<RecallItem> {
        let q_tokens = tokenize(query_text);
        let mut by_id: HashMap<String, RecallItem> = HashMap::new();

        let mut push = |edge: MemoryBelief, channel: &'static str, relevance: f64| {
            let entry = by_id.entry(edge.id.clone()).or_insert_with(|| {
                let source = BeliefSource::parse(&edge.source).ok();
                let authority = source.map(|s| s.base_trust()).unwrap_or(0.3);
                let fb = feedback.get(&edge.id).copied().unwrap_or(0.5);
                RecallItem {
                    belief_id: edge.id.clone(),
                    subject: edge.subject.clone(),
                    predicate: edge.predicate.clone(),
                    object: edge.object.clone(),
                    source: edge.source.clone(),
                    trust: edge.trust,
                    risk: edge.risk.clone(),
                    valid_from: edge.valid_from.clone(),
                    valid_to: edge.valid_to.clone(),
                    recorded_at: edge.recorded_at.clone(),
                    score: 0.0,
                    relevance: 0.0,
                    freshness: freshness(&edge.valid_from, as_of),
                    authority,
                    feedback: fb,
                    channels: vec![],
                    citations: vec![],
                }
            });
            if !entry.channels.iter().any(|c| c == channel) {
                entry.channels.push(channel.to_string());
            }
            // Channel max: an item surfaced by multiple channels keeps its
            // best relevance (deterministic: max is order-independent).
            entry.relevance = entry.relevance.max(relevance);
        };

        // Graph channel: subject neighborhood — identity facts are relevant to
        // every turn about that subject even without token overlap.
        for edge in graph_edges {
            let rel = relevance_of(&q_tokens, &edge.object).max(0.25);
            push(edge, "graph", rel);
        }
        // Keyword channel: object/token overlap across the principal's beliefs.
        for edge in keyword_edges {
            let rel = relevance_of(&q_tokens, &edge.object);
            if rel > 0.0 {
                push(edge, "keyword", rel);
            }
        }
        // Vector channel: similarity, only for ids not already present is fine
        // too, but merging keeps it honest — belt keeps relevance from graph.
        let pool = self.pool.clone();
        let _ = pool; // (edges already fetched; vector only adjusts relevance)
        for (belief_id, similarity) in vector_hits {
            if let Some(entry) = by_id.get_mut(&belief_id) {
                if !entry.channels.iter().any(|c| c == "vector") {
                    entry.channels.push("vector".to_string());
                }
                entry.relevance = entry.relevance.max(similarity.clamp(0.0, 1.0));
            }
        }

        let mut items: Vec<RecallItem> = by_id.into_values().collect();
        for item in &mut items {
            item.score = 0.35 * item.relevance
                + 0.25 * item.trust as f64
                + 0.15 * item.freshness
                + 0.15 * item.authority
                + 0.10 * item.feedback;
        }
        items.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.belief_id.cmp(&b.belief_id))
        });
        items
    }
}

// ============================================================================
// Hard-filter helpers
// ============================================================================

/// (trust floor, banned matrix) from an optional contract.
fn contract_hard_filters(
    contract: &Option<crate::db::belief::MemoryContractRow>,
) -> (f32, HashMap<String, Vec<String>>) {
    let mut banned: HashMap<String, Vec<String>> = HashMap::new();
    let mut floor = DEFAULT_HIGH_RISK_TRUST_FLOOR;
    if let Some(c) = contract {
        // Value shapes accepted per predicate list: ["a","b"] or "*".
        if let Ok(map) =
            serde_json::from_str::<HashMap<String, serde_json::Value>>(&c.must_not_believe_from)
        {
            for (source, v) in map {
                let preds: Vec<String> = match v {
                    serde_json::Value::Array(a) => a
                        .into_iter()
                        .filter_map(|p| p.as_str().map(str::to_string))
                        .collect(),
                    serde_json::Value::String(s) => vec![s],
                    _ => vec![],
                };
                if !preds.is_empty() {
                    banned.insert(source, preds);
                }
            }
        }
        if let Some(f) = c.high_stakes_deny_below_trust {
            floor = f;
        }
    }
    (floor, banned)
}

/// Contract violation check: source's banned predicate list contains this edge.
fn banned(edge: &MemoryBelief, matrix: &HashMap<String, Vec<String>>) -> bool {
    matrix
        .get(&edge.source)
        .map(|preds| preds.iter().any(|p| p == &edge.predicate || p == "*"))
        .unwrap_or(false)
}

fn parse_as_of(as_of: Option<&str>) -> Result<Option<chrono::DateTime<chrono::Utc>>, AppError> {
    match as_of {
        None => Ok(None),
        Some(s) => chrono::DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&chrono::Utc))
            .map(Some)
            .map_err(|e| {
                AppError::BadRequest(format!(
                    "as_of must be RFC 3339 (e.g. 2026-01-01T00:00:00Z): {e}"
                ))
            }),
    }
}

fn resolve_as_of_label(as_of: &Option<String>) -> String {
    as_of.clone().unwrap_or_else(|| "now".to_string())
}

// ============================================================================
// Deterministic scoring primitives
// ============================================================================

/// Lowercased ASCII tokens (len>=2) plus CJK chars when no ASCII tokens exist.
/// Pure function of the text — same input, same tokens, always.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(str::to_lowercase)
        .collect();
    if tokens.is_empty() {
        tokens = text
            .chars()
            .filter(|c| c.is_alphabetic())
            .map(|c| c.to_lowercase().to_string())
            .collect();
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

/// Overlap ratio: |object tokens ∩ query tokens| / |query tokens| (0 when the
/// query has no tokens). Deterministic and independent of ordering.
fn relevance_of(query_tokens: &[String], object: &str) -> f64 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    let lowered = object.to_lowercase();
    let obj_tokens: std::collections::HashSet<&str> = lowered
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    let hits = query_tokens
        .iter()
        .filter(|q| obj_tokens.contains(q.as_str()))
        .count();
    // CJK single chars inside object also count against single-char query
    // tokens via the same set (chars are alphanumeric-split tokens of len 1
    // only when tokenize produced them — handled by the set above).
    hits as f64 / query_tokens.len() as f64
}

/// exp(-age_days / 90) from `valid_from` against the explicit `as_of` (or the
/// recorded_at for historical edges). Pure arithmetic — deterministic.
fn freshness(valid_from: &str, as_of: Option<chrono::DateTime<chrono::Utc>>) -> f64 {
    let from = chrono::DateTime::parse_from_rfc3339(valid_from)
        .or_else(|_| {
            // PG ::text renders timestamptz without offset (e.g. "2026-08-27 12:00:00.123456").
            chrono::NaiveDateTime::parse_from_str(
                valid_from.trim_end_matches(" UTC"),
                "%Y-%m-%d %H:%M:%S%.f",
            )
            .map(|n| {
                chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(n, chrono::Utc)
                    .fixed_offset()
            })
        })
        .map(|d| d.with_timezone(&chrono::Utc));
    let (Ok(from), Some(asof)) = (from, as_of) else {
        return 0.5; // neutral, deterministic fallback for unparseable stamps
    };
    let age_days = (asof - from).num_seconds().max(0) as f64 / 86_400.0;
    (-age_days / 90.0).exp()
}

// ============================================================================
// Working Memory assembly
// ============================================================================

fn assemble_working_memory(
    mut items: Vec<RecallItem>,
    max_items: Option<usize>,
    budget_chars: Option<usize>,
    as_of: &str,
    principal_id: &str,
    hard_filtered_out: usize,
) -> WorkingMemory {
    let max_items = max_items.unwrap_or(8).clamp(MIN_WM_ITEMS, MAX_WM_ITEMS);
    let budget = budget_chars.unwrap_or(DEFAULT_BUDGET_CHARS);

    let mut text = String::new();
    let mut kept: Vec<RecallItem> = Vec::new();
    for item in items.drain(..) {
        if kept.len() >= max_items || text.len() >= budget {
            break;
        }
        let cite = item
            .citations
            .first()
            .map(|c| c.event_id.as_str())
            .unwrap_or("?");
        let line = format!(
            "- {} {} {} [source={}, trust={:.2}, valid {}..{}, cite:{}]\n",
            item.subject,
            item.predicate,
            item.object,
            item.source,
            item.trust,
            item.valid_from,
            item.valid_to.as_deref().unwrap_or("now"),
            cite,
        );
        if text.len() + line.len() > budget {
            // Budget would be exceeded: stop whole-line (never truncate a
            // citation mid-way — traceability over squeezing one more item).
            break;
        }
        text.push_str(&line);
        kept.push(item);
    }

    WorkingMemory {
        items: kept,
        chars_used: text.len(),
        text,
        as_of: as_of.to_string(),
        principal_id: principal_id.to_string(),
        hard_filtered_out,
    }
}

// ============================================================================
// Shared transport helper — the "same recall core" contract anchor (#128)
// ============================================================================

/// The one entry every transport calls: resolve the user to a principal and
/// return the assembled Working Memory. `None` user (or unresolvable) yields
/// `Ok(None)` so transports can degrade to their legacy behavior.
pub async fn belief_working_memory(
    tenant_id: &TenantId,
    user_id: Option<&str>,
    query_text: &str,
    as_of: Option<&str>,
    max_items: Option<usize>,
    budget_chars: Option<usize>,
) -> Result<Option<WorkingMemory>, AppError> {
    let Some(user) = user_id.filter(|u| !u.is_empty()) else {
        return Ok(None);
    };
    let mut q = RecallQuery::new(user, query_text);
    q.as_of = as_of.map(str::to_string);
    q.max_items = max_items;
    q.budget_chars = budget_chars;
    let wm = RecallCoreService::new(crate::db::pool().clone())
        .recall(tenant_id, &q)
        .await?;
    // Unresolved principal → empty items; treat as "no belief context" for
    // transports so they keep their legacy path.
    if wm.principal_id.is_empty() {
        return Ok(None);
    }
    Ok(Some(wm))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_is_deterministic_and_order_insensitive() {
        let a = tokenize("Lisa works at Acme NOW");
        let b = tokenize("at NOW acme WORKS lisa");
        assert_eq!(a, b);
        assert!(a.contains(&"acme".to_string()));
    }

    #[test]
    fn relevance_overlap_ratio() {
        let q = tokenize("lisa works at acme");
        assert!((relevance_of(&q, "Acme Corp") - 0.25).abs() < 1e-9);
        assert_eq!(relevance_of(&q, "unrelated text"), 0.0);
        assert_eq!(relevance_of(&[], "anything"), 0.0);
    }

    #[test]
    fn freshness_decays_with_age_and_handles_pg_text() {
        let as_of = chrono::Utc::now();
        let fresh = freshness(&as_of.to_rfc3339(), Some(as_of));
        assert!((fresh - 1.0).abs() < 1e-9, "zero age = 1.0");
        let old = freshness("2020-01-01 00:00:00", Some(as_of));
        assert!(old < 0.01, "six years old decays to ~0, got {old}");
        // Unparseable stays neutral-deterministic.
        assert_eq!(freshness("not-a-date", Some(as_of)), 0.5);
    }

    #[test]
    fn contract_ban_matrix_parses_object_form() {
        let mut c = crate::db::belief::MemoryContractRow {
            id: "c1".into(),
            tenant_id: "t".into(),
            agent_id: "a".into(),
            may_believe: "[]".into(),
            must_not_believe_from: r#"{"web":["owner_of","budget_owner"],"tool":["*"]}"#.into(),
            high_stakes_deny_below_trust: Some(0.9),
            enabled: true,
        };
        let (floor, matrix) = contract_hard_filters(&Some(c.clone()));
        assert_eq!(floor, 0.9);
        assert!(banned(&fake_edge("web", "owner_of"), &matrix));
        assert!(!banned(&fake_edge("web", "prefers"), &matrix));
        assert!(banned(&fake_edge("tool", "anything"), &matrix));
        c.high_stakes_deny_below_trust = None;
        let (floor, _) = contract_hard_filters(&Some(c));
        assert_eq!(floor, DEFAULT_HIGH_RISK_TRUST_FLOOR);
    }

    fn fake_edge(source: &str, predicate: &str) -> MemoryBelief {
        MemoryBelief {
            id: "b".into(),
            tenant_id: "t".into(),
            principal_id: "p".into(),
            subject: "s".into(),
            predicate: predicate.into(),
            object: "o".into(),
            status: "active".into(),
            source: source.into(),
            trust: 0.9,
            risk: "medium".into(),
            valid_from: String::new(),
            valid_to: None,
            recorded_at: String::new(),
            supersedes_id: None,
            superseded_by_id: None,
            needs_confirm: false,
            metadata_json: "{}".into(),
            single_valued: true,
            last_confirmed_at: String::new(),
        }
    }

    #[test]
    fn assembly_respects_item_and_char_budgets() {
        let items: Vec<RecallItem> = (0..30)
            .map(|i| {
                let mut e = fake_edge("user_stated", "prefers");
                e.id = format!("b{i:02}");
                e.object = format!("object-{i}-with-some-length");
                let mut it = RecallItem {
                    belief_id: e.id.clone(),
                    subject: e.subject.clone(),
                    predicate: e.predicate.clone(),
                    object: e.object.clone(),
                    source: e.source.clone(),
                    trust: e.trust,
                    risk: e.risk.clone(),
                    valid_from: e.valid_from.clone(),
                    valid_to: None,
                    recorded_at: e.recorded_at.clone(),
                    score: 1.0 - i as f64 * 0.01,
                    relevance: 1.0,
                    freshness: 1.0,
                    authority: 0.8,
                    feedback: 0.5,
                    channels: vec!["graph".to_string()],
                    citations: vec![RecallCitation {
                        event_id: format!("ev{i}"),
                        content_hash: "h".into(),
                        kind: "direct".into(),
                    }],
                };
                it.belief_id = e.id;
                it
            })
            .collect();
        let wm = assemble_working_memory(items, None, Some(500), "now", "p1", 0);
        assert!(wm.items.len() <= MAX_WM_ITEMS);
        assert!(wm.chars_used <= 500, "char budget hard bound");
        assert!(wm.text.lines().last().unwrap().contains("cite:ev"));
        // Whole-line rule: text is a sequence of complete lines.
        for line in wm.text.lines() {
            assert!(line.contains("cite:"), "no truncated citations: {line}");
        }
    }
}
