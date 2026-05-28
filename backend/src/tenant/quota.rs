//! Resource Quota Management
//!
//! This module provides resource quota management for multi-tenancy.

use crate::tenant::context::QuotaResource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Resource quota for a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    pub storage_mb: u64,
    pub api_calls_per_day: u64,
    pub concurrent_sessions: u32,
    pub memory_entries: u64,
    /// Custom quota limits
    pub custom: HashMap<String, u64>,
    /// Used resources (for tracking)
    pub used: QuotaUsage,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            storage_mb: 1000, // 1GB default
            api_calls_per_day: 100000,
            concurrent_sessions: 10,
            memory_entries: 100000,
            custom: Default::default(),
            used: QuotaUsage::default(),
        }
    }
}

impl ResourceQuota {
    /// Create a new quota with specified limits.
    pub fn new(
        storage_mb: u64,
        api_calls_per_day: u64,
        concurrent_sessions: u32,
        memory_entries: u64,
    ) -> Self {
        Self {
            storage_mb,
            api_calls_per_day,
            concurrent_sessions,
            memory_entries,
            custom: Default::default(),
            used: QuotaUsage::default(),
        }
    }

    /// Check if a resource is within quota.
    pub fn check(&self, resource: &QuotaResource) -> bool {
        match resource {
            QuotaResource::StorageMB => self.used.storage_mb < self.storage_mb,
            QuotaResource::ApiCallsPerDay => self.used.api_calls_today < self.api_calls_per_day,
            QuotaResource::ConcurrentSessions => {
                self.used.concurrent_sessions < self.concurrent_sessions
            }
            QuotaResource::MemoryEntries => self.used.memory_entries < self.memory_entries,
        }
    }

    /// Get remaining quota for a resource.
    pub fn remaining(&self, resource: &QuotaResource) -> u64 {
        match resource {
            QuotaResource::StorageMB => self.storage_mb.saturating_sub(self.used.storage_mb),
            QuotaResource::ApiCallsPerDay => self
                .api_calls_per_day
                .saturating_sub(self.used.api_calls_today),
            QuotaResource::ConcurrentSessions => (self.concurrent_sessions as u64)
                .saturating_sub(self.used.concurrent_sessions as u64),
            QuotaResource::MemoryEntries => {
                self.memory_entries.saturating_sub(self.used.memory_entries)
            }
        }
    }

    /// Add custom quota.
    pub fn add_custom(&mut self, name: impl Into<String>, limit: u64) {
        self.custom.insert(name.into(), limit);
    }
}

/// Quota usage tracking.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuotaUsage {
    pub storage_mb: u64,
    pub api_calls_today: u64,
    pub concurrent_sessions: u32,
    pub memory_entries: u64,
    /// Custom usage
    pub custom: HashMap<String, u64>,
    /// Last reset timestamp
    pub last_reset: i64,
}

impl QuotaUsage {
    pub fn new() -> Self {
        Self {
            storage_mb: 0,
            api_calls_today: 0,
            concurrent_sessions: 0,
            memory_entries: 0,
            custom: Default::default(),
            last_reset: chrono::Utc::now().timestamp(),
        }
    }

    /// Increment API call count.
    pub fn add_api_call(&mut self) {
        self.api_calls_today += 1;
    }

    /// Reset daily counters if needed.
    pub fn reset_if_needed(&mut self) {
        let now = chrono::Utc::now().timestamp();
        let day_seconds = 86400;

        if now - self.last_reset > day_seconds {
            self.api_calls_today = 0;
            self.last_reset = now;
        }
    }
}

/// Quota manager for tracking and enforcing quotas.
pub struct QuotaManager {
    quotas: std::sync::RwLock<HashMap<String, ResourceQuota>>,
}

impl QuotaManager {
    pub fn new() -> Self {
        Self {
            quotas: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Get quota for a tenant.
    pub fn get_quota(&self, tenant_id: &str) -> Option<ResourceQuota> {
        let quotas = self.quotas.read().ok()?;
        quotas.get(tenant_id).cloned()
    }

    /// Set quota for a tenant.
    pub fn set_quota(&self, tenant_id: &str, quota: ResourceQuota) {
        if let Ok(mut quotas) = self.quotas.write() {
            quotas.insert(tenant_id.to_string(), quota);
        }
    }

    /// Check if tenant can perform an action.
    pub fn can_perform(&self, tenant_id: &str, resource: &QuotaResource) -> bool {
        let quotas = match self.quotas.read() {
            Ok(q) => q,
            Err(_) => return false,
        };
        if let Some(quota) = quotas.get(tenant_id) {
            quota.check(resource)
        } else {
            true // No quota means unlimited
        }
    }
}

impl Default for QuotaManager {
    fn default() -> Self {
        Self::new()
    }
}
