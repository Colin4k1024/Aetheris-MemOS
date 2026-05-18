//! Memory Providers - External Backend Adapters
//!
//! This module implements the MemoryProvider trait for various backends:
//! - Builtin: wraps internal MemoryLayer chain
//! - Mem0: HTTP API integration
//! - Zep: HTTP API integration
//! - Letta: stub (interface only)

pub mod builtin;
pub mod circuit_breaker;
pub mod config;
pub mod letta;
pub mod mem0;
pub mod zep;

pub use builtin::BuiltinProvider;
pub use config::ProviderConfig;
pub use letta::LettaProvider;
pub use mem0::Mem0Provider;
pub use zep::ZepProvider;

use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::provider::{MemoryProvider, ProviderType};

pub fn validate_path_segment(segment: &str) -> MemoryResult<()> {
    if segment.is_empty()
        || segment.contains('/')
        || segment.contains('\\')
        || segment.contains('\0')
        || segment.contains("..")
        || segment.contains('?')
        || segment.contains('#')
    {
        return Err(MemoryError::InvalidOperation(format!(
            "invalid path segment: {:?}",
            segment
        )));
    }
    Ok(())
}

pub fn create_provider(config: &ProviderConfig) -> Box<dyn MemoryProvider> {
    match config.active {
        ProviderType::Builtin => Box::new(BuiltinProvider::new()),
        ProviderType::Mem0 => Box::new(Mem0Provider::new(config.mem0.clone().unwrap_or_default())),
        ProviderType::Zep => Box::new(ZepProvider::new(config.zep.clone().unwrap_or_default())),
        ProviderType::Letta => Box::new(LettaProvider::new()),
    }
}
