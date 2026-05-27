//! Lease Coordinator
//!
//! Coordinates worker leases with epoch tracking and interrupt propagation.
//! When heartbeat failures occur, the HeartbeatGuardian triggers cancellation
//! propagation for the current epoch to cleanly interrupt all related operations.

use std::sync::Arc;
use tokio::sync::RwLock;

use super::epoch_manager::EpochManager;
use super::interrupt_propagator::InterruptPropagator;

/// Lease state within an epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseState {
    /// Lease is active and valid.
    Active,
    /// Lease has expired and should be renewed.
    Expired,
    /// Lease has been cancelled via interrupt propagation.
    Cancelled,
}

/// A lease tied to a specific epoch.
/// The lease is cancelled if the epoch is interrupted.
#[derive(Debug, Clone)]
pub struct EpochLease {
    /// The epoch this lease belongs to.
    pub epoch: u64,
    /// The current state of the lease.
    pub state: LeaseState,
}

impl EpochLease {
    /// Creates a new epoch lease for the given epoch.
    pub fn new(epoch: u64) -> Self {
        Self {
            epoch,
            state: LeaseState::Active,
        }
    }

    /// Marks the lease as cancelled.
    pub fn cancel(&mut self) {
        self.state = LeaseState::Cancelled;
    }
}

/// Coordinates leases with epoch management and interrupt propagation.
#[derive(Debug)]
pub struct LeaseCoordinator {
    /// The epoch manager for generating and tracking epochs.
    epoch_manager: Arc<EpochManager>,
    /// The interrupt propagator for cancelling epoch-scoped operations.
    interrupt_propagator: Arc<InterruptPropagator>,
    /// The current active lease, if any.
    current_lease: RwLock<Option<EpochLease>>,
}

impl LeaseCoordinator {
    /// Creates a new LeaseCoordinator with a new EpochManager and InterruptPropagator.
    pub fn new() -> Self {
        Self {
            epoch_manager: Arc::new(EpochManager::new()),
            interrupt_propagator: Arc::new(InterruptPropagator::new()),
            current_lease: RwLock::new(None),
        }
    }

    /// Creates a new LeaseCoordinator with shared epoch manager and interrupt propagator.
    pub fn with_shared(
        epoch_manager: Arc<EpochManager>,
        interrupt_propagator: Arc<InterruptPropagator>,
    ) -> Self {
        Self {
            epoch_manager,
            interrupt_propagator,
            current_lease: RwLock::new(None),
        }
    }

    /// Acquires a new lease for a fresh epoch.
    /// Cancels any existing lease first.
    pub async fn acquire_lease(&self) -> EpochLease {
        // Cancel existing lease if any
        {
            let mut current = self.current_lease.write().await;
            if let Some(ref mut lease) = *current {
                lease.cancel();
            }
            let old_epoch = current.as_ref().map(|l| l.epoch);
            drop(current);

            // Propagate cancellation for old epoch
            if let Some(epoch) = old_epoch {
                self.interrupt_propagator.propagate_cancellation(epoch);
            }
        }

        // Generate new epoch and create lease
        let epoch = self.epoch_manager.generate_epoch();
        let lease = EpochLease::new(epoch);

        {
            let mut current = self.current_lease.write().await;
            *current = Some(lease.clone());
        }

        lease
    }

    /// Returns the current epoch value.
    pub fn current_epoch(&self) -> u64 {
        self.epoch_manager.current_epoch()
    }

    /// Returns the interrupt propagator for registering context callbacks.
    pub fn interrupt_propagator(&self) -> &Arc<InterruptPropagator> {
        &self.interrupt_propagator
    }

    /// Returns the epoch manager.
    pub fn epoch_manager(&self) -> &Arc<EpochManager> {
        &self.epoch_manager
    }

    /// Gets the current lease state.
    pub async fn get_lease_state(&self) -> Option<LeaseState> {
        let current = self.current_lease.read().await;
        current.as_ref().map(|l| l.state)
    }
}

impl Default for LeaseCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Guardian that monitors heartbeat and triggers interrupt propagation on failure.
#[derive(Debug)]
pub struct HeartbeatGuardian {
    /// The lease coordinator for epoch management.
    coordinator: Arc<LeaseCoordinator>,
    /// Whether the guardian is currently running.
    is_running: RwLock<bool>,
}

impl HeartbeatGuardian {
    /// Creates a new HeartbeatGuardian wrapping the given coordinator.
    pub fn new(coordinator: Arc<LeaseCoordinator>) -> Self {
        Self {
            coordinator,
            is_running: RwLock::new(false),
        }
    }

    /// Called when heartbeat failure is detected.
    /// Triggers interrupt propagation for the current epoch.
    pub async fn on_heartbeat_failure(&self) {
        let current_epoch = self.coordinator.current_epoch();
        if current_epoch > 0 {
            tracing::warn!(
                epoch = current_epoch,
                "Heartbeat failure detected, propagating cancellation for epoch"
            );
            self.coordinator
                .interrupt_propagator
                .propagate_cancellation(current_epoch);
        }
    }

    /// Starts the guardian.
    pub async fn start(&self) {
        let mut running = self.is_running.write().await;
        *running = true;
    }

    /// Stops the guardian.
    pub async fn stop(&self) {
        let mut running = self.is_running.write().await;
        *running = false;
    }

    /// Returns whether the guardian is running.
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_acquire_lease_creates_new_epoch() {
        let coordinator = LeaseCoordinator::new();
        let lease1 = coordinator.acquire_lease().await;
        assert_eq!(lease1.epoch, 0);
        assert_eq!(lease1.state, LeaseState::Active);

        let lease2 = coordinator.acquire_lease().await;
        assert_eq!(lease2.epoch, 1);
        assert_eq!(lease2.state, LeaseState::Active);
    }

    #[tokio::test]
    async fn test_acquire_lease_cancels_previous() {
        let coordinator = LeaseCoordinator::new();
        let _lease1 = coordinator.acquire_lease().await;
        let _lease2 = coordinator.acquire_lease().await;

        // Coordinator should now have the second lease active
        let state = coordinator.get_lease_state().await;
        assert_eq!(state, Some(LeaseState::Active));
    }

    #[tokio::test]
    async fn test_epoch_manager_increments() {
        let manager = EpochManager::new();
        let e1 = manager.generate_epoch();
        let e2 = manager.generate_epoch();
        assert!(e2 > e1);
    }
}
