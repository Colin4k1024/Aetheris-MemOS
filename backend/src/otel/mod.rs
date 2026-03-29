//! OpenTelemetry integration for time-travel debugging and DAG holographic replay.
//!
//! Provides distributed tracing capabilities with workflow-level context tracking,
//! enabling precise reconstruction of execution DAGs for debugging and analysis.
//!
//! This module provides the core types and context management. Telemetry initialization
//! requires OTLP collector infrastructure and should be called explicitly at startup
//! if tracing is desired.

use std::collections::HashMap;
use std::sync::OnceLock;
use tracing::warn;

/// Global tracer flag — set to true when telemetry is initialized.
static TELEMETRY_INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Trace context extracted from an active span for propagation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowTraceContext {
    /// 32-character hex-encoded trace ID.
    pub trace_id: String,
    /// 16-character hex-encoded span ID.
    pub span_id: String,
    /// Optional parent span ID for hierarchical spans.
    pub parent_span_id: Option<String>,
}

impl WorkflowTraceContext {
    /// Create a new context from raw values.
    pub fn new(
        trace_id: impl Into<String>,
        span_id: impl Into<String>,
        parent_span_id: Option<String>,
    ) -> Self {
        Self {
            trace_id: trace_id.into(),
            span_id: span_id.into(),
            parent_span_id,
        }
    }
}

/// Initialize OpenTelemetry tracing with OTLP exporter.
///
/// This is a placeholder that marks telemetry as initialized.
/// Full OTLP initialization requires specific gRPC/TLS infrastructure.
/// For production use, integrate with your observability backend
/// (Jaeger, Tempo, etc.) via environment-configured OTLP endpoint.
pub fn init_telemetry() -> Result<(), String> {
    TELEMETRY_INITIALIZED
        .set(true)
        .map_err(|_| "telemetry already initialized".to_string())?;
    tracing::info!("OpenTelemetry telemetry initialized");
    Ok(())
}

/// Returns true if telemetry has been initialized.
pub fn is_telemetry_initialized() -> bool {
    TELEMETRY_INITIALIZED.get() == Some(&true)
}

/// Workflow-level span attributes used across the adaptive memory pipeline.
#[derive(Debug, Clone)]
pub struct WorkflowSpanAttributes {
    pub workflow_instance_id: String,
    pub attempt_id: String,
    pub epoch_id: Option<String>,
}

impl WorkflowSpanAttributes {
    /// Convert to key-value pairs for span creation.
    pub fn into_kv(self) -> Vec<(String, String)> {
        let mut kv = vec![
            ("workflow.instance_id".to_string(), self.workflow_instance_id),
            ("workflow.attempt_id".to_string(), self.attempt_id),
        ];
        if let Some(epoch) = self.epoch_id {
            kv.push(("workflow.epoch_id".to_string(), epoch));
        }
        kv
    }
}

/// Inject current trace context into a HashMap for propagation.
/// In full implementation, this would extract from the active OpenTelemetry span.
pub fn inject_context_to_map(map: &mut HashMap<String, String>) {
    // Placeholder: in full implementation, extract from active span via OpenTelemetry
    // map.insert("trace_id".to_string(), ctx.trace_id);
    let _ = map;
}

/// Shutdown the global tracer provider, flushing any pending spans.
pub fn shutdown_telemetry() {
    // In full implementation, call opentelemetry::global::shutdown_tracer_provider()
    tracing::info!("OpenTelemetry telemetry shutdown");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_trace_context_from_values() {
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
    fn workflow_span_attributes_kv() {
        let attrs = WorkflowSpanAttributes {
            workflow_instance_id: "wf-001".to_string(),
            attempt_id: "attempt-1".to_string(),
            epoch_id: Some("epoch-5".to_string()),
        };
        let kv = attrs.into_kv();
        assert_eq!(kv.len(), 3);
    }
}
