//! Belief consolidation acceptance suite (#129) — criteria 1-7.
//!
//! | #129 criterion                                              | test |
//! |-------------------------------------------------------------|------|
//! | 1. multi-active single predicates → conflict/confirm flow    | `multi_active_detected_and_repaired` |
//! | 2. stale/expired/archived excluded from default recall       | `retired_states_leave_recall` |
//! | 3. SoR change closes old belief within the SLA (one cycle)   | `sor_change_closes_and_reconfirms` |
//! | 4. double consolidation run is idempotent                    | `double_run_is_idempotent` |
//! | 5. one tenant's backlog cannot starve another                | `tenant_fair_batching` |
//! | 6. crash-restart safe, no duplicate supersede                | `crash_restart_no_duplicate_supersede` |
//! | 7. metrics have real callers                                 | `metrics_advance_on_real_runs` |
//!
//! Plus the multi-value coexistence regression the multi-active scan surfaced
//! (the #127 constraint applied to ALL predicates): `multi_valued_predicates_coexist`.

use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::PgPool;

use backend::db::belief::BeliefRepository;
use backend::db::memory_event::MemoryEventRepository;
use backend::db::principal::PrincipalRepository;
use backend::models::belief::BeliefSource;
use backend::models::belief_record::{BeliefClaim, ClaimOrigin, GateOutcome};
use backend::models::memory_event::{AppendMemoryEventRequest, MemoryEventType};
use backend::models::principal::{PrincipalAliasType, PrincipalKind};
use backend::services::belief::BeliefGateService;
use backend::services::consolidation::{
    BeliefConsolidationConfig, BeliefConsolidationService, SorUpdate, StaticSorAdapter,
};
use backend::services::recall::core::{RecallCoreService, RecallQuery};
use backend::tenant::TenantId;

fn suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos()
        .to_string()
}

/// Pools: an OWNER pool (bypasses RLS and can drop the exclusion constraint to
/// fabricate anomalies) and a PROBE pool (the app posture). Both on an
/// immortal detached-thread runtime (see the #128 suite for the pattern).
static POOLS: OnceLock<(PgPool, PgPool)> = OnceLock::new();

fn pools() -> (PgPool, PgPool) {
    POOLS
        .get_or_init(|| {
            std::thread::scope(|scope| {
                scope
                    .spawn(|| {
                        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL set");
                        let rt = Box::leak(Box::new(
                            tokio::runtime::Builder::new_multi_thread()
                                .worker_threads(2)
                                .enable_all()
                                .build()
                                .expect("setup runtime"),
                        ));
                        let owner = rt.block_on(async { PgPool::connect(&url).await }).expect("owner");

                        let migrations_path =
                            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
                        rt.block_on(async {
                            sqlx::migrate::Migrator::new(migrations_path)
                                .await
                                .expect("migrator")
                                .run(&owner)
                                .await
                                .expect("migrations");
                            let repo = BeliefRepository::new(owner.clone());
                            let n = repo.sync_catalog_from_code().await.expect("catalog");
                            assert!(n >= 6);

                            let role = "aetheris_belief_probe";
                            let pw = "aetheris_belief_probe_pw";
                            sqlx::raw_sql(&format!(
                                r#"DO $$ BEGIN
                                    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = '{role}') THEN
                                        CREATE ROLE {role} LOGIN PASSWORD '{pw}' NOSUPERUSER NOBYPASSRLS;
                                    END IF;
                                END $$;"#
                            ))
                            .execute(&owner)
                            .await
                            .expect("role");
                            for stmt in [
                                "GRANT USAGE ON SCHEMA public TO aetheris_belief_probe",
                                "GRANT SELECT ON memory_predicate_policies TO aetheris_belief_probe",
                                "GRANT SELECT, INSERT, UPDATE ON memory_beliefs TO aetheris_belief_probe",
                                "GRANT SELECT, INSERT, UPDATE ON memory_belief_candidates TO aetheris_belief_probe",
                                "GRANT SELECT, INSERT, UPDATE ON memory_belief_evidence TO aetheris_belief_probe",
                                "GRANT SELECT, INSERT ON memory_events TO aetheris_belief_probe",
                                "GRANT SELECT, INSERT, UPDATE ON memory_principals TO aetheris_belief_probe",
                                "GRANT SELECT, INSERT, UPDATE ON principal_aliases TO aetheris_belief_probe",
                                "GRANT SELECT, INSERT ON memory_audit_events TO aetheris_belief_probe",
                                "GRANT SELECT, INSERT ON memory_feedback TO aetheris_belief_probe",
                            ] {
                                sqlx::raw_sql(stmt).execute(&owner).await.unwrap_or_else(|e| panic!("{stmt}: {e}"));
                            }

                            // Repair any constraint left dropped by an aborted
                            // debug/test run (defense against dirty databases).
                            sqlx::raw_sql(
                                "DO $$ BEGIN \
                                    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'beliefs_single_open_edge_per_subject') THEN \
                                        ALTER TABLE memory_beliefs ADD CONSTRAINT beliefs_single_open_edge_per_subject \
                                        EXCLUDE USING gist (tenant_id WITH =, subject WITH =, predicate WITH =, \
                                        tstzrange(valid_from, COALESCE(valid_to,'infinity'),'[)') WITH &&) \
                                        WHERE (single_valued AND status IN ('active','needs_confirm')); \
                                    END IF; \
                                END $$;",
                            )
                            .execute(&owner)
                            .await
                            .expect("constraint self-heal");

                            let opts = url
                                .parse::<sqlx::postgres::PgConnectOptions>()
                                .expect("url")
                                .username(role)
                                .password(pw);
                            let probe = sqlx::postgres::PgPoolOptions::new()
                                .min_connections(12)
                                .max_connections(12)
                                .acquire_timeout(std::time::Duration::from_secs(10))
                                .connect_with(opts)
                                .await
                                .expect("probe");
                            for _ in 0..12 {
                                sqlx::query("SELECT 1").execute(&probe).await.expect("warm");
                            }
                            (owner, probe)
                        })
                    })
                    .join()
                    .expect("setup thread")
            })
        })
        .clone()
}

/// Re-add the single-open-edge exclusion constraint (idempotent). Also used
/// at setup: previous failed runs may have left the database without it.
async fn restore_single_edge_constraint(owner: &PgPool) {
    sqlx::raw_sql(
        "DO $$ BEGIN \
            IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'beliefs_single_open_edge_per_subject') THEN \
                ALTER TABLE memory_beliefs ADD CONSTRAINT beliefs_single_open_edge_per_subject \
                EXCLUDE USING gist (tenant_id WITH =, subject WITH =, predicate WITH =, \
                tstzrange(valid_from, COALESCE(valid_to,'infinity'),'[)') WITH &&) \
                WHERE (single_valued AND status IN ('active','needs_confirm')); \
            END IF; \
        END $$;",
    )
    .execute(owner)
    .await
    .expect("constraint restored");
}

struct Ctx {
    tenant: TenantId,
    alias: String,
    principal: String,
    subject: String,
    gate: BeliefGateService,
    repo: BeliefRepository,
    core: RecallCoreService,
    events: MemoryEventRepository,
    probe: PgPool,
    owner: PgPool,
}

async fn ctx(label: &str) -> Ctx {
    let (owner, probe) = pools();
    let tenant = TenantId::from_string(format!("{label}-{}", suffix()));
    let alias = format!("u-{}", suffix());
    let person = PrincipalRepository::new(probe.clone())
        .ensure_with_alias(
            &tenant,
            PrincipalKind::Person,
            Some("Lisa"),
            PrincipalAliasType::JwtSub,
            &alias,
        )
        .await
        .expect("principal");
    let principal = person.principal.id;
    Ctx {
        alias,
        principal: principal.clone(),
        subject: format!("principal:{principal}"),
        tenant,
        gate: BeliefGateService::new(probe.clone()),
        repo: BeliefRepository::new(probe.clone()),
        core: RecallCoreService::new(probe.clone()),
        events: MemoryEventRepository::new(probe.clone()),
        probe,
        owner,
    }
}

impl Ctx {
    async fn evidence(&self, text: &str) -> String {
        self.events
            .append(
                &self.tenant,
                AppendMemoryEventRequest::new(self.principal.clone(), MemoryEventType::UserMessage)
                    .payload(serde_json::json!({ "text": text }))
                    .idempotency_key(format!("ev-{}-{text:0<10}", suffix())),
            )
            .await
            .expect("evidence")
            .id()
            .to_string()
    }

    async fn commit(&self, predicate: &str, object: &str, source: BeliefSource) -> GateOutcome {
        let out = self
            .gate
            .submit(
                &self.tenant,
                BeliefClaim::new(
                    self.principal.clone(),
                    self.subject.clone(),
                    predicate,
                    object,
                    source,
                )
                .origin(ClaimOrigin::Api)
                .idempotency_key(format!(
                    "c|{}|{}|{}|{}",
                    self.principal,
                    predicate,
                    object,
                    suffix()
                ))
                .evidence(vec![self.evidence(&format!("{predicate} {object}")).await]),
            )
            .await
            .expect("gate submit");
        assert!(
            matches!(
                out,
                GateOutcome::Committed { .. } | GateOutcome::Superseded { .. }
            ),
            "expected commit, got {out:?}"
        );
        out
    }

    fn service(&self, adapter: Arc<StaticSorAdapter>, batch: i64) -> BeliefConsolidationService {
        BeliefConsolidationService::new(
            self.probe.clone(),
            adapter as Arc<dyn backend::services::consolidation::SorAdapter>,
            BeliefConsolidationConfig {
                per_tenant_batch: batch,
                ..Default::default()
            },
        )
    }

    /// Count open (active|needs_confirm) edges for subject+predicate.
    async fn open_count(&self, predicate: &str) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM memory_beliefs WHERE tenant_id=$1 AND subject=$2 AND predicate=$3 \
             AND valid_to IS NULL AND status IN ('active','needs_confirm')",
        )
        .bind(self.tenant.as_str())
        .bind(&self.subject)
        .bind(predicate)
        .fetch_one(&mut *backend::db::tenant_scope::begin_tenant_tx(&self.probe, &self.tenant).await.unwrap())
        .await
        .unwrap()
    }
}

/// Run one tenant-scoped statement AND COMMIT it (a dropped Transaction rolls
/// back — the exact trap this suite's first version fell into).
async fn tx_exec(probe: &PgPool, tenant: &TenantId, sql: &str, binds: &[&str]) {
    use backend::db::tenant_scope::begin_tenant_tx;
    let mut tx = begin_tenant_tx(probe, tenant).await.expect("tx");
    let mut q = sqlx::query(sql);
    for b in binds {
        q = q.bind(b);
    }
    q.execute(&mut *tx).await.expect("tx exec");
    tx.commit().await.expect("tx commit");
}

fn no_adapter() -> Arc<StaticSorAdapter> {
    Arc::new(StaticSorAdapter::new())
}

// ── Regression: multi-valued predicates coexist (#127 constraint fix) ────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn multi_valued_predicates_coexist() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP");
        return;
    }
    let c = ctx("coex").await;
    for obj in ["coffee", "tea", "dark-mode"] {
        let _ = c.commit("prefers", obj, BeliefSource::UserStated).await;
    }
    assert_eq!(
        c.open_count("prefers").await,
        3,
        "distinct preferences coexist"
    );
    // And a repeat of the SAME preference is a NOOP, not a new edge.
    let dup = c
        .gate
        .submit(
            &c.tenant,
            BeliefClaim::new(
                c.principal.clone(),
                c.subject.clone(),
                "prefers",
                "coffee",
                BeliefSource::UserStated,
            )
            .idempotency_key(format!("dup-{}", suffix()))
            .evidence(vec![c.evidence("coffee again").await]),
        )
        .await
        .unwrap();
    assert!(matches!(dup, GateOutcome::Noop { .. }), "{dup:?}");
    assert_eq!(c.open_count("prefers").await, 3);
}

// ── 1. Multi-active detection + repair ───────────────────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn multi_active_detected_and_repaired() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP");
        return;
    }
    let c = ctx("multia").await;
    let _ = c
        .commit("works_at", "OldCo", BeliefSource::UserStated)
        .await;

    // Fabricate the anomaly the exclusion constraint should have prevented:
    // drop it as the OWNER, insert a second open edge, restore afterwards.
    sqlx::raw_sql(
        "ALTER TABLE memory_beliefs DROP CONSTRAINT IF EXISTS beliefs_single_open_edge_per_subject",
    )
    .execute(&c.owner)
    .await
    .expect("drop constraint (idempotent)");
    let anomalous_id = format!("anom-{}", suffix());
    sqlx::query(
        "INSERT INTO memory_beliefs (id, tenant_id, principal_id, subject, predicate, object, status, source, trust, risk, single_valued, last_confirmed_at) \
         VALUES ($1,$2,$3,$4,'works_at','NewCoByBypass','active','user_stated',0.8,'medium',TRUE, NOW())",
    )
    .bind(&anomalous_id)
    .bind(c.tenant.as_str())
    .bind(&c.principal)
    .bind(&c.subject)
    .execute(&c.owner)
    .await
    .expect("fabricate anomaly");

    let report = c.service(no_adapter(), 10).run_for_tenant(&c.tenant).await;
    assert_eq!(
        report.multi_active_repaired, 1,
        "scan found the bypass group"
    );
    assert!(report.edges_closed_in_repair >= 1);
    assert!(report.errors.is_empty(), "{:?}", report.errors);

    // Exactly one open edge remains; the older one is closed and chain-linked.
    assert_eq!(c.open_count("works_at").await, 1);
    let winner = c
        .repo
        .open_edge(&c.tenant, &c.subject, "works_at")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(winner.object, "NewCoByBypass", "newest wins the slot");
    let history = c
        .repo
        .history_for(&c.tenant, &c.subject, "works_at")
        .await
        .unwrap();
    let loser = history.iter().find(|b| b.id != winner.id).unwrap();
    assert_eq!(loser.status, "superseded");
    assert!(loser.valid_to.is_some());
    assert_eq!(loser.superseded_by_id.as_deref(), Some(winner.id.as_str()));

    // The constraint can come back now that the invariant holds again.
    sqlx::raw_sql(
        "ALTER TABLE memory_beliefs ADD CONSTRAINT beliefs_single_open_edge_per_subject \
         EXCLUDE USING gist (tenant_id WITH =, subject WITH =, predicate WITH =, \
         tstzrange(valid_from, COALESCE(valid_to,'infinity'),'[)') WITH &&) \
         WHERE (single_valued AND status IN ('active','needs_confirm'))",
    )
    .execute(&c.owner)
    .await
    .expect("constraint restored after repair");
}

// ── 2. Retired states leave default recall ───────────────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn retired_states_leave_recall() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP");
        return;
    }
    let c = ctx("retire").await;

    // (a) A promise with an explicit past due date retires to archived.
    let _ = c
        .gate
        .submit(
            &c.tenant,
            BeliefClaim::new(
                c.principal.clone(),
                c.subject.clone(),
                "promised",
                "deliver by Friday",
                BeliefSource::UserStated,
            )
            .payload(serde_json::json!({ "due_date": "2020-01-01T00:00:00Z" }))
            .idempotency_key(format!("promise-{}", suffix()))
            .evidence(vec![c.evidence("I promise").await]),
        )
        .await
        .unwrap();

    // (b) A mutable fact made stale-eligible by backdating last_confirmed_at.
    let _ = c
        .commit("works_at", "StaleCo", BeliefSource::UserStated)
        .await;
    tx_exec(
        &c.probe,
        &c.tenant,
        "UPDATE memory_beliefs SET last_confirmed_at = NOW() - INTERVAL '400 days' WHERE tenant_id=$1 AND subject=$2 AND predicate='works_at'",
        &[c.tenant.as_str(), &c.subject],
    )
    .await;

    // Sanity: before consolidation both are recallable.
    let before = c
        .core
        .recall(&c.tenant, &RecallQuery::new(&c.alias, ""))
        .await
        .unwrap();
    assert!(before.items.iter().any(|i| i.predicate == "promised"));
    assert!(before.items.iter().any(|i| i.object == "StaleCo"));

    let report = c.service(no_adapter(), 10).run_for_tenant(&c.tenant).await;
    assert_eq!(report.promises_expired, 1);
    assert!(report.stale_marked + report.confirm_queued >= 1);

    let after = c
        .core
        .recall(&c.tenant, &RecallQuery::new(&c.alias, ""))
        .await
        .unwrap();
    assert!(
        !after.items.iter().any(|i| i.predicate == "promised"),
        "expired promise left the current set"
    );
    assert!(
        !after.items.iter().any(|i| i.object == "StaleCo"),
        "stale fact left the current set"
    );
    // History preserved: both still exist as rows, just not current truth.
    let promise_hist = c
        .repo
        .history_for(&c.tenant, &c.subject, "promised")
        .await
        .unwrap();
    assert_eq!(promise_hist.len(), 1);
    assert_eq!(promise_hist[0].status, "archived");
    assert!(promise_hist[0].valid_to.is_some());
}

// ── 3. SoR close/reconfirm within one cycle ──────────────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn sor_change_closes_and_reconfirms() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP");
        return;
    }
    let c = ctx("sor").await;
    let _ = c
        .commit("works_at", "OldCo", BeliefSource::UserStated)
        .await;

    // HR says transferred.
    let adapter = Arc::new(StaticSorAdapter::new());
    adapter.set_updates(
        &c.tenant,
        vec![SorUpdate::new(&c.subject, "works_at", "NewCo", "hr-mock").principal(&c.principal)],
    );
    let report = c
        .service(adapter.clone(), 10)
        .run_for_tenant(&c.tenant)
        .await;
    assert_eq!(
        report.sor_closed, 1,
        "SoR supersedes within one cycle (the SLA)"
    );
    assert_eq!(report.sor_diffs, 1);

    let open = c
        .repo
        .open_edge(&c.tenant, &c.subject, "works_at")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(open.object, "NewCo");
    assert_eq!(open.source, "system_of_record");
    // Old edge closed with chain pointer — history intact.
    let history = c
        .repo
        .history_for(&c.tenant, &c.subject, "works_at")
        .await
        .unwrap();
    let old = history.iter().find(|b| b.object == "OldCo").unwrap();
    assert_eq!(old.status, "superseded");

    // HR re-vouches the SAME value: reconfirm refreshes the clock; a stale
    // edge returns to active without a new version.
    tx_exec(
        &c.probe,
        &c.tenant,
        "UPDATE memory_beliefs SET last_confirmed_at = NOW() - INTERVAL '400 days', status='stale' WHERE tenant_id=$1 AND subject=$2 AND predicate='works_at' AND valid_to IS NULL",
        &[c.tenant.as_str(), &c.subject],
    )
    .await;
    adapter.set_updates(
        &c.tenant,
        vec![SorUpdate::new(&c.subject, "works_at", "NewCo", "hr-mock").principal(&c.principal)],
    );
    let report2 = c.service(adapter, 10).run_for_tenant(&c.tenant).await;
    assert_eq!(
        report2.sor_refreshed, 1,
        "same-object SoR update reconfirms"
    );
    let revived = c
        .repo
        .open_edge(&c.tenant, &c.subject, "works_at")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revived.status, "active", "stale edge revived by authority");
    assert_eq!(
        c.repo
            .history_for(&c.tenant, &c.subject, "works_at")
            .await
            .unwrap()
            .len(),
        2,
        "no extra version rows"
    );
}

// ── 4. Idempotent double run ─────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn double_run_is_idempotent() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP");
        return;
    }
    let c = ctx("idem").await;
    let _ = c
        .commit("works_at", "OldCo", BeliefSource::UserStated)
        .await;
    tx_exec(
        &c.probe,
        &c.tenant,
        "UPDATE memory_beliefs SET last_confirmed_at = NOW() - INTERVAL '400 days' WHERE tenant_id=$1 AND subject=$2 AND predicate='works_at'",
        &[c.tenant.as_str(), &c.subject],
    )
    .await;

    let adapter = no_adapter();
    let svc = c.service(adapter, 10);
    let r1 = svc.run_for_tenant(&c.tenant).await;
    async fn snapshot(ctx: &Ctx) -> (Vec<(String, String, Option<String>)>, usize) {
        let hist = ctx
            .repo
            .history_for(&ctx.tenant, &ctx.subject, "works_at")
            .await
            .unwrap();
        let candidates = ctx
            .repo
            .list_candidates(&ctx.tenant, None, 100)
            .await
            .unwrap()
            .len();
        (
            hist.iter()
                .map(|b| (b.id.clone(), b.status.clone(), b.valid_to.clone()))
                .collect(),
            candidates,
        )
    }
    let s1 = snapshot(&c).await;
    let r2 = svc.run_for_tenant(&c.tenant).await;
    let s2 = snapshot(&c).await;

    assert_eq!(
        s1, s2,
        "identical belief state and candidate count after the second run"
    );
    assert_eq!(
        r2.stale_marked + r2.confirm_queued,
        0,
        "nothing left to mark"
    );
    assert!(r2.errors.is_empty(), "{:?}", r2.errors);
    assert!(r1.stale_marked + r1.confirm_queued >= 1);
}

// ── 5. Tenant fairness ───────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn tenant_fair_batching() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP");
        return;
    }
    let (owner, probe) = pools();
    let big = ctx("fairbig").await;
    let small = ctx("fairsmall").await;

    // Big tenant: 12 stale-eligible mutable facts. Small tenant: 1.
    for ctx in [&big, &small] {
        let n = if std::ptr::eq(ctx.tenant.as_str(), big.tenant.as_str()) {
            12
        } else {
            1
        };
        for i in 0..n {
            let pred = if i == 0 { "works_at" } else { "prefers" };
            let _ = ctx
                .gate
                .submit(
                    &ctx.tenant,
                    BeliefClaim::new(
                        ctx.principal.clone(),
                        ctx.subject.clone(),
                        pred,
                        if pred == "works_at" {
                            "AgedCo".to_string()
                        } else {
                            format!("old-pref-{i}")
                        },
                        BeliefSource::UserStated,
                    )
                    .idempotency_key(format!("fair|{}|{i}", ctx.principal))
                    .evidence(vec![ctx.evidence(&format!("fair {i}")).await]),
                )
                .await
                .unwrap();
        }
        tx_exec(
            &probe,
            &ctx.tenant,
            "UPDATE memory_beliefs SET last_confirmed_at = NOW() - INTERVAL '400 days' WHERE tenant_id=$1 AND subject=$2",
            &[ctx.tenant.as_str(), &ctx.subject],
        )
        .await;
    }
    let _ = (&owner, &small.alias);

    // Budget of 5 per scan per tenant: the big tenant processes part of its
    // backlog while the small tenant finishes ENTIRELY in the same round.
    let svc = BeliefConsolidationService::new(
        probe.clone(),
        no_adapter() as Arc<dyn backend::services::consolidation::SorAdapter>,
        BeliefConsolidationConfig {
            per_tenant_batch: 5,
            ..Default::default()
        },
    );
    let reports = svc
        .process_round(&[big.tenant.clone(), small.tenant.clone()])
        .await;
    let big_report = &reports[big.tenant.as_str()];
    let small_report = &reports[small.tenant.as_str()];
    assert_eq!(
        big_report.stale_marked + big_report.confirm_queued,
        5,
        "big tenant got exactly its budget"
    );
    assert_eq!(
        small_report.stale_marked + small_report.confirm_queued,
        1,
        "small tenant was NOT starved by the big backlog"
    );
}

// ── 6. Crash-restart: no duplicate supersede ─────────────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn crash_restart_no_duplicate_supersede() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP");
        return;
    }
    let c = ctx("crash").await;
    let _ = c
        .commit("works_at", "OldCo", BeliefSource::UserStated)
        .await;

    // Same SoR update applied across THREE separate "process lifetimes"
    // (fresh service instances, as after a crash-restart). Exactly one
    // supersede happens; the replays resolve to the original outcome.
    for _ in 0..3 {
        let adapter = Arc::new(StaticSorAdapter::new());
        adapter.set_updates(
            &c.tenant,
            vec![
                SorUpdate::new(&c.subject, "works_at", "NewCo", "crm-mock").principal(&c.principal)
            ],
        );
        let svc = BeliefConsolidationService::new(
            c.probe.clone(),
            adapter as Arc<dyn backend::services::consolidation::SorAdapter>,
            Default::default(),
        );
        let _ = svc.run_for_tenant(&c.tenant).await;
    }

    let history = c
        .repo
        .history_for(&c.tenant, &c.subject, "works_at")
        .await
        .unwrap();
    assert_eq!(
        history.len(),
        2,
        "two versions only — no duplicate supersede links"
    );
    assert_eq!(
        history.iter().filter(|b| b.status == "superseded").count(),
        1
    );
    assert_eq!(history.iter().filter(|b| b.status == "active").count(), 1);
    // Idempotency-keyed candidates: one reconcile trail, not three.
    let sor_candidates = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM memory_belief_candidates WHERE tenant_id=$1 AND idempotency_key LIKE 'sor|crm-mock|%'",
    )
    .bind(c.tenant.as_str())
    .fetch_one(&mut *backend::db::tenant_scope::begin_tenant_tx(&c.probe, &c.tenant).await.unwrap())
    .await
    .unwrap();
    assert_eq!(
        sor_candidates, 1,
        "claim-level idempotency collapses replays"
    );
}

// ── 7. Metrics advance on real runs ──────────────────────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn metrics_advance_on_real_runs() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP");
        return;
    }
    let c = ctx("metric").await;
    let _ = c
        .commit("works_at", "OldCo", BeliefSource::UserStated)
        .await;
    tx_exec(
        &c.probe,
        &c.tenant,
        "UPDATE memory_beliefs SET last_confirmed_at = NOW() - INTERVAL '400 days' WHERE tenant_id=$1 AND subject=$2 AND predicate='works_at'",
        &[c.tenant.as_str(), &c.subject],
    )
    .await;

    let adapter = Arc::new(StaticSorAdapter::new());
    adapter.set_updates(
        &c.tenant,
        vec![
            SorUpdate::new(&c.subject, "works_at", "MetricCo", "crm-mock").principal(&c.principal),
        ],
    );

    let before = metric_snapshot();
    let report = c.service(adapter, 10).run_for_tenant(&c.tenant).await;
    let after = metric_snapshot();

    // Counters are process-global and this suite runs its tests in parallel:
    // assert MONOTONE growth of at least this run's contribution.
    assert!(
        after.runs >= before.runs + 1,
        "runs advanced: {} -> {}",
        before.runs,
        after.runs
    );
    assert!(after.stale >= before.stale + (report.stale_marked + report.confirm_queued) as u64);
    assert!(after.reconciliation_diffs >= before.reconciliation_diffs + report.sor_diffs as u64);
    assert!(
        after.run_duration_observed >= before.run_duration_observed + 1,
        "latency histogram observed: {} -> {}",
        before.run_duration_observed,
        after.run_duration_observed
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
}

#[derive(Default)]
struct MetricSnapshot {
    runs: u64,
    stale: u64,
    reconciliation_diffs: u64,
    failures: u64,
    run_duration_observed: u64,
}

fn metric_snapshot() -> MetricSnapshot {
    // Real-caller surface: the /metrics exposition the exporter serves. Reading
    // through gather() proves the counters are REGISTERED and moved by the
    // service, not just incremented in isolation.
    use prometheus::Encoder as _;
    let exporter = backend::services::prometheus_exporter::get_exporter();
    let text = exporter.generate_prometheus_output();

    let pick = |name: &str| -> u64 {
        for line in text.lines() {
            let with_underscore = format!("{name}_");
            if line.starts_with(name) && !line.starts_with(&with_underscore) {
                if let Some(v) = line.rsplit(' ').next() {
                    if let Ok(f) = v.parse::<f64>() {
                        return f as u64;
                    }
                }
            }
        }
        0
    };
    let observed = text
        .lines()
        .find(|l| l.starts_with("consolidation_run_duration_seconds_count"))
        .and_then(|l| l.rsplit(' ').next())
        .and_then(|v| v.parse::<f64>().ok())
        .map(|f| f as u64)
        .unwrap_or(0);
    MetricSnapshot {
        runs: pick("consolidation_runs_total"),
        stale: pick("consolidation_stale_marked_total"),
        reconciliation_diffs: pick("consolidation_reconciliation_diffs_total"),
        failures: pick("consolidation_failures_total"),
        run_duration_observed: observed,
    }
}
