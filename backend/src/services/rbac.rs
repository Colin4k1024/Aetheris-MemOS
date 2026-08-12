//! RBAC (Role-Based Access Control) Service
//!
//! Roles are stored in `tenant_members` (see
//! `migrations/20260812000100_org_tenant_model.sql`) and read through
//! [`crate::db::tenant_members`]. **No membership row means denied** — there is no
//! fallback role and no implicit grant.
//!
//! ## What changed, and why the old behaviour was misleading (C-3 / ADR-0009 方案 A)
//!
//! Until this change roles lived in a process-memory `HashMap` and
//! `blocking_has_permission` **auto-granted `Owner`** whenever
//! `tenant_id == user_id`. Because `RequestTenantContext` hard-wired
//! `tenant_id = user_id`, that condition held for every authenticated caller, so
//! the role check was constant-true and RBAC differentiated nothing. Two further
//! consequences were easy to miss:
//!
//! - Nothing was persisted or seeded, so `assign_role` was silently lost on
//!   restart — the role-management API existed but did not take effect.
//! - Two entry points disagreed. `blocking_has_permission` auto-granted, while
//!   the async `get_role` did not, so `routers/mcp.rs` (which used the latter with
//!   a `Reader` fallback) saw a *different* role for the same caller than
//!   `hoops/governance.rs` did.
//!
//! Both are gone. There is now one lookup, one source of truth, and it fails
//! closed.
//!
//! ## Non-PostgreSQL backends
//!
//! [`RbacService::get_role`] cannot query `tenant_members` when the process is
//! running on the SQLite fallback, and `db::pool()` panics there. That path keeps
//! the old self-tenant `Owner` grant — see the function's own documentation for
//! why that is the correct trade-off in a backend that has no DB-layer tenant
//! isolation at all and requires an explicit opt-in to enable.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use tracing::info;

/// Global RBAC service singleton.
///
/// Shared across the enterprise hook set and the tenant router so that role
/// assignments made through the API are visible to governance pre-hooks.
static RBAC_SERVICE: OnceLock<Arc<RbacService>> = OnceLock::new();

/// Initialize the global RBAC service. Optional — the service auto-initializes
/// on first access via [`get_rbac_service`]. Calling this before first access
/// seeds the singleton with a fresh instance and returns whether it was set.
pub fn init_rbac_service() -> bool {
    RBAC_SERVICE.set(Arc::new(RbacService::new())).is_ok()
}

/// Get the global RBAC service instance. The underlying [`RbacService`] is
/// shared across all callers, so role mutations made through one handle are
/// immediately visible to all other handles.
pub fn get_rbac_service() -> Arc<RbacService> {
    RBAC_SERVICE
        .get_or_init(|| Arc::new(RbacService::new()))
        .clone()
}

/// Roles in the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Admin,
    Member,
    Reader,
}

impl Role {
    /// Check if role has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        match self {
            Role::Owner => true, // Owners have all permissions
            Role::Admin => !matches!(
                permission,
                Permission::DeleteTenant | Permission::ManageBilling
            ),
            Role::Member => matches!(
                permission,
                Permission::Read
                    | Permission::Write
                    | Permission::Delete
                    | Permission::Manage
                    | Permission::ManageMemory
                    | Permission::ManageAgents
            ),
            Role::Reader => matches!(permission, Permission::Read),
        }
    }

    /// Get role hierarchy level (higher = more permissions)
    pub fn level(&self) -> u8 {
        match self {
            Role::Owner => 3,
            Role::Admin => 2,
            Role::Member => 1,
            Role::Reader => 0,
        }
    }

    /// Every role, in the same order as the `tenant_members.role` CHECK clause.
    ///
    /// The order is load-bearing: `anti_drift_tenant_member_role_matches_migration_check`
    /// (in `models/memory_enums.rs`, where the other enum↔migration guards live)
    /// compares this sequence against the migration text.
    pub const ALL: [Role; 4] = [Role::Owner, Role::Admin, Role::Member, Role::Reader];

    /// Parse a role as stored in `tenant_members.role`.
    ///
    /// Returns `None` for anything unrecognised rather than defaulting. The
    /// caller is making an authorization decision, so an unparseable value must
    /// fail **closed** — defaulting to `Reader` would silently grant read access
    /// on corrupt data, and defaulting to anything higher is obviously worse.
    ///
    /// The stored column is `TEXT` and this conversion is explicit and fallible,
    /// rather than mapping the column straight onto the enum in a row struct: a
    /// row carrying a value written by a future (or buggy) version must not make
    /// the whole query fail to decode. Same constraint recorded for the other
    /// CHECK-backed enums in backlog D-k.
    pub fn from_db_str(value: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.to_string() == value)
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Owner => write!(f, "owner"),
            Role::Admin => write!(f, "admin"),
            Role::Member => write!(f, "member"),
            Role::Reader => write!(f, "reader"),
        }
    }
}

/// Permissions in the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Read,
    Write,
    Delete,
    Manage,
    ManageMemory,
    ManageAgents,
    ManageTenant,
    ManageBilling,
    DeleteTenant,
}

/// User role assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRole {
    pub user_id: String,
    pub tenant_id: String,
    pub role: Role,
    pub assigned_at: i64,
    pub assigned_by: String,
}

/// Maps a governance [`Operation`] to the RBAC [`Permission`] it requires.
///
/// Lives here, next to `Permission`, because **both** enforcement planes consult
/// it: `hoops::governance` for the REST data plane and `a2a::handler` for A2A
/// skills. It used to exist only in the A2A handler while the REST hook chain used
/// a separate string-keyed mapping that defaulted unknown actions to `Read` — two
/// mappings for one decision, one of which silently under-classified.
///
/// Exhaustive on purpose: a new `Operation` variant fails to COMPILE until its
/// permission is decided here, so no operation class can slip through unguarded.
pub fn operation_to_permission(operation: crate::hoops::enterprise::Operation) -> Permission {
    use crate::hoops::enterprise::Operation;
    match operation {
        Operation::Store => Permission::Write,
        Operation::Update => Permission::Write,
        Operation::Delete => Permission::Delete,
        Operation::Search => Permission::Read,
    }
}

/// RBAC service — a thin, stateless facade over the `tenant_members` table.
///
/// Holds no role state of its own. The previous version cached roles in a
/// `HashMap` that was never persisted or seeded, which meant `assign_role` was
/// lost on restart and a permission *check* could mutate the map as a side effect
/// (the auto-grant). Both are gone: every answer comes from the database.
pub struct RbacService {
    _private: (),
}

impl RbacService {
    /// Create a new RBAC service.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// The caller's role in `tenant_id`, or `None` if they hold none.
    ///
    /// `None` is the deny answer. There is deliberately **no fallback role**: the
    /// old code auto-granted `Owner` whenever `tenant_id == user_id`, which — given
    /// that `tenant_id` *was* `user_id` — made every check pass.
    ///
    /// ## The non-PostgreSQL exception
    ///
    /// On the SQLite fallback there is no `tenant_members` table and `db::pool()`
    /// panics, so this keeps the old self-tenant `Owner` grant there. That is not a
    /// loophole being preserved for convenience: SQLite mode has **no DB-layer
    /// tenant isolation at all** (backlog C-4), refuses to start without an
    /// explicit `db.allow_sqlite_fallback` opt-in, and prints a startup banner. A
    /// backend that cannot isolate tenants cannot meaningfully distinguish roles
    /// within one either; denying everything instead would only make the documented
    /// dev escape hatch unusable without adding any real protection.
    pub async fn get_role(&self, tenant_id: &str, user_id: &str) -> Option<Role> {
        if !crate::db::is_postgres() {
            return (tenant_id == user_id).then_some(Role::Owner);
        }

        let tenant = crate::tenant::TenantId::from_string(tenant_id);
        match crate::db::tenant_members::role_for(&tenant, user_id).await {
            Ok(role) => role,
            Err(e) => {
                // A failed lookup is not an authorization grant. Log loudly and deny.
                tracing::error!(
                    tenant = %tenant_id,
                    user = %user_id,
                    error = %e,
                    "tenant_members lookup failed; denying"
                );
                None
            }
        }
    }

    /// Whether the caller holds `permission` in `tenant_id`.
    ///
    /// This is the single entry point. The old service exposed two — an async
    /// `has_permission` that did not auto-grant and a sync `blocking_has_permission`
    /// that did — so `routers/mcp.rs` and `hoops/governance.rs` could reach opposite
    /// conclusions about the same caller.
    pub async fn has_permission(
        &self,
        tenant_id: &str,
        user_id: &str,
        permission: Permission,
    ) -> bool {
        match self.get_role(tenant_id, user_id).await {
            Some(role) => role.has_permission(&permission),
            None => false,
        }
    }

    /// Assign (or re-assign) a role. Persisted to `tenant_members`.
    ///
    /// The write runs under the tenant GUC, so PostgreSQL's `WITH CHECK` rejects a
    /// row for any other org regardless of what the caller passed — the handler's
    /// `authorize_path_tenant` and this policy have to *both* be wrong for a
    /// cross-org write to land.
    pub async fn assign_role(
        &self,
        tenant_id: &str,
        user_id: &str,
        role: Role,
        assigned_by: &str,
    ) -> Result<UserRole, crate::AppError> {
        let tenant = crate::tenant::TenantId::from_string(tenant_id);
        crate::db::tenant_members::upsert_member(&tenant, user_id, role, assigned_by).await?;

        info!(
            "Assigned role {} to user {} in tenant {}",
            role, user_id, tenant_id
        );

        Ok(UserRole {
            user_id: user_id.to_string(),
            tenant_id: tenant_id.to_string(),
            role,
            assigned_at: chrono::Utc::now().timestamp(),
            assigned_by: assigned_by.to_string(),
        })
    }

    /// All role assignments in `tenant_id`.
    ///
    /// Rows whose stored `role` this build cannot parse are **skipped with an
    /// error log** rather than failing the whole listing or being coerced to some
    /// default — one unrecognised value must not hide the other members, and must
    /// not invent a role for the row it came from.
    pub async fn list_roles(&self, tenant_id: &str) -> Result<Vec<UserRole>, crate::AppError> {
        let tenant = crate::tenant::TenantId::from_string(tenant_id);
        let rows = crate::db::tenant_members::list_members(&tenant).await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| match Role::from_db_str(&row.role) {
                Some(role) => Some(UserRole {
                    user_id: row.user_id,
                    tenant_id: row.tenant_id,
                    role,
                    assigned_at: 0,
                    assigned_by: row.assigned_by.unwrap_or_default(),
                }),
                None => {
                    tracing::error!(
                        tenant = %tenant_id,
                        user = %row.user_id,
                        stored_role = %row.role,
                        "skipping tenant_members row with an unrecognised role"
                    );
                    None
                }
            })
            .collect())
    }
}

impl Default for RbacService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This file's source with the test module stripped off.
    ///
    /// Required because these guards search for strings that also appear in the
    /// guards themselves. Searching the whole file makes a test match its own
    /// assertion text and fail (or, worse, pass) for reasons that have nothing to
    /// do with the production code.
    ///
    /// This is the third variant of the same trap in this codebase — D-i's guard
    /// was satisfied by doc comments *explaining* the attribute it checked, A-3's
    /// counted symbol mentions in prose, and this one matched its own needle. The
    /// common cause is measuring code and text in one pass; the fix is always to
    /// separate them first.
    fn production_source() -> String {
        let src = include_str!("rbac.rs");
        src.split("\n#[cfg(test)]\n")
            .next()
            .unwrap_or(src)
            .to_string()
    }

    // The role→permission table and the operation→permission mapping are pure
    // logic and stay testable here. Everything that used to exercise
    // `RbacService` directly needed the in-memory role map, which no longer
    // exists — those behaviours are now database behaviour and are covered by the
    // PG-gated suite in `tests/tenant_members_pg.rs`. Asserting them against a
    // fake in-process store would only prove the fake works.

    #[test]
    fn role_permission_table_is_unchanged_by_the_storage_move() {
        assert!(Role::Owner.has_permission(&Permission::DeleteTenant));
        assert!(Role::Owner.has_permission(&Permission::ManageBilling));

        assert!(Role::Admin.has_permission(&Permission::Write));
        assert!(!Role::Admin.has_permission(&Permission::DeleteTenant));
        assert!(!Role::Admin.has_permission(&Permission::ManageBilling));

        assert!(Role::Member.has_permission(&Permission::Write));
        assert!(!Role::Member.has_permission(&Permission::ManageTenant));

        assert!(Role::Reader.has_permission(&Permission::Read));
        assert!(!Role::Reader.has_permission(&Permission::Write));
    }

    #[test]
    fn operation_permission_mapping_is_least_privilege() {
        use crate::hoops::enterprise::Operation;
        assert_eq!(operation_to_permission(Operation::Store), Permission::Write);
        assert_eq!(
            operation_to_permission(Operation::Update),
            Permission::Write
        );
        assert_eq!(
            operation_to_permission(Operation::Delete),
            Permission::Delete
        );
        assert_eq!(operation_to_permission(Operation::Search), Permission::Read);
    }

    /// The auto-grant must stay gone.
    ///
    /// It is the single most re-introducible thing in this file: bringing it back
    /// looks like a one-line convenience ("if the caller owns the tenant, let them
    /// in") and restores today's behaviour for every self-tenant caller, which is
    /// most of them in a dev database — so it would not obviously break anything.
    /// What it actually does is make the role check constant-true again and undo
    /// A-1 / C-1 / P0-6 / P0-7's observable effect, exactly as before C-3.
    ///
    /// Structural rather than behavioural because the behaviour now lives in
    /// PostgreSQL: there is no in-process state left to assert against.
    #[test]
    fn no_self_tenant_auto_grant_outside_the_sqlite_fallback() {
        let src = production_source();
        let body = src
            .split("pub async fn get_role(")
            .nth(1)
            .expect("get_role must exist");
        let body = &body[..body.find("\n    }").unwrap_or(body.len())];

        // The one permitted `tenant_id == user_id` grant is the SQLite branch, and
        // it must be unreachable on PostgreSQL.
        assert!(
            body.contains("if !crate::db::is_postgres()"),
            "the self-tenant Owner grant must be gated behind the non-PostgreSQL \
             branch; without that guard it applies to every caller again"
        );
        let pg_guard = body
            .find("if !crate::db::is_postgres()")
            .expect("guard present");
        let grant = body
            .find("tenant_id == user_id")
            .expect("the SQLite fallback grant should still exist");
        assert!(
            grant > pg_guard,
            "the `tenant_id == user_id` grant must sit INSIDE the non-PostgreSQL \
             branch, not before it"
        );

        assert!(
            !src.contains("fn blocking_has_permission"),
            "the sync entry point that auto-granted Owner and disagreed with the \
             async get_role must not come back"
        );
    }

    /// A failed lookup is not a grant.
    #[test]
    fn lookup_failure_denies() {
        let src = production_source();
        let body = src
            .split("pub async fn get_role(")
            .nth(1)
            .expect("get_role must exist");
        let body = &body[..body.find("\n    }").unwrap_or(body.len())];
        let err_arm = body
            .find("Err(e) =>")
            .expect("the lookup must handle its error case explicitly");
        let tail = &body[err_arm..];
        assert!(
            tail.contains("None"),
            "a tenant_members lookup failure must resolve to None (deny), never to \
             a role"
        );
    }
}
