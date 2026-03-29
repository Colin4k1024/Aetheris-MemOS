//! Integration tests for epoch generation and interrupt propagation
//!
//! These tests verify that:
//! - Epochs increment atomically
//! - Cancellation is properly propagated to registered contexts

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use backend::distributed::{EpochManager, InterruptPropagator};

#[test]
fn test_epoch_increment() {
    let manager = EpochManager::new();

    // Verify initial epoch is 0
    assert_eq!(manager.current_epoch(), 0);

    // Generate epochs and verify they increment
    let epoch1 = manager.generate_epoch();
    let epoch2 = manager.generate_epoch();
    let epoch3 = manager.generate_epoch();

    assert_eq!(epoch1, 0);
    assert_eq!(epoch2, 1);
    assert_eq!(epoch3, 2);

    // Verify current_epoch reflects the latest
    assert_eq!(manager.current_epoch(), 3);
}

#[test]
fn test_interrupt_propagation() {
    let propagator = InterruptPropagator::new();

    let called1 = Arc::new(AtomicBool::new(false));
    let called2 = Arc::new(AtomicBool::new(false));

    let c1 = called1.clone();
    let c2 = called2.clone();

    // Register two cancellation callbacks for epoch 1
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

    // Neither should be called yet
    assert!(!called1.load(Ordering::SeqCst));
    assert!(!called2.load(Ordering::SeqCst));

    // Propagate cancellation for epoch 1
    propagator.propagate_cancellation(1);

    // Both should now be called
    assert!(called1.load(Ordering::SeqCst));
    assert!(called2.load(Ordering::SeqCst));

    // Contexts for epoch 1 should be cleared
    assert_eq!(propagator.active_context_count(1), 0);
}

#[test]
fn test_interrupt_propagation_only_for_specific_epoch() {
    let propagator = InterruptPropagator::new();

    let called = Arc::new(AtomicBool::new(false));
    let c = called.clone();

    // Register callback for epoch 5 only
    propagator.register_context(
        5,
        Box::new(move || {
            c.store(true, Ordering::SeqCst);
        }),
    );

    // Propagate for a different epoch
    propagator.propagate_cancellation(3);

    // Should not have been called
    assert!(!called.load(Ordering::SeqCst));

    // Now propagate for epoch 5
    propagator.propagate_cancellation(5);

    // Should have been called
    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn test_unregister_context() {
    let propagator = InterruptPropagator::new();

    let called = Arc::new(AtomicBool::new(false));
    let c = called.clone();

    propagator.register_context(
        1,
        Box::new(move || {
            c.store(true, Ordering::SeqCst);
        }),
    );

    // Verify it's registered
    assert_eq!(propagator.active_context_count(1), 1);

    // Unregister
    propagator.unregister_context(1);

    // Verify it's gone
    assert_eq!(propagator.active_context_count(1), 0);

    // Propagate should have nothing to call
    propagator.propagate_cancellation(1);
    assert!(!called.load(Ordering::SeqCst));
}

#[test]
fn test_propagate_nonexistent_epoch_does_not_panic() {
    let propagator = InterruptPropagator::new();

    // Should not panic
    propagator.propagate_cancellation(999);

    // Epoch 999 should still show 0 contexts after
    assert_eq!(propagator.active_context_count(999), 0);
}

#[test]
fn test_active_epoch_count() {
    let propagator = InterruptPropagator::new();

    assert_eq!(propagator.active_epoch_count(), 0);

    // Register for epoch 1
    propagator.register_context(1, Box::new(|| {}));
    assert_eq!(propagator.active_epoch_count(), 1);

    // Register for epoch 2
    propagator.register_context(2, Box::new(|| {}));
    assert_eq!(propagator.active_epoch_count(), 2);

    // Register more for epoch 1
    propagator.register_context(1, Box::new(|| {}));
    assert_eq!(propagator.active_epoch_count(), 2); // Still 2 epochs

    // Unregister epoch 1
    propagator.unregister_context(1);
    assert_eq!(propagator.active_epoch_count(), 1);
}

#[test]
fn test_epoch_manager_thread_safe() {
    let manager = Arc::new(EpochManager::new());

    // Spawn multiple threads to generate epochs
    let mut handles = vec![];

    for _ in 0..4 {
        let m = manager.clone();
        handles.push(std::thread::spawn(move || {
            for _ in 0..25 {
                m.generate_epoch();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // All 100 epochs should be generated
    assert_eq!(manager.current_epoch(), 100);
}

#[test]
fn test_epoch_manager_concurrent_registration() {
    let propagator = Arc::new(InterruptPropagator::new());

    let mut handles = vec![];

    for i in 0u64..50 {
        let p = propagator.clone();
        handles.push(std::thread::spawn(move || {
            p.register_context(i, Box::new(|| {}));
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(propagator.active_epoch_count(), 50);
}
