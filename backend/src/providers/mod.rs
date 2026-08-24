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
pub use mem0::Mem0Provider;
pub use zep::ZepProvider;
// W2.5: LettaProvider removed from public re-exports (reserved stub, not yet implemented)

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

/// Construct the active provider from config.
///
/// `ProviderType::Letta` is a reserved, unimplemented stub and is **rejected
/// here** rather than silently built — selecting it surfaces a clear error
/// instead of a provider whose every operation returns "not implemented".
/// This is #87's "显式禁用" path (option 2): the stub stays as a reserved
/// marker, but it cannot be constructed for production use.
pub fn create_provider(config: &ProviderConfig) -> MemoryResult<Box<dyn MemoryProvider>> {
    match config.active {
        ProviderType::Builtin => Ok(Box::new(BuiltinProvider::new())),
        ProviderType::Mem0 => Ok(Box::new(Mem0Provider::new(
            config.mem0.clone().unwrap_or_default(),
        ))),
        ProviderType::Zep => Ok(Box::new(ZepProvider::new(
            config.zep.clone().unwrap_or_default(),
        ))),
        ProviderType::Letta => Err(MemoryError::InvalidOperation(
            "Letta provider is reserved but not implemented; select builtin, mem0, or zep"
                .to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_provider_is_constructible() {
        let cfg = ProviderConfig::default(); // active = Builtin
        assert!(create_provider(&cfg).is_ok());
    }

    #[test]
    fn mem0_and_zep_providers_are_constructible() {
        let cfg = ProviderConfig {
            active: ProviderType::Mem0,
            ..Default::default()
        };
        assert!(create_provider(&cfg).is_ok());
        let cfg = ProviderConfig {
            active: ProviderType::Zep,
            ..Default::default()
        };
        assert!(create_provider(&cfg).is_ok());
    }

    #[test]
    fn letta_provider_is_rejected_with_a_clear_error() {
        let cfg = ProviderConfig {
            active: ProviderType::Letta,
            ..Default::default()
        };
        // `unwrap_err()` would require `Box<dyn MemoryProvider>: Debug`; match
        // instead to avoid that bound.
        let err = match create_provider(&cfg) {
            Err(e) => e,
            Ok(_) => panic!("Letta provider should be rejected, but create_provider returned Ok"),
        };
        assert!(
            matches!(err, MemoryError::InvalidOperation(_)),
            "expected InvalidOperation, got {err:?}"
        );
        assert!(err.to_string().contains("Letta"));
    }
}
