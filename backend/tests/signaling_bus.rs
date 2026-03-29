//! Integration tests for Signaling Bus and Sub-Agent Pool

use backend::distributed::signaling_bus::{SignalingBus, WorkflowSignal};
use backend::runtime::subagent_pool::{PoolStatus, SubagentPool};

#[tokio::test]
async fn test_signal_publish_and_subscribe() {
    let bus = SignalingBus::new();

    // Subscribe to a workflow
    let mut rx = bus.subscribe("workflow-1");

    // Publish a SubagentSpawn signal
    let signal = WorkflowSignal::SubagentSpawn {
        child_workflow_id: "child-1".to_string(),
    };
    bus.publish(signal.clone(), "workflow-1");

    // Receive should get the signal
    let received = rx.recv().await.unwrap();
    assert!(matches!(received, WorkflowSignal::SubagentSpawn { .. }));

    // Verify stored signals
    let signals = bus.get_parent_signals("workflow-1");
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].1.parent_workflow_id, "workflow-1");
    assert_eq!(signals[0].1.signal_type, "SubagentSpawn");
}

#[tokio::test]
async fn test_subagent_pool_status() {
    let pool = SubagentPool::new(4);

    // Initially all slots should be idle
    let status = pool.status().await;
    assert_eq!(status.idle_slots, 4);
    assert_eq!(status.busy_slots, 0);
}

#[tokio::test]
async fn test_pool_allocation() {
    let pool = SubagentPool::new(4);

    // Allocate 2 slots
    let allocated = pool.allocate(2).await;
    assert_eq!(allocated.len(), 2);

    let status = pool.status().await;
    assert_eq!(status.idle_slots, 2);
    assert_eq!(status.busy_slots, 2);

    // Release the slots
    pool.release(&allocated).await;

    let status = pool.status().await;
    assert_eq!(status.idle_slots, 4);
    assert_eq!(status.busy_slots, 0);
}

#[tokio::test]
async fn test_pool_allocation_limit() {
    let pool = SubagentPool::new(2);

    // Allocate more than available
    let allocated = pool.allocate(5).await;
    assert_eq!(allocated.len(), 2); // Should only get 2

    let status = pool.status().await;
    assert_eq!(status.idle_slots, 0);
    assert_eq!(status.busy_slots, 2);
}

#[tokio::test]
async fn test_get_parent_signals_empty() {
    let bus = SignalingBus::new();
    let signals = bus.get_parent_signals("nonexistent");
    assert!(signals.is_empty());
}
