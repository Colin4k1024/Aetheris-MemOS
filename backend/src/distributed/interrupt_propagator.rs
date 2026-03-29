//! Interrupt Propagator
//!
//! Manages registration and propagation of cancellation signals across epoch-scoped
//! execution contexts. When heartbeat failure or other critical events occur,
//! cancellation is propagated to all registered contexts for the affected epoch.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;

use super::epoch_manager::{CancellationFunc, RegisteredContext};

/// Manages active cancellation contexts and propagates cancellation signals by epoch.
#[derive(Debug)]
pub struct InterruptPropagator {
    /// Map of epoch -> list of registered cancellation contexts.
    active_contexts: Arc<Mutex<HashMap<u64, Vec<RegisteredContext>>>>,
}

impl InterruptPropagator {
    /// Creates a new InterruptPropagator with no active contexts.
    pub fn new() -> Self {
        Self {
            active_contexts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Registers a cancellation context for the given epoch.
    /// When propagate_cancellation is called for this epoch, the cancel_fn will be invoked.
    pub fn register_context(&self, epoch: u64, cancel_fn: CancellationFunc) {
        let mut contexts = self.active_contexts.lock().unwrap();
        let entry = contexts.entry(epoch).or_insert_with(Vec::new);
        entry.push(RegisteredContext::new(epoch, cancel_fn));
    }

    /// Unregisters all cancellation contexts for the given epoch.
    /// This is typically called when an epoch completes successfully.
    pub fn unregister_context(&self, epoch: u64) {
        let mut contexts = self.active_contexts.lock().unwrap();
        contexts.remove(&epoch);
    }

    /// Propagates cancellation to all contexts registered for the given epoch.
    /// Calls the cancel function for each registered context, then removes them.
    pub fn propagate_cancellation(&self, epoch: u64) {
        let contexts: Vec<RegisteredContext> = {
            let mut contexts = self.active_contexts.lock().unwrap();
            contexts.remove(&epoch).unwrap_or_default()
        };

        for ctx in contexts {
            ctx.cancel();
        }
    }

    /// Returns the number of active contexts for the given epoch.
    #[allow(dead_code)]
    pub fn active_context_count(&self, epoch: u64) -> usize {
        let contexts = self.active_contexts.lock().unwrap();
        contexts.get(&epoch).map(|v| v.len()).unwrap_or(0)
    }

    /// Returns the total number of epochs with registered contexts.
    #[allow(dead_code)]
    pub fn active_epoch_count(&self) -> usize {
        let contexts = self.active_contexts.lock().unwrap();
        contexts.len()
    }
}

impl Default for InterruptPropagator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_unregister_context() {
        let propagator = InterruptPropagator::new();

        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();

        propagator.register_context(
            1,
            Box::new(move || {
                called_clone.store(true, Ordering::SeqCst);
            }),
        );

        assert_eq!(propagator.active_context_count(1), 1);
        propagator.unregister_context(1);
        assert_eq!(propagator.active_context_count(1), 0);
    }

    #[test]
    fn test_propagate_cancellation() {
        use std::sync::atomic::Ordering;

        let propagator = InterruptPropagator::new();

        let called1 = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called2 = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let c1 = called1.clone();
        let c2 = called2.clone();

        propagator.register_context(
            1,
            Box::new(move || {
                c1.store(true, Ordering::SeqCst);
            }),
        );
        propagator.register_context(
            1,
            Box::new(move || {
                c2.store(true, Ordering::SeqCst);
            }),
        );

        assert!(!called1.load(Ordering::SeqCst));
        assert!(!called2.load(Ordering::SeqCst));

        propagator.propagate_cancellation(1);

        assert!(called1.load(Ordering::SeqCst));
        assert!(called2.load(Ordering::SeqCst));
        assert_eq!(propagator.active_context_count(1), 0);
    }

    #[test]
    fn test_propagate_nonexistent_epoch() {
        let propagator = InterruptPropagator::new();
        // Should not panic
        propagator.propagate_cancellation(999);
    }
}
