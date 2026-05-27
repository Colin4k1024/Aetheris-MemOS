//! Integration tests for OpenTelemetry time-travel debugging infrastructure.
//!
//! Tests the workflow event store operations and DAG reconstruction
//! from workflow events.

use backend::db::event_store::{EventStore, WorkflowEvent};
use backend::otel::{WorkflowSpanAttributes, WorkflowTraceContext};

#[test]
fn test_workflow_trace_context_creation() {
    let ctx = WorkflowTraceContext::new(
        "abc123def456abc123def456abc123de",
        "0123456789abcdef",
        Some("fedcba9876543210".to_string()),
    );
    assert_eq!(ctx.trace_id, "abc123def456abc123def456abc123de");
    assert_eq!(ctx.span_id, "0123456789abcdef");
    assert_eq!(ctx.parent_span_id, Some("fedcba9876543210".to_string()));
}

#[test]
fn test_workflow_span_attributes_to_kv() {
    let attrs = WorkflowSpanAttributes {
        workflow_instance_id: "wf-001".to_string(),
        attempt_id: "attempt-1".to_string(),
        epoch_id: Some("epoch-5".to_string()),
    };
    let kv = attrs.into_kv();
    assert_eq!(kv.len(), 3);

    let map: std::collections::HashMap<String, String> = kv.into_iter().collect();
    assert_eq!(map.get("workflow.instance_id"), Some(&"wf-001".to_string()));
    assert_eq!(
        map.get("workflow.attempt_id"),
        Some(&"attempt-1".to_string())
    );
    assert_eq!(map.get("workflow.epoch_id"), Some(&"epoch-5".to_string()));
}

#[test]
fn test_workflow_span_attributes_without_epoch() {
    let attrs = WorkflowSpanAttributes {
        workflow_instance_id: "wf-002".to_string(),
        attempt_id: "attempt-2".to_string(),
        epoch_id: None,
    };
    let kv = attrs.into_kv();
    assert_eq!(kv.len(), 2);
}

#[test]
fn test_workflow_event_creation() {
    let event = WorkflowEvent::new(
        "wf-test-001",
        "attempt-1",
        Some("parent-span-001".to_string()),
        "span_start",
        serde_json::json!({ "operation": "analyze" }),
    );

    assert_eq!(event.workflow_instance_id, "wf-test-001");
    assert_eq!(event.attempt_id, "attempt-1");
    assert_eq!(event.parent_span_id, Some("parent-span-001".to_string()));
    assert_eq!(event.event_type, "span_start");
    assert_eq!(event.payload["operation"], "analyze");
    assert!(!event.event_id.is_empty());
    assert!(!event.timestamp.is_empty());
}

#[test]
fn test_workflow_event_without_parent() {
    let event = WorkflowEvent::new(
        "wf-root",
        "attempt-1",
        None,
        "workflow_start",
        serde_json::json!({ "started": true }),
    );

    assert!(event.parent_span_id.is_none());
    assert_eq!(event.event_type, "workflow_start");
    assert_eq!(event.payload["started"], true);
}

#[test]
fn test_workflow_event_various_types() {
    let event_types = vec![
        "span_start",
        "span_end",
        "decision_start",
        "decision_end",
        "action_start",
        "action_end",
        "error",
        "workflow_complete",
    ];

    for event_type in event_types {
        let event = WorkflowEvent::new(
            "wf-001",
            "attempt-1",
            None,
            event_type,
            serde_json::json!({ "type": event_type }),
        );
        assert!(!event.event_id.is_empty());
        assert_eq!(event.event_type, event_type);
    }
}
