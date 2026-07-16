//! In-process coordination primitives (NOT a distributed cluster).
//!
//! Provides in-process cancellation (epoch), interrupt propagation, lease
//! coordination, and workflow signaling used by the runtime. This is deliberately
//! NOT a distributed consensus/replication/sharding layer: high availability is
//! delegated to managed infrastructure (see
//! `docs/adr/ADR-0005-ha-infrastructure-selection.md`). The former
//! consensus/node/replication/sharding "cluster" modules were removed as unused
//! stub dead code (P0 build-front cleanup); do not reintroduce hand-rolled
//! consensus here.

pub mod epoch_manager;
pub mod interrupt_propagator;
pub mod lease_coordinator;
pub mod signaling_bus;

pub use epoch_manager::{CancellationFunc, EpochContext, EpochManager, RegisteredContext};
pub use interrupt_propagator::InterruptPropagator;
