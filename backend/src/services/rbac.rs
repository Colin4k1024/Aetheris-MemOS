//! RBAC (Role-Based Access Control) Service
//!
//! This module provides role-based access control for multi-tenancy.
//!
//! ## Design constraint (honest assessment)
//!
//! In the current MVP, every authenticated user is the sole member of their own
//! tenant (`tenant_id == user_id`).  When a permission check runs and no role is
//! recorded for (tenant, user), the service **auto-grants the Owner role** if
//! `tenant_id == user_id`.  This makes governance functional without requiring an
//! admin bootstrap flow that has nobody to administer it.
//!
//! **What this means in practice:** every authenticated user is Owner of their
//! own single-user tenant, so RBAC does **not** yet differentiate privileges
//! among users.  All governed store/search operations are permitted as long as
//! the caller is authenticated.
//!
//! **Future work:** meaningful role separation (Owner / Admin / Member / Reader)
//! requires an organization-level tenant model where `tenant_id` is decoupled
//! from `user_id`, so multiple users can belong to the same tenant with different
//! roles.  That is not implemented yet.
//!
//! The role map is **in-memory only** (no persistence, no seeding at startup),
//! so the auto-grant is **lazy and idempotent** — it self-heals after every
//! restart without any explicit bootstrap step.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
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

/// RBAC service
///
/// Uses a synchronous `std::sync::RwLock` so that blocking reads are safe in
/// both async and sync contexts.  Lock contention never causes a spurious
/// "permission denied" — the reader simply blocks until the lock is available.
pub struct RbacService {
    roles: Arc<RwLock<HashMap<String, HashMap<String, Role>>>>, // tenant_id -> user_id -> role
}

impl RbacService {
    /// Get internal roles for testing or advanced use cases
    pub fn roles(&self) -> &Arc<RwLock<HashMap<String, HashMap<String, Role>>>> {
        &self.roles
    }
}

impl RbacService {
    /// Create a new RBAC service
    pub fn new() -> Self {
        Self {
            roles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Assign a role to a user
    pub async fn assign_role(
        &self,
        tenant_id: &str,
        user_id: &str,
        role: Role,
        assigned_by: &str,
    ) -> Result<UserRole, crate::AppError> {
        let mut roles = self.roles.write().unwrap();

        let tenant_roles = roles.entry(tenant_id.to_string()).or_default();
        tenant_roles.insert(user_id.to_string(), role);

        let user_role = UserRole {
            user_id: user_id.to_string(),
            tenant_id: tenant_id.to_string(),
            role,
            assigned_at: chrono::Utc::now().timestamp(),
            assigned_by: assigned_by.to_string(),
        };

        info!(
            "Assigned role {} to user {} in tenant {}",
            role, user_id, tenant_id
        );

        Ok(user_role)
    }

    /// Remove a role from a user
    pub async fn remove_role(&self, tenant_id: &str, user_id: &str) -> Result<(), crate::AppError> {
        let mut roles = self.roles.write().unwrap();

        if let Some(tenant_roles) = roles.get_mut(tenant_id) {
            tenant_roles.remove(user_id);
        }

        info!("Removed role from user {} in tenant {}", user_id, tenant_id);
        Ok(())
    }

    /// Get user's role in a tenant
    pub async fn get_role(&self, tenant_id: &str, user_id: &str) -> Option<Role> {
        let roles = self.roles.read().unwrap();
        roles.get(tenant_id).and_then(|r| r.get(user_id)).copied()
    }

    /// Check if user has permission
    pub async fn has_permission(
        &self,
        tenant_id: &str,
        user_id: &str,
        permission: Permission,
    ) -> bool {
        if let Some(role) = self.get_role(tenant_id, user_id).await {
            role.has_permission(&permission)
        } else {
            false
        }
    }

    /// Check if user can perform action (with role level check)
    pub async fn can_perform(&self, tenant_id: &str, user_id: &str, required_role: Role) -> bool {
        if let Some(role) = self.get_role(tenant_id, user_id).await {
            role.level() >= required_role.level()
        } else {
            false
        }
    }

    /// List all users and their roles in a tenant
    pub async fn list_roles(&self, tenant_id: &str) -> Vec<UserRole> {
        let roles = self.roles.read().unwrap();

        roles
            .get(tenant_id)
            .map(|tenant_roles| {
                tenant_roles
                    .iter()
                    .map(|(user_id, role)| UserRole {
                        user_id: user_id.clone(),
                        tenant_id: tenant_id.to_string(),
                        role: *role,
                        assigned_at: 0, // Not stored per-user
                        assigned_by: String::new(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Synchronous permission check with **lazy auto-grant**.
    ///
    /// When no role is recorded for `(tenant_id, user_id)` AND the two IDs are
    /// equal (i.e. the caller is checking their own single-user tenant), the
    /// Owner role is automatically granted and the check proceeds.  This makes
    /// governance functional without a bootstrap flow.
    ///
    /// The auto-grant is **idempotent** — if the role already exists (from a
    /// previous auto-grant or explicit assignment), it is not overwritten.
    ///
    /// Lock contention is **not** a failure mode: the internal `std::sync::RwLock`
    /// blocks the calling thread until the lock is available, so a contended
    /// lock never produces a spurious `false`.
    pub fn blocking_has_permission(
        &self,
        tenant_id: &str,
        user_id: &str,
        permission: Permission,
    ) -> bool {
        // Fast path: check under a read lock.
        {
            let roles = self.roles.read().unwrap();
            if let Some(tenant_roles) = roles.get(tenant_id) {
                if let Some(role) = tenant_roles.get(user_id) {
                    return role.has_permission(&permission);
                }
            }
        }

        // Slow path: no role recorded.  If the caller is checking their own
        // single-user tenant, auto-grant Owner.
        if tenant_id == user_id {
            let mut roles = self.roles.write().unwrap();
            // Double-check under write lock to avoid TOCTOU race.
            let tenant_roles = roles.entry(tenant_id.to_string()).or_default();
            if let Some(role) = tenant_roles.get(user_id) {
                return role.has_permission(&permission);
            }
            // Auto-grant Owner (idempotent).
            tenant_roles.insert(user_id.to_string(), Role::Owner);
            info!(
                "Auto-granted Owner role to user {} in self-tenant {} (lazy bootstrap)",
                user_id, tenant_id
            );
            return true; // Owner has all permissions.
        }

        false
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

    #[tokio::test]
    async fn test_assign_role() {
        let rbac = RbacService::new();
        let result = rbac
            .assign_role("tenant1", "user1", Role::Admin, "owner1")
            .await
            .unwrap();

        assert_eq!(result.role, Role::Admin);
    }

    #[tokio::test]
    async fn test_has_permission() {
        let rbac = RbacService::new();
        rbac.assign_role("tenant1", "admin_user", Role::Admin, "owner1")
            .await
            .unwrap();
        rbac.assign_role("tenant1", "reader_user", Role::Reader, "owner1")
            .await
            .unwrap();

        assert!(
            rbac.has_permission("tenant1", "admin_user", Permission::Write)
                .await
        );
        assert!(
            !rbac
                .has_permission("tenant1", "reader_user", Permission::Write)
                .await
        );
    }

    #[tokio::test]
    async fn test_owner_has_all_permissions() {
        let rbac = RbacService::new();
        rbac.assign_role("tenant1", "owner", Role::Owner, "system")
            .await
            .unwrap();

        assert!(
            rbac.has_permission("tenant1", "owner", Permission::DeleteTenant)
                .await
        );
        assert!(
            rbac.has_permission("tenant1", "owner", Permission::ManageBilling)
                .await
        );
    }

    #[test]
    fn test_blocking_has_permission_existing_role() {
        let rbac = RbacService::new();
        // Pre-assign admin role
        rbac.roles
            .write()
            .unwrap()
            .entry("tenant1".to_string())
            .or_default()
            .insert("admin_user".to_string(), Role::Admin);

        assert!(rbac.blocking_has_permission("tenant1", "admin_user", Permission::Write));
        assert!(!rbac.blocking_has_permission("tenant1", "admin_user", Permission::DeleteTenant));
    }

    #[test]
    fn test_blocking_has_permission_lazy_auto_grant() {
        let rbac = RbacService::new();
        // No role assigned — but tenant_id == user_id, so auto-grant Owner.
        assert!(rbac.blocking_has_permission("user42", "user42", Permission::Read));
        assert!(rbac.blocking_has_permission("user42", "user42", Permission::DeleteTenant));

        // Verify the role was actually persisted.
        let roles = rbac.roles.read().unwrap();
        let role = roles
            .get("user42")
            .and_then(|r| r.get("user42"))
            .copied();
        assert_eq!(role, Some(Role::Owner));
    }

    #[test]
    fn test_blocking_has_permission_no_auto_grant_for_cross_tenant() {
        let rbac = RbacService::new();
        // tenant_id != user_id — no auto-grant.
        assert!(!rbac.blocking_has_permission("tenant1", "user1", Permission::Read));

        // Verify no role was created.
        let roles = rbac.roles.read().unwrap();
        assert!(roles.get("tenant1").is_none());
    }

    #[test]
    fn test_blocking_has_permission_idempotent() {
        let rbac = RbacService::new();
        // First call auto-grants Owner.
        assert!(rbac.blocking_has_permission("u1", "u1", Permission::Read));
        // Second call should find the existing Owner role (not re-grant).
        assert!(rbac.blocking_has_permission("u1", "u1", Permission::Read));

        // Verify only one entry exists.
        let roles = rbac.roles.read().unwrap();
        assert_eq!(roles.get("u1").unwrap().len(), 1);
    }
}