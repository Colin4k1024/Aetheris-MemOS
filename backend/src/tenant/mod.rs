//! Multi-Tenant Support
//!
//! This module provides multi-tenancy features:
//! - Tenant context and isolation
//! - Resource quota management
//! - Tenant-specific configurations

pub mod context;
pub mod quota;
pub mod isolation;

pub use context::{TenantContext, RequestTenantContext};
pub use quota::QuotaManager;
pub use isolation::TenantIsolation;

pub use crate::tenant::context::QuotaResource;

/// Tenant identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantId(pub String);

impl TenantId {
    /// Create a new TenantId with a freshly generated ULID.
    pub fn new() -> Self {
        Self(ulid::Ulid::new().to_string())
    }

    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TenantId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
