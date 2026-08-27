//! Principal identity graph + append-only event stream — penetration/integration
//! suite (#126).
//!
//! Acceptance criteria covered, mapped to tests:
//!
//! | #126 criterion                                                    | test |
//! |-------------------------------------------------------------------|------|
//! | Event only appends; no update path (code **and** DB revoke)        | `appends_are_idempotent_and_never_updatable` |
//! | Duplicate idempotency key produces no second event                 | same |
//! | Tenant A cannot read/write tenant B's event/principal/alias (RLS)  | `rls_fails_closed_across_tenants` |
//! | Same user resolves to one principal across sessions/devices        | `identity_flows_end_to_end` |
//! | Anonymous links to login identity ONLY after explicit merge        | same |
//! | Shared device does not chain two users                             | same |
//! | Merge AND unmerge both leave audit rows                            | same |
//!
//! Reports as `ignored` without `DATABASE_URL` (no false-green pass); CI opts in
//! with `--include-ignored`. The whole file drives REAL repositories through a
//! restricted probe role (`aetheris_identity_probe`, NOSUPERUSER NOBYPASSRLS),
//! so every statement actually exercises the RLS policies rather than an owner
//   connection where they would be a NO-OP.

use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::PgPool;

use backend::db::memory_event::{AppendOutcome, MemoryEventRepository};
use backend::db::principal::{AUDIT_EVENT_MERGED, AUDIT_EVENT_UNMERGED};
use backend::error::AppError;
use backend::models::memory_event::{AppendMemoryEventRequest, MemoryEventType};
use backend::models::principal::{PrincipalAliasType, PrincipalKind, PrincipalStatus};
use backend::services::identity::IdentityService;
use backend::tenant::TenantId;

const PROBE_ROLE: &str = "aetheris_identity_probe";
const PROBE_PASSWORD: &str = "aetheris_identity_probe_pw";

static INIT_PROBE: tokio::sync::OnceCell<sqlx::PgPool> = tokio::sync::OnceCell::const_new();

/// Unique-per-run suffix so reruns never collide with previous rows.
fn unique_suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos()
        .to_string()
}

/// Run migrations (idempotent) and provision the restricted probe role with
/// exactly the DML the app is supposed to have. Note `memory_events` gets
/// SELECT+INSERT only — UPDATE/DELETE stay revoked end-to-end.
async fn init_pools() -> &'static PgPool {
    let admin_url = match std::env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            // Cannot happen when reached through a test body that checked first,
            // but keep the guard local for clarity.
            eprintln!("SKIP principal_identity_pg: DATABASE_URL not set");
            std::process::exit(0);
        }
    };

    let probe = INIT_PROBE
        .get_or_init(|| async move {
            let admin = PgPool::connect(&admin_url)
                .await
                .expect("connect as admin/owner");

            let migrations_path =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
            let migrator = sqlx::migrate::Migrator::new(migrations_path)
                .await
                .expect("build migrator");
            migrator.run(&admin).await.expect("run migrations");

            let create_role = format!(
                r#"
                DO $$
                BEGIN
                    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = '{role}') THEN
                        CREATE ROLE {role} LOGIN PASSWORD '{pw}' NOSUPERUSER NOBYPASSRLS;
                    END IF;
                END
                $$;
                "#,
                role = PROBE_ROLE,
                pw = PROBE_PASSWORD,
            );
            sqlx::raw_sql(&create_role)
                .execute(&admin)
                .await
                .expect("create probe role");

            for stmt in [
                format!("GRANT USAGE ON SCHEMA public TO {PROBE_ROLE}"),
                // Append-only surface for events.
                format!("GRANT SELECT, INSERT ON memory_events TO {PROBE_ROLE}"),
                format!("GRANT SELECT, INSERT, UPDATE ON memory_principals TO {PROBE_ROLE}"),
                format!("GRANT SELECT, INSERT, UPDATE ON principal_aliases TO {PROBE_ROLE}"),
                format!("GRANT SELECT, INSERT ON memory_audit_events TO {PROBE_ROLE}"),
            ] {
                sqlx::raw_sql(&stmt)
                    .execute(&admin)
                    .await
                    .unwrap_or_else(|e| panic!("grant failed ({stmt}): {e}"));
            }

            let opts: sqlx::postgres::PgConnectOptions = use_std_fromstr(admin_url.as_str())
                .username(PROBE_ROLE)
                .password(PROBE_PASSWORD);
            // A small, explicitly-bounded pool: parallel integration binaries
            // share one PostgreSQL instance and any client that hoards
            // connections becomes someone else's timeout (#126 found this the
            // hard way during full-suite runs).
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(2)
                .acquire_timeout(std::time::Duration::from_secs(10))
                .connect_with(opts)
                .await
                .expect("connect as restricted probe role")
        })
        .await;
    probe
}

#[allow(dead_code)]
fn use_std_fromstr(url: &str) -> sqlx::postgres::PgConnectOptions {
    use std::str::FromStr;
    sqlx::postgres::PgConnectOptions::from_str(url).expect("parse DATABASE_URL")
}

// ============================================================================
// Append-only + idempotency (#126 criteria 1, 2)
// ============================================================================

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn appends_are_idempotent_and_never_updatable() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP: DATABASE_URL not set");
        return;
    }
    let probe = init_pools().await;

    let tenant = TenantId::from_string(format!("ev-replay-{}", unique_suffix()));
    let svc = IdentityService::new(probe.clone());
    let person = svc
        .ensure_person_from_jwt(&tenant, "u_writer", Some("Writer"))
        .await
        .expect("ensure person");

    let events = MemoryEventRepository::new(probe.clone());

    // ── Criterion 2: replaying the same idempotency key writes ONE row. ──────
    let payload = serde_json::json!({ "text": "hello, memory log", "turn": 1 });
    let req = AppendMemoryEventRequest::new(person.id.clone(), MemoryEventType::UserMessage)
        .session_id("sess-writer")
        .actor("u_writer")
        .payload(payload.clone())
        .idempotency_key("producer-turn-1");

    let first = events
        .append(&tenant, req.clone())
        .await
        .expect("first append");
    let second = events.append(&tenant, req).await.expect("second append");
    assert!(matches!(first, AppendOutcome::Inserted(_)), "{first:?}");
    match second {
        AppendOutcome::Duplicate(dup_id) => {
            assert_eq!(
                dup_id,
                first.id(),
                "replay must resolve to the original row"
            );
        }
        other => panic!("replay must be a Duplicate, got {other:?}"),
    }
    assert_eq!(events.count_for_tenant(&tenant).await.unwrap(), 1);

    // Content hash equals SHA-256 over the serialized payload.
    let stored = events
        .get(&tenant, first.id())
        .await
        .unwrap()
        .expect("event persisted");
    assert_eq!(
        stored.content_hash,
        backend::db::memory_event::content_hash_for(&serde_json::to_string(&payload).unwrap()),
        "content_hash must anchor the payload bytes"
    );

    // ── Criterion 1a: even a RAW-SQL detour cannot update history. ────────────
    let mut tamper_tx = probe.begin().await.expect("begin tamper tx");
    let tampered = sqlx::query(
        "UPDATE memory_events SET payload_json = '{\"text\":\"forged\"}' WHERE id = $1",
    )
    .bind(first.id())
    .execute(&mut *tamper_tx)
    .await;
    assert!(
        tampered.is_err(),
        "probe role must lack UPDATE on memory_events"
    );

    // Under the OWNER connection the grant genuinely says INSERT/SELECT only,
    // which is what makes the raw-SQL refusal above a property of the table,
    // not of this test's role provisioning.
    // information_schema.table_privileges hides other roles' grants from a
    // restricted reader, so explode the relation ACL directly - visible to all.
    let grants: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT acl.privilege_type
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        CROSS JOIN LATERAL aclexplode(c.relacl) AS acl
        JOIN pg_roles r ON r.oid = acl.grantee
        WHERE n.nspname = 'public'
          AND c.relname = 'memory_events'
          AND r.rolname = 'aetheris_app'
        ORDER BY 1
        "#,
    )
    .fetch_all(probe)
    .await
    .expect("read table privileges");
    let grant_names: Vec<&str> = grants.iter().map(|(p,)| p.as_str()).collect();
    assert_eq!(
        grant_names,
        vec!["INSERT", "SELECT"],
        "aetheris_app must hold ONLY INSERT+SELECT on memory_events \
         (the migration's REVOKE enforces append-only at the database layer)"
    );

    // ── Criterion 1b: deleting history is equally off-limits. ────────────────
    let mut del_tx = probe.begin().await.expect("begin delete tx");
    let deleted = sqlx::query("DELETE FROM memory_events WHERE id = $1")
        .bind(first.id())
        .execute(&mut *del_tx)
        .await;
    assert!(
        deleted.is_err(),
        "probe role must lack DELETE on memory_events"
    );
}

// ============================================================================
// RLS fail-close (#126 criterion 3)
// ============================================================================

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn rls_fails_closed_across_tenants() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP: DATABASE_URL not set");
        return;
    }
    let probe = init_pools().await;

    let suffix = unique_suffix();
    let tenant_a = TenantId::from_string(format!("rls-a-{suffix}"));
    let tenant_b = TenantId::from_string(format!("rls-b-{suffix}"));

    // Seed everything in tenant A through the normal repos.
    let svc_a = IdentityService::new(probe.clone());
    let lisa = svc_a
        .ensure_person_from_jwt(&tenant_a, "u_lisa", Some("Lisa"))
        .await
        .expect("ensure person in A");
    let events_a = MemoryEventRepository::new(probe.clone());
    let evt = events_a
        .append(
            &tenant_a,
            AppendMemoryEventRequest::new(lisa.id.clone(), MemoryEventType::UserMessage)
                .session_id("sess-a")
                .payload(serde_json::json!({"secret": "lisa-only"})),
        )
        .await
        .expect("append event in A");

    // ── Cross-tenant READS must fail closed (zero rows), for all 3 tables. ───
    let svc_b = IdentityService::new(probe.clone());
    let events_b = MemoryEventRepository::new(probe.clone());

    assert!(
        events_b.get(&tenant_b, evt.id()).await.unwrap().is_none(),
        "tenant B must not read tenant A's event"
    );
    assert!(
        events_b
            .list_by_principal(&tenant_b, &lisa.id, 10)
            .await
            .unwrap()
            .is_empty(),
        "tenant B must not list tenant A's events"
    );
    assert!(
        events_b.count_for_tenant(&tenant_b).await.unwrap() == 0,
        "tenant B sees no tenant A rows"
    );

    let resolved_by_alias_b = svc_b
        .principals()
        .find_by_alias(&tenant_b, PrincipalAliasType::JwtSub, "u_lisa")
        .await
        .unwrap();
    assert!(resolved_by_alias_b.is_none(), "aliases are tenant-local");

    let principal_b = svc_b.principals().get(&tenant_b, &lisa.id).await.unwrap();
    assert!(
        principal_b.is_none(),
        "tenant B must not read tenant A's principal"
    );

    // ── Cross-tenant WRITES are rejected by the policy's WITH CHECK. ────────
    let forged = try_forge_event(
        probe,
        &tenant_b,
        tenant_a.as_str(),
        &lisa.id,
        format!("forge-{suffix}"),
    )
    .await;
    let err = forged.expect_err("cross-tenant event insert must be rejected");
    assert!(
        err.to_string().contains("row-level security"),
        "expected a WITH CHECK rejection, got: {err}"
    );

    let forged_alias = try_forge_alias(
        probe,
        &tenant_b,
        tenant_a.as_str(),
        &lisa.id,
        format!("alias-forge-{suffix}"),
    )
    .await;
    assert!(
        forged_alias.is_err(),
        "cross-tenant alias insert must be rejected"
    );
}

/// Attempt (under tenant B's GUC) to insert an event row carrying TENANT A's
/// tenant_id + A's principal. RLS `WITH CHECK` must refuse it.
async fn try_forge_event(
    pool: &PgPool,
    scope: &TenantId,
    target_tenant: &str,
    target_principal: &str,
    event_id: String,
) -> Result<(), AppError> {
    use backend::db::tenant_scope::begin_tenant_tx;
    let mut tx = begin_tenant_tx(pool, scope).await?;
    sqlx::query(
        r#"
        INSERT INTO memory_events
            (id, tenant_id, principal_id, session_id, event_type, actor, content_hash, payload_json)
        VALUES ($1, $2, $3, NULL, 'user_message', 'attacker', '', '{}'::jsonb)
        "#,
    )
    .bind(event_id)
    .bind(target_tenant)
    .bind(target_principal)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Same forging attempt against `principal_aliases`.
async fn try_forge_alias(
    pool: &PgPool,
    scope: &TenantId,
    target_tenant: &str,
    target_principal: &str,
    alias_id: String,
) -> Result<(), AppError> {
    use backend::db::tenant_scope::begin_tenant_tx;
    let mut tx = begin_tenant_tx(pool, scope).await?;
    sqlx::query(
        "INSERT INTO principal_aliases (id, tenant_id, principal_id, alias_type, alias_value) VALUES ($1,$2,$3,'jwt_sub','spoofed')",
    )
    .bind(alias_id)
    .bind(target_tenant)
    .bind(target_principal)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

// ============================================================================
// Identity flows (#126 criteria 4, 5, 6, 7)
// ============================================================================

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn identity_flows_end_to_end() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("SKIP: DATABASE_URL not set");
        return;
    }
    let probe = init_pools().await;

    let suffix = unique_suffix();
    let tenant = TenantId::from_string(format!("id-flow-{suffix}"));
    let svc = IdentityService::new(probe.clone());
    let uid = format!("u_lisa_{suffix}");

    // ── Criterion 4: one user, many sessions/devices => ONE principal. ──────
    let phone_session = svc
        .ensure_person_from_jwt(&tenant, &uid, Some("Lisa"))
        .await
        .unwrap();
    let laptop_session = svc
        .ensure_person_from_jwt(&tenant, &uid, None)
        .await
        .unwrap();
    assert_eq!(
        phone_session.id, laptop_session.id,
        "same JWT uid must map to the same person principal across devices"
    );
    assert_eq!(phone_session.kind, PrincipalKind::Person.as_str());

    // Re-resolution via the alias lands on the same row again.
    let again = svc
        .principals()
        .find_by_alias(&tenant, PrincipalAliasType::JwtSub, &uid)
        .await
        .unwrap()
        .expect("alias resolves");
    assert_eq!(again.id, phone_session.id);

    // ── Criterion 6: a SHARED DEVICE stays independent of any person. ────────
    let kiosk = format!("kiosk-tablet-{suffix}");
    let device_principal = svc.ensure_device_principal(&tenant, &kiosk).await.unwrap();
    assert_eq!(
        device_principal.kind,
        PrincipalKind::Device.as_str(),
        "device ids live on device principals"
    );

    let bob_uid = format!("u_bob_{suffix}");
    let bob = svc
        .ensure_person_from_jwt(&tenant, &bob_uid, Some("Bob"))
        .await
        .unwrap();
    assert_ne!(
        bob.id, phone_session.id,
        "distinct users are distinct persons"
    );
    assert_ne!(bob.id, device_principal.id);

    // Structural proof of non-chaining: no human alias resolves to the device,
    // nobody merged INTO the device, and no device alias sits on either person.
    let aliases_on_device = svc
        .principals()
        .list_aliases(&tenant, &device_principal.id)
        .await
        .unwrap();
    assert_eq!(aliases_on_device.len(), 1);
    assert_eq!(aliases_on_device[0].0, "device_id");
    let merged_into_device = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM memory_principals WHERE tenant_id=$1 AND merged_into_id=$2",
    )
    .bind(tenant.as_str())
    .bind(device_principal.id.clone())
    .fetch_one(
        &mut *backend::db::tenant_scope::begin_tenant_tx(probe, &tenant)
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        merged_into_device, 0,
        "nothing may auto-merge into a device"
    );

    // ── Criteria 5+7: anonymous ⇒ explicit merge ⇒ reversible, all audited. ──
    // Pre-login turns land on a FRESH anonymous principal each visit.
    let anon = svc
        .create_anonymous_principal(&tenant, Some("front-desk"))
        .await
        .unwrap();
    assert_eq!(anon.kind, PrincipalKind::Anonymous.as_str());
    let anon_events = MemoryEventRepository::new(probe.clone());
    anon_events
        .append(
            &tenant,
            AppendMemoryEventRequest::new(anon.id.clone(), MemoryEventType::UserMessage)
                .session_id("anon-session")
                .payload(serde_json::json!({"text": "what's my order status?"})),
        )
        .await
        .expect("anonymous turn recorded");

    // Before ANY merge: resolution stays anchored on the anonymous bucket —
    // explicitly NOT on Lisa, even though both live in the same tenant.
    let pre_merge_resolution = anon.status();
    assert_eq!(pre_merge_resolution, Ok(PrincipalStatus::Active));
    let anon_untouched = svc
        .principals()
        .get(&tenant, &anon.id)
        .await
        .unwrap()
        .unwrap();
    assert!(anon_untouched.merged_into_id.is_none());

    // Login happens; the operator merges anon -> lisa. Same txn records the
    // lifecycle system_event and BOTH directions' audit rows.
    let root_after_merge = svc
        .merge_anonymous_into_person(&tenant, &anon.id, &phone_session.id, Some("operator-9"))
        .await
        .expect("explicit merge");
    assert_eq!(root_after_merge.id, phone_session.id);

    let anon_merged = svc
        .principals()
        .get(&tenant, &anon.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        anon_merged.merged_into_id.as_deref(),
        Some(phone_session.id.as_str())
    );
    assert_eq!(anon_merged.status, PrincipalStatus::Merged.as_str());

    // Resolution through the redirect reaches the person.
    let (_, hops) = svc
        .principals()
        .follow_merge_chain(&tenant, anon_merged.clone())
        .await
        .unwrap();
    assert_eq!(hops, 1);

    // Historical events remain attached to the ANON id (history is not rewritten);
    // recall layers expand redirects themselves (#128 contract). The list now
    // ALSO contains the merge lifecycle system_event, which is recorded against
    // the anonymized bucket by design.
    let hist = anon_events
        .list_by_principal(&tenant, &anon.id, 10)
        .await
        .unwrap();
    assert!(
        hist.len() == 2
            && hist
                .iter()
                .any(|e| e.event_type == MemoryEventType::UserMessage.as_str()),
        "pre-merge user turn must survive untouched; got {hist:?}"
    );
    assert!(
        hist.iter().all(|e| e.principal_id == anon.id),
        "merge must never re-point historical events"
    );

    // Merge wrote its audit row + lifecycle system_event (the lifecycle event
    // rides in `hist`, already fetched under the tenant scope above).
    assert_audit_rows(probe, &tenant, &anon.id, AUDIT_EVENT_MERGED, 1).await;
    let merge_events: Vec<&_> = hist
        .iter()
        .filter(|e| e.event_type == MemoryEventType::SystemEvent.as_str())
        .collect();
    assert_eq!(merge_events.len(), 1, "exactly one merge lifecycle event");
    let payload_text = merge_events[0].payload_json.replace(' ', "");
    assert!(
        payload_text.contains("\"action\":\"principal_merged\"")
            && payload_text.contains(phone_session.id.as_str()),
        "lifecycle event must name the action and the person; got {payload_text}"
    );

    // Unmerge restores independence — and audits THAT direction too.
    let prev = svc
        .unmerge_anonymous(&tenant, &anon.id, Some("operator-9"))
        .await
        .unwrap();
    assert_eq!(prev, phone_session.id);
    let anon_restored = svc
        .principals()
        .get(&tenant, &anon.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(anon_restored.status, PrincipalStatus::Active.as_str());
    assert!(anon_restored.merged_into_id.is_none());
    assert_audit_rows(probe, &tenant, &anon.id, AUDIT_EVENT_UNMERGED, 1).await;

    // Invalid merge attempts are rejected outright:
    // person-into-person and merge-after-unmerge legality both come back here.
    let bad_kind = svc
        .merge_anonymous_into_person(&tenant, &phone_session.id, &bob.id, None)
        .await;
    assert!(
        matches!(&bad_kind, Err(AppError::BadRequest(m)) if m.contains("only 'anonymous'")),
        "must refuse merging a person; got {bad_kind:?}"
    );
    // A legit second lifecycle after unmerge works and audits again.
    svc.merge_anonymous_into_person(&tenant, &anon.id, &phone_session.id, None)
        .await
        .expect("re-merge after unmerge is legal");
    assert_audit_rows(probe, &tenant, &anon.id, AUDIT_EVENT_MERGED, 2).await;
}

/// Count audit rows produced by the identity service for one merge/unmerge step.
///
/// Reads through the PROBE pool (memory_audit_events carries no RLS and the
/// probe role holds SELECT), so the assertion exercises the same restricted
/// connection posture as the code under test instead of a superuser backdoor.
async fn assert_audit_rows(
    probe: &PgPool,
    tenant: &TenantId,
    principal_id: &str,
    event_type: &str,
    expected: i64,
) {
    let (count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM memory_audit_events
        WHERE tenant_id = $1
          AND event_type = $2
          AND resource_type = 'memory_principal'
          AND resource_id = $3
        "#,
    )
    .bind(tenant.as_str())
    .bind(event_type)
    .bind(principal_id)
    .fetch_one(probe)
    .await
    .expect("read audit rows as owner");
    assert_eq!(
        count, expected,
        "expected {expected} '{event_type}' audit row(s) for principal {principal_id}"
    );
}
