//! PR-1 integration test — verifies the tenant GUC executor against a real
//! PostgreSQL. Skips (does not fail) when `DATABASE_URL` is unset, so the offline
//! `cargo test` stays green; run it as the CI gate with a live PG:
//!
//!   DATABASE_URL=postgres://memory:memory@localhost:5432/memory \
//!     cargo test --test tenant_scope_pg

use backend::db::tenant_scope::{begin_tenant_tx, TENANT_GUC};
use backend::tenant::TenantId;

#[tokio::test]
async fn tenant_guc_is_set_within_tx_and_does_not_leak() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP tenant_guc test: DATABASE_URL not set");
        return;
    };

    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("connect to postgres");

    let tenant = TenantId::from_string("acme-corp");
    let mut tx = begin_tenant_tx(&pool, &tenant)
        .await
        .expect("begin tenant transaction");

    // Inside the transaction the GUC reflects the tenant.
    let (value,): (Option<String>,) = sqlx::query_as("SELECT current_setting($1, true)")
        .bind(TENANT_GUC)
        .fetch_one(&mut *tx)
        .await
        .expect("read tenant GUC inside tx");
    assert_eq!(
        value.as_deref(),
        Some("acme-corp"),
        "GUC must reflect the tenant inside its transaction"
    );

    tx.rollback().await.ok();

    // Outside the transaction the local GUC is gone (NULL/empty) — no leak.
    let (leaked,): (Option<String>,) = sqlx::query_as("SELECT current_setting($1, true)")
        .bind(TENANT_GUC)
        .fetch_one(&pool)
        .await
        .expect("read tenant GUC after tx");
    assert!(
        leaked.as_deref().unwrap_or("").is_empty(),
        "transaction-local tenant GUC must not leak to other queries"
    );
}
