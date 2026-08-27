//! Epic #124 gap-closure suite — the three real gaps surfaced by the
//! post-Epic audit, each with real-database evidence:
//!
//! | Gap (#124 body)                                | test |
//! |------------------------------------------------|------|
//! | 预取: prefetch warms the next turn, invalidates on write | `prefetch_cache_hits_and_invalidates` |
//! | WM = 近 N 轮 + 信念 + 工具草稿, one budget    | `working_memory_merges_context_lines` |
//! | 记忆契约管理面 (no API existed)                | `contract_lifecycle_enforced_in_recall` |
//! | 用户可改/可删自己的记忆                          | `self_service_permissions` |
//! | 行为监控告警有真实调用者                          | unit suite in `services::memory_monitor` + worker wiring (source assertion below) |

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::PgPool;

use backend::db::belief::BeliefRepository;
use backend::db::memory_event::MemoryEventRepository;
use backend::db::principal::PrincipalRepository;
use backend::models::belief::BeliefSource;
use backend::models::belief_record::BeliefClaim;
use backend::models::memory_event::{AppendMemoryEventRequest, MemoryEventType};
use backend::models::principal::{PrincipalAliasType, PrincipalKind};
use backend::routers::memory_governance::{
    archive_belief, list_contracts, self_correct, self_forget, upsert_contract,
    GovernanceContractList, GovernanceMutationResult, UpsertContractRequest,
};
use backend::services::belief::BeliefGateService;
use backend::services::rbac::get_rbac_service;
use backend::services::recall::core::{RecallCoreService, RecallQuery};
use backend::tenant::{RequestTenantContext, TenantId};

fn suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos()
        .to_string()
}

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
                            BeliefRepository::new(owner.clone())
                                .sync_catalog_from_code()
                                .await
                                .expect("catalog");

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
                                "GRANT SELECT, INSERT, UPDATE ON tenant_members TO aetheris_belief_probe",
                                "GRANT SELECT, INSERT, UPDATE ON memory_contracts TO aetheris_belief_probe",
                            ] {
                                sqlx::raw_sql(stmt).execute(&owner).await.unwrap_or_else(|e| panic!("{stmt}: {e}"));
                            }
                            let opts = url
                                .parse::<sqlx::postgres::PgConnectOptions>()
                                .expect("url")
                                .username(role)
                                .password(pw);
                            let probe = sqlx::postgres::PgPoolOptions::new()
                                .min_connections(16)
                                .max_connections(16)
                                .acquire_timeout(std::time::Duration::from_secs(10))
                                .connect_with(opts)
                                .await
                                .expect("probe");
                            for _ in 0..16 {
                                sqlx::query("SELECT 1").execute(&probe).await.expect("warm");
                            }
                            let _ = backend::db::DATABASE_POOL
                                .set(backend::db::DatabasePool::Postgres(probe.clone()));
                            (owner, probe)
                        })
                    })
                    .join()
                    .expect("setup thread")
            })
        })
        .clone()
}

struct World {
    tenant: TenantId,
    gate: BeliefGateService,
    repo: BeliefRepository,
    core: RecallCoreService,
    events: MemoryEventRepository,
    probe: PgPool,
    owner: PgPool,
}

async fn world(label: &str) -> World {
    let (owner, probe) = pools();
    World {
        tenant: TenantId::from_string(format!("{label}-{}", suffix())),
        gate: BeliefGateService::new(probe.clone()),
        repo: BeliefRepository::new(probe.clone()),
        core: RecallCoreService::new(probe.clone()),
        events: MemoryEventRepository::new(probe.clone()),
        probe,
        owner,
    }
}

impl World {
    async fn person(&self, name: &str) -> (String, String, String) {
        let alias = format!("{name}-{}", suffix());
        let p = PrincipalRepository::new(self.probe.clone())
            .ensure_with_alias(
                &self.tenant,
                PrincipalKind::Person,
                Some(name),
                PrincipalAliasType::JwtSub,
                &alias,
            )
            .await
            .expect("person");
        let id = p.principal.id;
        (alias, id.clone(), format!("principal:{id}"))
    }

    async fn say(&self, principal: &str, subject: &str, predicate: &str, object: &str) {
        let ev = self
            .events
            .append(
                &self.tenant,
                AppendMemoryEventRequest::new(principal.to_string(), MemoryEventType::UserMessage)
                    .payload(serde_json::json!({ "text": format!("{predicate} {object}") }))
                    .idempotency_key(format!("say|{principal}|{predicate}|{object}|{}", suffix())),
            )
            .await
            .expect("ev");
        let out = self
            .gate
            .submit(
                &self.tenant,
                BeliefClaim::new(
                    principal.to_string(),
                    subject.to_string(),
                    predicate,
                    object,
                    BeliefSource::UserStated,
                )
                .evidence(vec![ev.id().to_string()])
                .idempotency_key(format!("g|{principal}|{predicate}|{object}|{}", suffix())),
            )
            .await
            .expect("gate");
        assert!(
            matches!(
                out,
                backend::models::belief_record::GateOutcome::Committed { .. }
                    | backend::models::belief_record::GateOutcome::Superseded { .. }
            ),
            "{out:?}"
        );
    }

    fn ctx(&self, alias: &str) -> RequestTenantContext {
        RequestTenantContext::from_authenticated(
            self.tenant.as_str().to_string(),
            alias.to_string(),
        )
    }

    async fn make_admin(&self, alias: &str) {
        sqlx::query(
            "INSERT INTO tenants (tenant_id, name) VALUES ($1, 'gap') ON CONFLICT DO NOTHING",
        )
        .bind(self.tenant.as_str())
        .execute(&self.owner)
        .await
        .unwrap();
        sqlx::query("INSERT INTO users (id, username, password) VALUES ($1, $1, 'x') ON CONFLICT DO NOTHING")
            .bind(alias)
            .execute(&self.owner)
            .await
            .unwrap();
        get_rbac_service()
            .assign_role(
                self.tenant.as_str(),
                alias,
                backend::services::rbac::Role::Admin,
                "seed",
            )
            .await
            .expect("admin");
    }
}

// ── 预取: cache hit + write invalidation ─────────────────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn prefetch_cache_hits_and_invalidates() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP");
        return;
    }
    let w = world("pfx").await;
    let (alias, principal, subject) = w.person("penny").await;
    w.say(&principal, &subject, "works_at", "PreCo").await;

    // First recall: cold (no cache yet).
    let q = || RecallQuery::new(&alias, "工作");
    let cold = w.core.recall(&w.tenant, &q()).await.unwrap();
    assert!(!cold.from_prefetch_cache, "first recall is a cold fetch");
    assert!(cold.items.iter().any(|i| i.object == "PreCo"));

    // Prefetch (what pipeline.turn_committed does) + next recall hits cache.
    w.core.prefetch(&w.tenant, &alias).await.unwrap();
    let hit = w.core.recall(&w.tenant, &q()).await.unwrap();
    assert!(hit.from_prefetch_cache, "prefetched snapshot is reused");
    // Determinism preserved: identical output modulo the cache flag.
    let mut a = serde_json::to_value(&cold).unwrap();
    let mut b = serde_json::to_value(&hit).unwrap();
    a["from_prefetch_cache"].take();
    b["from_prefetch_cache"].take();
    assert_eq!(a, b, "cached recall is identical apart from the cache flag");

    // Any write to the principal's open edges invalidates: SoR correction.
    let ev = w
        .events
        .append(
            &w.tenant,
            AppendMemoryEventRequest::new(principal.clone(), MemoryEventType::ExternalRecord)
                .actor("hr")
                .payload(serde_json::json!({ "object": "PostCo" }))
                .idempotency_key(format!("hr|{}|{}", principal, suffix())),
        )
        .await
        .unwrap();
    w.gate
        .submit(
            &w.tenant,
            BeliefClaim::new(
                principal.clone(),
                subject.clone(),
                "works_at",
                "PostCo",
                BeliefSource::SystemOfRecord,
            )
            .evidence(vec![ev.id().to_string()])
            .idempotency_key(format!("hr2|{}|{}", principal, suffix())),
        )
        .await
        .unwrap();

    let fresh = w.core.recall(&w.tenant, &q()).await.unwrap();
    assert!(!fresh.from_prefetch_cache, "write invalidated the snapshot");
    assert!(
        fresh.items.iter().any(|i| i.object == "PostCo"),
        "fresh data visible immediately"
    );
}

// ── WM 上下文合并（近 N 轮 + 工具草稿，同一预算） ───────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn working_memory_merges_context_lines() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP");
        return;
    }
    let w = world("ctx").await;
    let (alias, principal, subject) = w.person("connor").await;
    w.say(&principal, &subject, "works_at", "CtxCo").await;

    let mut q = RecallQuery::new(&alias, "工作");
    q.context_lines = vec![
        "user: 我上周说要周五交付".to_string(),
        "tool: calendar shows Friday deadline".to_string(),
    ];
    let wm = w.core.recall(&w.tenant, &q).await.unwrap();
    assert!(wm.text.starts_with("[ctx]"), "context lines lead the block");
    assert!(wm.text.contains("周五交付") && wm.text.contains("calendar"));
    assert!(
        wm.text.contains("cite:"),
        "belief lines keep their citations"
    );
    assert!(wm.chars_used <= 2000, "single shared budget");

    // Tight budget: context is what gets truncated, never a belief citation.
    let mut tight = RecallQuery::new(&alias, "工作");
    tight.context_lines = vec!["a very long context line ".repeat(40)];
    tight.budget_chars = Some(400);
    let wm = w.core.recall(&w.tenant, &tight).await.unwrap();
    assert!(wm.chars_used <= 400);
    assert!(!wm.items.is_empty(), "beliefs survive context pressure");
    for line in wm.text.lines() {
        if line.contains("CtxCo") {
            assert!(line.contains("cite:"), "no truncated citation lines");
        }
    }
}

// ── 契约管理面 ───────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn contract_lifecycle_enforced_in_recall() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP");
        return;
    }
    let w = world("con").await;
    let (alias, principal, subject) = w.person("carl").await;
    let admin_alias = w.person("admin-con").await.0;
    w.make_admin(&admin_alias).await;

    // Belief exists and is recallable for the agent without a contract.
    w.say(&principal, &subject, "prefers", "email-digest").await;
    let mut q = RecallQuery::new(&alias, "digest");
    q.agent_id = Some("copilot".to_string());
    let wm = w.core.recall(&w.tenant, &q).await.unwrap();
    assert!(wm.items.iter().any(|i| i.object == "email-digest"));

    // Admin creates a contract: user_stated must not believe `prefers` for
    // this agent — the hard filter applies BEFORE ranking.
    let ctx = w.ctx(&admin_alias);
    upsert_contract(
        axum::extract::Extension(ctx.clone()),
        axum::extract::Path("copilot".to_string()),
        axum::Json(UpsertContractRequest {
            may_believe: serde_json::json!([]),
            must_not_believe_from: serde_json::json!({ "user_stated": ["prefers"] }),
            high_stakes_deny_below_trust: None,
            enabled: true,
        }),
    )
    .await
    .unwrap();

    let listed: GovernanceContractList = list_contracts(axum::extract::Extension(ctx.clone()))
        .await
        .unwrap()
        .0;
    assert_eq!(listed.contracts.len(), 1);
    assert_eq!(listed.contracts[0].agent_id, "copilot");

    let wm = w.core.recall(&w.tenant, &q).await.unwrap();
    assert!(
        wm.items.iter().all(|i| i.object != "email-digest"),
        "contract bans user_stated prefers for this agent — pre-ranking hard filter"
    );

    // Disabling the contract restores visibility (same data, policy changed).
    upsert_contract(
        axum::extract::Extension(ctx),
        axum::extract::Path("copilot".to_string()),
        axum::Json(UpsertContractRequest {
            may_believe: serde_json::json!([]),
            must_not_believe_from: serde_json::json!({}),
            high_stakes_deny_below_trust: None,
            enabled: false,
        }),
    )
    .await
    .unwrap();
    let wm = w.core.recall(&w.tenant, &q).await.unwrap();
    assert!(
        wm.items.iter().any(|i| i.object == "email-digest"),
        "disabled contract stops filtering"
    );

    // Non-admin cannot manage contracts.
    let denied = list_contracts(axum::extract::Extension(w.ctx(&alias))).await;
    assert!(
        matches!(denied, Err(backend::error::AppError::Forbidden { .. })),
        "{denied:?}"
    );
}

// ── 用户自助：可改、可删自己的；不可动他人的 ────────────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn self_service_permissions() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP");
        return;
    }
    let w = world("self").await;
    let (alice_alias, alice, alice_subject) = w.person("alice-s").await;
    let (bob_alias, bob, bob_subject) = w.person("bob-s").await;
    w.make_admin(&bob_alias).await; // bob doubles as admin for cross-checks

    w.say(&alice, &alice_subject, "works_at", "SelfCo").await;
    w.say(&bob, &bob_subject, "works_at", "BobCo").await;

    // 可改: Alice corrects HER belief through the same write gate.
    let verdict = self_correct(
        axum::extract::Extension(w.ctx(&alice_alias)),
        axum::Json(backend::routers::memory_governance::SelfCorrectRequest {
            predicate: "works_at".into(),
            object: "NewSelfCo".into(),
            note: Some("typo in company name".into()),
        }),
    )
    .await
    .unwrap()
    .0;
    assert!(
        verdict.decision.contains("Superseded"),
        "equal-rank correction supersedes: {}",
        verdict.decision
    );
    let wm = w
        .core
        .recall(&w.tenant, &RecallQuery::new(&alice_alias, "工作"))
        .await
        .unwrap();
    assert!(wm.items.iter().any(|i| i.object == "NewSelfCo"));

    // Self-correct replay is idempotent: the SAME claim key resolves to the
    // ORIGINAL verdict (Superseded — a replay reports what happened, it does
    // not re-decide), and crucially no new version row appears.
    let history_before = w
        .repo
        .history_for(&w.tenant, &alice_subject, "works_at")
        .await
        .unwrap()
        .len();
    let replay = self_correct(
        axum::extract::Extension(w.ctx(&alice_alias)),
        axum::Json(backend::routers::memory_governance::SelfCorrectRequest {
            predicate: "works_at".into(),
            object: "NewSelfCo".into(),
            note: None,
        }),
    )
    .await
    .unwrap()
    .0;
    assert!(
        replay.decision.contains("Superseded"),
        "{}",
        replay.decision
    );
    let history_after = w
        .repo
        .history_for(&w.tenant, &alice_subject, "works_at")
        .await
        .unwrap()
        .len();
    assert_eq!(history_before, history_after, "replay adds no versions");

    // 可删 (own single belief): Alice archives her own edge.
    let alice_edge = w
        .repo
        .open_edge(&w.tenant, &alice_subject, "works_at")
        .await
        .unwrap()
        .unwrap();
    let ok: GovernanceMutationResult = archive_belief(
        axum::extract::Extension(w.ctx(&alice_alias)),
        axum::extract::Path(alice_edge.id.clone()),
    )
    .await
    .unwrap()
    .0;
    assert!(ok.ok, "users can archive their own beliefs");

    // 可删 (own whole subject): restore one first, then self-forget.
    w.say(&alice, &alice_subject, "lives_in", "SelfCity").await;
    let forgot: GovernanceMutationResult =
        self_forget(axum::extract::Extension(w.ctx(&alice_alias)))
            .await
            .unwrap()
            .0;
    assert!(forgot.ok);
    let wm = w
        .core
        .recall(&w.tenant, &RecallQuery::new(&alice_alias, ""))
        .await
        .unwrap();
    assert!(wm.items.is_empty(), "self-forget archived every open edge");

    // 不可动他人的: Alice cannot archive Bob's belief.
    let bob_edge = w
        .repo
        .open_edge(&w.tenant, &bob_subject, "works_at")
        .await
        .unwrap()
        .unwrap();
    let denied = archive_belief(
        axum::extract::Extension(w.ctx(&alice_alias)),
        axum::extract::Path(bob_edge.id.clone()),
    )
    .await;
    assert!(
        matches!(denied, Err(backend::error::AppError::Forbidden { .. })),
        "{denied:?}"
    );
}

// ── 行为监控接线（源级断言 + 指标注册） ─────────────────────────────────────

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn monitor_wiring_is_real() {
    // The evaluator itself is unit-tested in services::memory_monitor; here we
    // assert the WIRING exists: the worker consumes it and the metric is
    // registered on the /metrics surface with a real caller path.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let worker = std::fs::read_to_string(manifest.join("src/services/consolidation.rs")).unwrap();
    assert!(
        worker.contains("memory_monitor::evaluate") && worker.contains("record_alerts"),
        "consolidation worker must drive the anomaly monitor"
    );
    let exporter_src =
        std::fs::read_to_string(manifest.join("src/services/prometheus_exporter.rs")).unwrap();
    assert!(exporter_src.contains("memory_anomaly_alerts_total"));

    // And the exposition really carries it. A CounterVec with no children
    // emits nothing, so drive the REAL caller path once first — the same
    // record_alerts the worker invokes.
    backend::services::memory_monitor::record_alerts(
        "wiring-test",
        &[backend::services::memory_monitor::AnomalyType::WriteRateSpike],
    );
    let text = backend::services::prometheus_exporter::get_exporter().generate_prometheus_output();
    assert!(
        text.contains("memory_anomaly_alerts_total") && text.contains("write_rate_spike"),
        "metric registered & exposed with its label"
    );
}
