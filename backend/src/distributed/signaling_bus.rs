//! Inter-Workflow Signaling Bus
//!
//! Provides publish-subscribe messaging for workflow orchestration,
//! enabling sub-agent spawning, suspension, and termination signaling.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast::{self, Receiver, Sender};

/// Workflow signal types for inter-workflow communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WorkflowSignal {
    /// Signal that a sub-agent has been spawned.
    SubagentSpawn { child_workflow_id: String },
    /// Signal that a sub-agent should wake up.
    SubagentWake { workflow_id: String },
    /// Signal that a sub-agent should suspend execution.
    SubagentSuspend { workflow_id: String },
    /// Signal that a sub-agent should terminate.
    SubagentTerminate { workflow_id: String },
    /// Signal that a sub-agent has completed.
    SubagentComplete { workflow_id: String },
}

impl WorkflowSignal {
    /// Returns the signal type name for metadata.
    pub fn signal_type(&self) -> &'static str {
        match self {
            Self::SubagentSpawn { .. } => "SubagentSpawn",
            Self::SubagentWake { .. } => "SubagentWake",
            Self::SubagentSuspend { .. } => "SubagentSuspend",
            Self::SubagentTerminate { .. } => "SubagentTerminate",
            Self::SubagentComplete { .. } => "SubagentComplete",
        }
    }

    /// Extract the workflow_id from the signal.
    pub fn workflow_id(&self) -> Option<&str> {
        match self {
            Self::SubagentSpawn { child_workflow_id } => Some(child_workflow_id),
            Self::SubagentWake { workflow_id } => Some(workflow_id),
            Self::SubagentSuspend { workflow_id } => Some(workflow_id),
            Self::SubagentTerminate { workflow_id } => Some(workflow_id),
            Self::SubagentComplete { workflow_id } => Some(workflow_id),
        }
    }
}

/// Metadata associated with a workflow signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalMetadata {
    /// The parent workflow that published this signal.
    pub parent_workflow_id: String,
    /// When the signal was published.
    pub timestamp: DateTime<Utc>,
    /// The type of signal.
    pub signal_type: String,
}

/// Stored signal with its metadata for querying.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSignal {
    pub signal: WorkflowSignal,
    pub metadata: SignalMetadata,
}

/// The signaling bus for inter-workflow communication.
///
/// Uses a broadcast channel to allow multiple subscribers to receive
/// signals for specific workflows.
pub struct SignalingBus {
    /// Event store: parent_workflow_id -> Vec<StoredSignal>
    event_store: Arc<RwLock<HashMap<String, Vec<StoredSignal>>>>,
    /// Per-workflow broadcast channels, keyed by the workflow a signal targets
    /// (`WorkflowSignal::workflow_id`). A subscriber only receives signals aimed
    /// at its own workflow_id, never another workflow's.
    subscriptions: Arc<RwLock<HashMap<String, Sender<WorkflowSignal>>>>,
}

impl SignalingBus {
    /// Create a new signaling bus.
    pub fn new() -> Self {
        Self {
            event_store: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Publish a signal to the bus, storing it and delivering it only to
    /// subscribers of the workflow the signal targets.
    pub fn publish(&self, signal: WorkflowSignal, parent_workflow_id: &str) {
        let metadata = SignalMetadata {
            parent_workflow_id: parent_workflow_id.to_string(),
            timestamp: Utc::now(),
            signal_type: signal.signal_type().to_string(),
        };

        // Store the signal in the event store
        let stored = StoredSignal {
            signal: signal.clone(),
            metadata,
        };

        {
            let mut store = self.event_store.write().unwrap();
            store
                .entry(parent_workflow_id.to_string())
                .or_insert_with(Vec::new)
                .push(stored);
        }

        // Route only to subscribers of the targeted workflow (no cross-talk).
        if let Some(target) = signal.workflow_id().map(|s| s.to_string()) {
            let subs = self.subscriptions.read().unwrap();
            if let Some(sender) = subs.get(&target) {
                let _ = sender.send(signal);
            }
        }
    }

    /// Subscribe to signals targeting a specific workflow.
    /// Returns a receiver that only yields signals whose `workflow_id` matches.
    pub fn subscribe(&self, workflow_id: &str) -> Receiver<WorkflowSignal> {
        let mut subs = self.subscriptions.write().unwrap();
        subs.entry(workflow_id.to_string())
            .or_insert_with(|| broadcast::channel(1024).0)
            .subscribe()
    }

    /// Get all signals for a parent workflow.
    pub fn get_parent_signals(
        &self,
        parent_workflow_id: &str,
    ) -> Vec<(WorkflowSignal, SignalMetadata)> {
        let store = self.event_store.read().unwrap();
        store
            .get(parent_workflow_id)
            .map(|signals| {
                signals
                    .iter()
                    .map(|s| (s.signal.clone(), s.metadata.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all stored signals (for debugging/admin purposes).
    pub fn get_all_signals(&self) -> Vec<(String, Vec<StoredSignal>)> {
        let store = self.event_store.read().unwrap();
        store.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    /// Clear all stored signals for a workflow.
    pub fn clear_signals(&self, workflow_id: &str) {
        let mut store = self.event_store.write().unwrap();
        store.remove(workflow_id);
    }
}

impl Default for SignalingBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_signal_routed_to_target_workflow() {
        let bus = SignalingBus::new();

        // Subscribe by the workflow the signal targets.
        let mut rx = bus.subscribe("child-1");

        let signal = WorkflowSignal::SubagentSpawn {
            child_workflow_id: "child-1".to_string(),
        };
        bus.publish(signal.clone(), "workflow-1");

        // The targeted subscriber receives it.
        let received = rx.recv().await.unwrap();
        assert!(matches!(received, WorkflowSignal::SubagentSpawn { .. }));

        // Stored under the publishing parent for querying.
        let signals = bus.get_parent_signals("workflow-1");
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].1.parent_workflow_id, "workflow-1");
    }

    #[tokio::test]
    async fn test_multiple_subscribers_same_workflow() {
        let bus = SignalingBus::new();

        let mut rx1 = bus.subscribe("workflow-1");
        let mut rx2 = bus.subscribe("workflow-1");

        bus.publish(
            WorkflowSignal::SubagentWake {
                workflow_id: "workflow-1".to_string(),
            },
            "parent-1",
        );

        // Both receivers on the same workflow get the signal.
        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();

        assert!(matches!(received1, WorkflowSignal::SubagentWake { .. }));
        assert!(matches!(received2, WorkflowSignal::SubagentWake { .. }));
    }

    #[tokio::test]
    async fn test_signal_not_delivered_to_other_workflow() {
        let bus = SignalingBus::new();

        let mut rx_a = bus.subscribe("workflow-A");
        let _rx_b = bus.subscribe("workflow-B"); // keep B's channel alive

        // Terminate targets workflow-B; workflow-A must not receive it.
        bus.publish(
            WorkflowSignal::SubagentTerminate {
                workflow_id: "workflow-B".to_string(),
            },
            "parent-1",
        );

        assert!(rx_a.try_recv().is_err());
    }

    #[test]
    fn test_signal_metadata() {
        let signal = WorkflowSignal::SubagentTerminate {
            workflow_id: "wf-123".to_string(),
        };

        assert_eq!(signal.signal_type(), "SubagentTerminate");
        assert_eq!(signal.workflow_id(), Some("wf-123"));
    }

    #[test]
    fn test_get_parent_signals_empty() {
        let bus = SignalingBus::new();
        let signals = bus.get_parent_signals("nonexistent");
        assert!(signals.is_empty());
    }
}
