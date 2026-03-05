//! Tenant Context
//!
//! This module provides tenant context management.

use serde::{Deserialize, Serialize};
use crate::tenant::TenantId;
use crate::tenant::quota::ResourceQuota;

/// Tenant context containing tenant-specific information.
#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub quota: ResourceQuota,
    pub settings: TenantSettings,
    pub created_at: i64,
    pub updated_at: i64,
}

impl TenantContext {
    pub fn new(tenant_id: TenantId) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            tenant_id,
            quota: ResourceQuota::default(),
            settings: TenantSettings::default(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_quota(mut self, quota: ResourceQuota) -> Self {
        self.quota = quota;
        self
    }

    pub fn with_settings(mut self, settings: TenantSettings) -> Self {
        self.settings = settings;
        self
    }

    /// Check if a resource is within quota.
    pub fn check_quota(&self, resource: &QuotaResource) -> bool {
        self.quota.check(resource)
    }
}

/// Tenant-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantSettings {
    pub name: String,
    pub timezone: String,
    pub default_language: String,
    pub features: Vec<String>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for TenantSettings {
    fn default() -> Self {
        Self {
            name: String::new(),
            timezone: "UTC".to_string(),
            default_language: "en-US".to_string(),
            features: vec![],
            metadata: Default::default(),
        }
    }
}

/// Resource quota type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaResource {
    StorageMB,
    ApiCallsPerDay,
    ConcurrentSessions,
    MemoryEntries,
}

impl std::fmt::Display for QuotaResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuotaResource::StorageMB => write!(f, "storage_mb"),
            QuotaResource::ApiCallsPerDay => write!(f, "api_calls_per_day"),
            QuotaResource::ConcurrentSessions => write!(f, "concurrent_sessions"),
            QuotaResource::MemoryEntries => write!(f, "memory_entries"),
        }
    }
}
