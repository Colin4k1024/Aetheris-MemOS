//! Per-request tenant-scoped transaction executor — the RLS keystone (P1 PR-1).
//!
//! PostgreSQL Row-Level Security enforces tenant isolation via a
//! transaction-local GUC, `aetheris.tenant_id`, which policies read with
//! `current_setting('aetheris.tenant_id', true)`. This module opens a transaction
//! and sets that GUC so every RLS-protected query executed inside it is scoped to
//! the tenant at the *database* layer — instead of relying on application-layer
//! `t:{tenant}:` prefix checks that any missed query path can bypass.
//!
//! See `docs/artifacts/2026-07-16-enterprise-productionization/p1-execution-runbook.md`
//! (PR-1) and ADR-0001. Repositories are being migrated to take an executor from
//! here instead of the global `pool()`. Uses the runtime `sqlx::query` API, so it
//! does not depend on the compile-time `.sqlx` cache.

use sqlx::{PgPool, Postgres, Transaction};

use crate::error::AppError;
use crate::tenant::TenantId;

/// GUC key RLS policies read via `current_setting(TENANT_GUC, true)`.
///
/// Must be dotted/namespaced so PostgreSQL accepts `set_config` on a custom
/// parameter without it being declared in `postgresql.conf`.
pub const TENANT_GUC: &str = "aetheris.tenant_id";

/// Begin a transaction with the tenant GUC set transaction-locally.
///
/// Every RLS-protected statement executed on the returned transaction is scoped to
/// `tenant_id`. The caller runs queries on `&mut *tx` and must `commit()` on
/// success (dropping the transaction rolls back). The GUC is set with
/// `is_local = true`, so it is automatically cleared when the transaction ends and
/// never leaks to other requests sharing the pooled connection.
pub async fn begin_tenant_tx<'a>(
    pool: &'a PgPool,
    tenant_id: &TenantId,
) -> Result<Transaction<'a, Postgres>, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to begin tenant transaction: {e}")))?;

    sqlx::query("SELECT set_config($1, $2, true)")
        .bind(TENANT_GUC)
        .bind(tenant_id.as_str())
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to set tenant GUC: {e}")))?;

    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::TENANT_GUC;

    #[test]
    fn tenant_guc_key_is_namespaced() {
        // A custom GUC must be dotted, else Postgres rejects set_config on it.
        assert!(TENANT_GUC.contains('.'), "GUC key must be namespaced");
        assert_eq!(TENANT_GUC, "aetheris.tenant_id");
    }
}
