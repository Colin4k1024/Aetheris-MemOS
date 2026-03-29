//! Epoch Manager
//!
//! Provides epoch generation and tracking for worker execution contracts.
//! Each epoch represents a distinct execution cycle that can be individually
//! cancelled via interrupt propagation.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Cancellation function type for epoch-scoped contexts.
pub type CancellationFunc = Box<dyn Fn() + Send + Sync>;

/// EpochManager generates monotonically increasing epochs for worker execution contracts.
/// Epochs are used to track and cancel batches of related operations atomically.
#[derive(Debug)]
pub struct EpochManager {
    current_epoch: Arc<AtomicU64>,
}

impl EpochManager {
    /// Creates a new EpochManager with epoch starting at 0.
    pub fn new() -> Self {
        Self {
            current_epoch: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Generates a new epoch by atomically incrementing the counter.
    /// Returns the newly generated epoch value.
    pub fn generate_epoch(&self) -> u64 {
        self.current_epoch.fetch_add(1, Ordering::SeqCst)
    }

    /// Returns the current epoch value without incrementing.
    pub fn current_epoch(&self) -> u64 {
        self.current_epoch.load(Ordering::SeqCst)
    }
}

impl Default for EpochManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Context that embeds the epoch into execution context.
/// Used to track which epoch a particular operation belongs to.
#[derive(Debug, Clone, Copy)]
pub struct EpochContext {
    /// The epoch number this context belongs to.
    pub epoch: u64,
}

impl EpochContext {
    /// Creates a new EpochContext for the given epoch.
    pub fn new(epoch: u64) -> Self {
        Self { epoch }
    }
}

/// A registered cancel context tied to a specific epoch.
/// When cancellation is propagated for an epoch, all registered contexts
/// for that epoch have their cancellation functions invoked.
pub struct RegisteredContext {
    /// The epoch this context is registered to.
    pub epoch: u64,
    /// The cancellation function to invoke on propagate_cancellation.
    pub cancel_fn: CancellationFunc,
}

impl std::fmt::Debug for RegisteredContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredContext")
            .field("epoch", &self.epoch)
            .finish()
    }
}

impl RegisteredContext {
    /// Creates a new RegisteredContext for the given epoch with the provided cancellation function.
    pub fn new(epoch: u64, cancel_fn: CancellationFunc) -> Self {
        Self { epoch, cancel_fn }
    }

    /// Invokes the cancellation function for this context.
    pub fn cancel(&self) {
        (self.cancel_fn)();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_starts_at_zero() {
        let manager = EpochManager::new();
        assert_eq!(manager.current_epoch(), 0);
    }

    #[test]
    fn test_generate_epoch_increments() {
        let manager = EpochManager::new();
        let epoch1 = manager.generate_epoch();
        let epoch2 = manager.generate_epoch();
        let epoch3 = manager.generate_epoch();

        assert_eq!(epoch1, 0);
        assert_eq!(epoch2, 1);
        assert_eq!(epoch3, 2);
    }

    #[test]
    fn test_current_epoch_returns_current_value() {
        let manager = EpochManager::new();
        assert_eq!(manager.current_epoch(), 0);
        manager.generate_epoch();
        manager.generate_epoch();
        assert_eq!(manager.current_epoch(), 2);
    }

    #[test]
    fn test_epoch_context() {
        let ctx = EpochContext::new(42);
        assert_eq!(ctx.epoch, 42);
    }

    #[test]
    fn test_registered_context_cancel() {
        let canceled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let canceled_clone = canceled.clone();

        let ctx = RegisteredContext::new(
            1,
            Box::new(move || {
                canceled_clone.store(true, Ordering::SeqCst);
            }),
        );

        assert!(!canceled.load(Ordering::SeqCst));
        ctx.cancel();
        assert!(canceled.load(Ordering::SeqCst));
    }
}
