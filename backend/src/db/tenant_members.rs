//! `tenant_members` / `tenants` repository — the persistent role store (C-3, ADR-0009 方案 A).
//!
//! Before this module, roles lived only in a process-memory `HashMap` inside
//! `services::rbac::RbacService`: nothing was persisted, nothing was seeded, and
//! `assign_role` was silently lost on restart. This is the first durable answer to
//! "who holds what role in which org".
//!
//! ## Two scoping modes, deliberately
//!
//! Reads split by which question is being asked:
//!
//! - **"what is this user's role in THIS org"** → [`begin_tenant_tx`]. The tenant
//!   GUC is set, the tenant-keyed RLS policy applies, and a caller claiming an org
//!   they are not in simply finds no row → denied. Fail-closed by construction.
//! - **"which orgs is this user in"** → [`begin_user_tx`]. That question is a read
//!   across tenants for one user and cannot be answered under a tenant-keyed
//!   policy — you would need the org id to discover the org ids. The
//!   `*_self_membership` policies exist for exactly this and are `FOR SELECT`
//!   only, so this path can never write.
//!
//! Queries use the runtime `sqlx::query*` API rather than the compile-time macros,
//! matching `db::tenant_scope`. That keeps the crate buildable with
//! `SQLX_OFFLINE=true` without regenerating the `.sqlx` cache, at the documented
//! cost that `cargo check` does **not** validate this SQL — the PG-gated tests do.

use sqlx::Row;

use crate::db::pool;
use crate::db::tenant_scope::{begin_tenant_tx, begin_user_tx};
use crate::error::AppError;
use crate::services::rbac::Role;
use crate::tenant::TenantId;

/// One membership row, as stored.
///
/// `role` is kept as `String`, not `Role`. Decoding straight into the enum would
/// make the whole query fail on a single row written by a future version with a
/// wider CHECK set; callers convert explicitly via [`Role::from_db_str`], which
/// fails closed per row. Same constraint recorded for the other CHECK-backed
/// enums in backlog D-k.
#[derive(Debug, Clone)]
pub struct TenantMemberRow {
    pub tenant_id: String,
    pub user_id: String,
    pub role: String,
    pub assigned_by: Option<String>,
}

/// One org the caller belongs to, for the switch-org listing.
#[derive(Debug, Clone)]
pub struct Membership {
    pub tenant_id: String,
    pub tenant_name: String,
    pub role: String,
}

/// The caller's role in `tenant_id`, or `None` if they hold no membership there.
///
/// `None` is the deny answer and covers three cases that are indistinguishable to
/// the caller on purpose: no such org, not a member, or a role string this build
/// does not recognise. Distinguishing them would turn the check into a
/// membership oracle.
pub async fn role_for(tenant_id: &TenantId, user_id: &str) -> Result<Option<Role>, AppError> {
    let mut tx = begin_tenant_tx(pool(), tenant_id).await?;

    let row = sqlx::query("SELECT role FROM tenant_members WHERE tenant_id = $1 AND user_id = $2")
        .bind(tenant_id.as_str())
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read tenant membership: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to commit membership read: {e}")))?;

    let Some(row) = row else {
        return Ok(None);
    };
    let stored: String = row.get("role");

    match Role::from_db_str(&stored) {
        Some(role) => Ok(Some(role)),
        None => {
            // Data this build cannot interpret must not become an access grant.
            tracing::error!(
                tenant = %tenant_id,
                user = %user_id,
                stored_role = %stored,
                "tenant_members.role holds a value this build does not recognise; \
                 treating as no membership (deny)"
            );
            Ok(None)
        }
    }
}

/// Every member of `tenant_id`. Scoped by the tenant GUC, so a caller can only
/// ever enumerate the org they are already scoped to.
pub async fn list_members(tenant_id: &TenantId) -> Result<Vec<TenantMemberRow>, AppError> {
    let mut tx = begin_tenant_tx(pool(), tenant_id).await?;

    let rows = sqlx::query(
        "SELECT tenant_id, user_id, role, assigned_by \
         FROM tenant_members WHERE tenant_id = $1 ORDER BY user_id",
    )
    .bind(tenant_id.as_str())
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to list tenant members: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to commit member list: {e}")))?;

    Ok(rows
        .into_iter()
        .map(|r| TenantMemberRow {
            tenant_id: r.get("tenant_id"),
            user_id: r.get("user_id"),
            role: r.get("role"),
            assigned_by: r.get("assigned_by"),
        })
        .collect())
}

/// Create or update a membership.
///
/// Runs under the tenant GUC, so the tenant-keyed policy's `WITH CHECK` applies:
/// a caller scoped to org A physically cannot write a row for org B, independent
/// of whatever the handler above believed. The `role` bind is the enum's `Display`
/// form, which is what the migration's CHECK accepts.
pub async fn upsert_member(
    tenant_id: &TenantId,
    user_id: &str,
    role: Role,
    assigned_by: &str,
) -> Result<(), AppError> {
    let mut tx = begin_tenant_tx(pool(), tenant_id).await?;

    sqlx::query(
        "INSERT INTO tenant_members (tenant_id, user_id, role, assigned_by) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (tenant_id, user_id) \
         DO UPDATE SET role = EXCLUDED.role, \
                       assigned_by = EXCLUDED.assigned_by, \
                       assigned_at = now()",
    )
    .bind(tenant_id.as_str())
    .bind(user_id)
    .bind(role.to_string())
    .bind(assigned_by)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to upsert tenant member: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to commit member upsert: {e}")))?;

    Ok(())
}

/// Every org `user_id` belongs to, with the org name and their role in it.
///
/// Uses [`begin_user_tx`], so only the `*_self_membership` policies are in play:
/// the caller sees their own rows and nothing else, and cannot write. This is the
/// backing query for the switch-org listing.
pub async fn memberships_for_user(user_id: &str) -> Result<Vec<Membership>, AppError> {
    let mut tx = begin_user_tx(pool(), user_id).await?;

    let rows = sqlx::query(
        "SELECT m.tenant_id, t.name AS tenant_name, m.role \
         FROM tenant_members m \
         JOIN tenants t ON t.tenant_id = m.tenant_id \
         WHERE m.user_id = $1 \
         ORDER BY t.name",
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to list memberships: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to commit membership list: {e}")))?;

    Ok(rows
        .into_iter()
        .map(|r| Membership {
            tenant_id: r.get("tenant_id"),
            tenant_name: r.get("tenant_name"),
            role: r.get("role"),
        })
        .collect())
}

/// Whether `user_id` may act as `tenant_id` — the switch-org gate.
///
/// Deliberately routed through [`begin_user_tx`] rather than the tenant GUC: the
/// caller is asking to *enter* an org they are not yet scoped to, so scoping the
/// check by the org they claim would make the question answer itself.
pub async fn is_member(user_id: &str, tenant_id: &str) -> Result<bool, AppError> {
    let mut tx = begin_user_tx(pool(), user_id).await?;

    let row = sqlx::query(
        "SELECT 1 AS ok FROM tenant_members WHERE user_id = $1 AND tenant_id = $2 LIMIT 1",
    )
    .bind(user_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to check membership: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to commit membership check: {e}")))?;

    Ok(row.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrecognised_stored_role_is_not_an_access_grant() {
        // The deny path in `role_for` hinges on this returning None rather than
        // falling back to a role. A `Reader` default would hand out read access on
        // corrupt or future-version data.
        assert_eq!(Role::from_db_str("superuser"), None);
        assert_eq!(Role::from_db_str("Owner"), None, "match must be exact-case");
        assert_eq!(Role::from_db_str(""), None);
    }

    #[test]
    fn every_role_round_trips_through_its_stored_form() {
        for role in Role::ALL {
            assert_eq!(
                Role::from_db_str(&role.to_string()),
                Some(role),
                "{role:?} does not survive a write/read round trip"
            );
        }
    }

    /// The two scoping modes must not be interchangeable. `is_member` and
    /// `memberships_for_user` answer cross-tenant questions and must use the user
    /// GUC; everything else must use the tenant GUC, whose policy is the only one
    /// that permits writes.
    #[test]
    fn cross_tenant_reads_use_the_user_guc_and_writes_use_the_tenant_guc() {
        let src = include_str!("tenant_members.rs");
        let body_of = |name: &str| -> String {
            let start = src
                .find(&format!("pub async fn {name}("))
                .unwrap_or_else(|| panic!("{name} must exist"));
            let rest = &src[start..];
            let end = rest.find("\n}\n").unwrap_or(rest.len());
            rest[..end].to_string()
        };

        for name in ["memberships_for_user", "is_member"] {
            let body = body_of(name);
            assert!(
                body.contains("begin_user_tx") && !body.contains("begin_tenant_tx"),
                "{name} answers a cross-tenant question and must use begin_user_tx"
            );
        }

        for name in ["role_for", "list_members", "upsert_member"] {
            let body = body_of(name);
            assert!(
                body.contains("begin_tenant_tx") && !body.contains("begin_user_tx"),
                "{name} must be tenant-scoped; the self-membership policies are \
                 FOR SELECT only and would silently drop its RLS protection"
            );
        }
    }
}
