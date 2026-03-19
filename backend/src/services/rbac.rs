//! RBAC (Role-Based Access Control) Service
//!
//! This module provides role-based access control for multi-tenancy.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

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
        let mut roles = self.roles.write().await;

        let tenant_roles = roles.entry(tenant_id.to_string()).or_default();
        tenant_roles.insert(
            user_id.to_string(),
            role,
        );

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
        let mut roles = self.roles.write().await;

        if let Some(tenant_roles) = roles.get_mut(tenant_id) {
            tenant_roles.remove(user_id);
        }

        info!("Removed role from user {} in tenant {}", user_id, tenant_id);
        Ok(())
    }

    /// Get user's role in a tenant
    pub async fn get_role(&self, tenant_id: &str, user_id: &str) -> Option<Role> {
        let roles = self.roles.read().await;
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
    pub async fn can_perform(
        &self,
        tenant_id: &str,
        user_id: &str,
        required_role: Role,
    ) -> bool {
        if let Some(role) = self.get_role(tenant_id, user_id).await {
            role.level() >= required_role.level()
        } else {
            false
        }
    }

    /// List all users and their roles in a tenant
    pub async fn list_roles(&self, tenant_id: &str) -> Vec<UserRole> {
        let roles = self.roles.read().await;

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

        assert!(rbac
            .has_permission("tenant1", "admin_user", Permission::Write)
            .await);
        assert!(!rbac
            .has_permission("tenant1", "reader_user", Permission::Write)
            .await);
    }

    #[tokio::test]
    async fn test_owner_has_all_permissions() {
        let rbac = RbacService::new();
        rbac.assign_role("tenant1", "owner", Role::Owner, "system")
            .await
            .unwrap();

        assert!(rbac
            .has_permission("tenant1", "owner", Permission::DeleteTenant)
            .await);
        assert!(rbac
            .has_permission("tenant1", "owner", Permission::ManageBilling)
            .await);
    }
}
