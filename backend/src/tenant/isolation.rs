//! Tenant Isolation
//!
//! This module provides tenant isolation mechanisms for multi-tenant memory systems.

use crate::kernel::types::*;
use crate::tenant::TenantId;

/// Tenant isolation configuration.
#[derive(Debug, Clone)]
pub struct TenantIsolationConfig {
    /// Enable row-level security
    pub enable_rls: bool,
    /// Use separate database per tenant
    pub separate_databases: bool,
    /// Cache tenant data in memory
    pub enable_caching: bool,
}

impl Default for TenantIsolationConfig {
    fn default() -> Self {
        Self {
            enable_rls: false,
            separate_databases: false,
            enable_caching: true,
        }
    }
}

/// Tenant isolation layer.
///
/// Ensures tenants cannot access each other's data through:
/// - Query filtering
/// - Access verification
/// - Data scoping
pub struct TenantIsolation {
    config: TenantIsolationConfig,
}

impl TenantIsolation {
    /// Create a new tenant isolation layer.
    pub fn new() -> Self {
        Self {
            config: TenantIsolationConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: TenantIsolationConfig) -> Self {
        Self { config }
    }

    /// Add tenant filter to a query.
    ///
    /// This ensures all queries are scoped to the tenant.
    pub fn add_filter(&self, filters: &mut MemoryFilters, tenant_id: &TenantId) {
        // Add tenant_id to the filters
        // This would be used in SQL queries with RLS or as application-level filtering

        // Store tenant_id in tags for additional filtering
        let tenant_tag = format!("tenant:{}", tenant_id.as_str());
        if let Some(tags) = &mut filters.tags {
            if !tags.contains(&tenant_tag) {
                tags.push(tenant_tag);
            }
        } else {
            filters.tags = Some(vec![tenant_tag]);
        }
    }

    /// Verify tenant has access to a memory entry.
    pub fn verify_access(&self, memory: &MemoryEntry, tenant_id: &TenantId) -> bool {
        let tenant_tag = format!("tenant:{}", tenant_id.as_str());
        let has_tenant_tag = memory
            .metadata
            .tags
            .iter()
            .any(|tag| tag.starts_with("tenant:"));

        if has_tenant_tag {
            return memory.metadata.tags.contains(&tenant_tag);
        }

        // Fail closed when the entry does not carry tenant scoping metadata.
        false
    }

    /// Get tenant-scoped filters.
    ///
    /// Returns a MemoryFilters that restricts access to the tenant's data.
    pub fn scoped_filters(&self, tenant_id: &TenantId) -> MemoryFilters {
        let tenant_tag = format!("tenant:{}", tenant_id.as_str());

        MemoryFilters {
            user_id: None,
            session_id: None,
            agent_id: None,
            tags: Some(vec![tenant_tag]),
            min_importance: None,
            created_after: None,
            created_before: None,
        }
    }

    /// Filter a list of memory entries to only those accessible by the tenant.
    pub fn filter_entries<'a>(
        &self,
        entries: &'a [MemoryEntry],
        tenant_id: &TenantId,
    ) -> Vec<&'a MemoryEntry> {
        entries
            .iter()
            .filter(|entry| self.verify_access(entry, tenant_id))
            .collect()
    }

    /// Create a tenant-scoped query.
    pub fn create_tenant_query(
        &self,
        base_filters: Option<MemoryFilters>,
        tenant_id: &TenantId,
    ) -> MemoryFilters {
        let mut filters = base_filters.unwrap_or_default();
        self.add_filter(&mut filters, tenant_id);
        filters
    }

    /// Check if a tenant can access another tenant's data (cross-tenant access).
    pub fn can_access_cross_tenant(
        &self,
        _tenant_id: &TenantId,
        _target_tenant_id: &TenantId,
    ) -> bool {
        // By default, cross-tenant access is denied
        // In production, this could check for admin privileges or explicit grants
        false
    }

    /// Get the effective tenant ID for queries.
    pub fn effective_tenant_id(&self, tenant_id: &TenantId) -> String {
        tenant_id.as_str().to_string()
    }
}

impl Default for TenantIsolation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tenant() -> TenantId {
        TenantId::from_string("tenant_123")
    }

    fn create_test_memory_with_tenant(tenant_id: &str) -> MemoryEntry {
        let tenant_tag = format!("tenant:{}", tenant_id);
        let now = chrono::Utc::now().timestamp();
        MemoryEntry {
            id: MemoryId::new(),
            content: MemoryContent::Text("Test content".to_string()),
            metadata: MemoryMetadata {
                user_id: Some("user_456".to_string()),
                session_id: None,
                agent_id: None,
                tags: vec![tenant_tag],
                importance: 0.5,
                access_count: 0,
                last_accessed: None,
                expires_at: None,
                source: None,
                extra: std::collections::HashMap::new(),
            },
            layer: LayerType::Stm,
            created_at: now,
            updated_at: now,
        }
    }

    fn create_test_memory_no_tenant() -> MemoryEntry {
        let now = chrono::Utc::now().timestamp();
        MemoryEntry {
            id: MemoryId::new(),
            content: MemoryContent::Text("Test content".to_string()),
            metadata: MemoryMetadata {
                user_id: Some("user_456".to_string()),
                session_id: None,
                agent_id: None,
                tags: vec![],
                importance: 0.5,
                access_count: 0,
                last_accessed: None,
                expires_at: None,
                source: None,
                extra: std::collections::HashMap::new(),
            },
            layer: LayerType::Stm,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn test_verify_access_same_tenant() {
        let isolation = TenantIsolation::new();
        let tenant = create_test_tenant();
        let memory = create_test_memory_with_tenant("tenant_123");

        assert!(isolation.verify_access(&memory, &tenant));
    }

    #[test]
    fn test_verify_access_different_tenant() {
        let isolation = TenantIsolation::new();
        let tenant = create_test_tenant();
        let memory = create_test_memory_with_tenant("other_tenant");

        assert!(!isolation.verify_access(&memory, &tenant));
    }

    #[test]
    fn test_verify_access_no_tenant_tag() {
        let isolation = TenantIsolation::new();
        let tenant = create_test_tenant();
        let memory = create_test_memory_no_tenant();

        assert!(!isolation.verify_access(&memory, &tenant));
    }

    #[test]
    fn test_scoped_filters() {
        let isolation = TenantIsolation::new();
        let tenant = create_test_tenant();

        let filters = isolation.scoped_filters(&tenant);

        assert!(filters
            .tags
            .as_ref()
            .unwrap()
            .contains(&"tenant:tenant_123".to_string()));
    }

    #[test]
    fn test_filter_entries() {
        let isolation = TenantIsolation::new();
        let tenant = create_test_tenant();

        let entries = vec![
            create_test_memory_with_tenant("tenant_123"),
            create_test_memory_with_tenant("other_tenant"),
            create_test_memory_with_tenant("tenant_123"),
            create_test_memory_no_tenant(),
        ];

        let filtered = isolation.filter_entries(&entries, &tenant);

        assert_eq!(filtered.len(), 2);
    }
}
