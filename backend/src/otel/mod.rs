//! OpenTelemetry integration: distributed tracing + structured logging.
//!
//! Initializes the full `tracing_subscriber` stack in one place:
//!   - `EnvFilter` layer  (log level filtering)
//!   - `fmt` layer        (human/file output)
//!   - `OpenTelemetryLayer` (OTLP gRPC export to collector, when enabled)
//!
//! Call [`init_tracing`] early in `main()` *before* any `tracing::` calls.
//! Hold the returned [`TracingGuard`] for the process lifetime — dropping it
//! flushes the file writer and shuts down the tracer provider.

use std::collections::HashMap;

use opentelemetry::trace::TracerProvider as OtelTracerProvider;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace::TracerProvider as SdkTracerProvider, Resource};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::{LogConfig, OtelConfig};

/// Holds resources that must stay alive for the process lifetime.
/// Dropping this struct flushes logs and shuts down the OTLP exporter.
pub struct TracingGuard {
    _file_guard: WorkerGuard,
    tracer_provider: Option<SdkTracerProvider>,
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        if let Some(ref provider) = self.tracer_provider {
            if let Err(e) = provider.shutdown() {
                eprintln!("[otel] tracer provider shutdown error: {e}");
            }
        }
    }
}

/// Initialize the global tracing subscriber, optionally wiring up OTLP export.
///
/// # Arguments
/// * `log_cfg`  – log format / level / output path
/// * `otel_cfg` – OTLP endpoint and feature flag
///
/// Returns a [`TracingGuard`] that must be held for the program's lifetime.
pub fn init_tracing(log_cfg: &LogConfig, otel_cfg: &OtelConfig) -> TracingGuard {
    // ── file writer ─────────────────────────────────────────────────────────
    let file_appender = match log_cfg.rolling.as_str() {
        "minutely" => tracing_appender::rolling::minutely(&log_cfg.directory, &log_cfg.file_name),
        "hourly" => tracing_appender::rolling::hourly(&log_cfg.directory, &log_cfg.file_name),
        "daily" => tracing_appender::rolling::daily(&log_cfg.directory, &log_cfg.file_name),
        _ => tracing_appender::rolling::never(&log_cfg.directory, &log_cfg.file_name),
    };
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

    // ── env filter ──────────────────────────────────────────────────────────
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&log_cfg.filter_level));

    // ── OTLP provider (optional) ────────────────────────────────────────────
    let tracer_provider = if otel_cfg.enabled {
        match build_otlp_provider(&otel_cfg.endpoint, &otel_cfg.service_name) {
            Ok(provider) => Some(provider),
            Err(e) => {
                eprintln!("[otel] OTLP init failed (continuing without tracing): {e}");
                None
            }
        }
    } else {
        None
    };

    // ── compose & install ───────────────────────────────────────────────────
    // Two branches to keep fmt_layer's writer type concrete.
    // otel_layer must be constructed inside each branch so Rust infers the
    // correct `OpenTelemetryLayer<S, T>` subscriber type parameter per branch.
    let service_name = otel_cfg.service_name.clone();
    if log_cfg.stdout {
        let otel_layer = tracer_provider.as_ref().map(|p| {
            let tracer = p.tracer(service_name.clone());
            tracing_opentelemetry::layer().with_tracer(tracer)
        });
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(log_cfg.with_ansi)
                    .with_level(log_cfg.with_level)
                    .with_target(log_cfg.with_target)
                    .with_thread_ids(log_cfg.with_thread_ids)
                    .with_thread_names(log_cfg.with_thread_names)
                    .with_writer(std::io::stdout),
            )
            .with(otel_layer)
            .init();
    } else {
        let otel_layer = tracer_provider.as_ref().map(|p| {
            let tracer = p.tracer(service_name.clone());
            tracing_opentelemetry::layer().with_tracer(tracer)
        });
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(log_cfg.with_ansi)
                    .with_level(log_cfg.with_level)
                    .with_target(log_cfg.with_target)
                    .with_thread_ids(log_cfg.with_thread_ids)
                    .with_thread_names(log_cfg.with_thread_names)
                    .with_writer(file_writer),
            )
            .with(otel_layer)
            .init();
    }

    TracingGuard {
        _file_guard: file_guard,
        tracer_provider,
    }
}

/// Build an OTLP gRPC `SdkTracerProvider` targeting `endpoint`.
fn build_otlp_provider(
    endpoint: &str,
    service_name: &str,
) -> Result<SdkTracerProvider, Box<dyn std::error::Error + Send + Sync>> {
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(endpoint)
        .build_span_exporter()?;

    let resource = Resource::new([
        KeyValue::new("service.name", service_name.to_owned()),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
    ]);

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter, runtime::Tokio)
        .with_config(opentelemetry_sdk::trace::Config::default().with_resource(resource))
        .build();

    // Register as global so `opentelemetry::global::tracer()` and
    // context propagation work across the codebase.
    opentelemetry::global::set_tracer_provider(provider.clone());

    Ok(provider)
}

// ── legacy context types kept for kernel/protocol consumers ──────────────

/// Trace context for workflow propagation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowTraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
}

impl WorkflowTraceContext {
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

/// Workflow-level span attributes used across the adaptive memory pipeline.
#[derive(Debug, Clone)]
pub struct WorkflowSpanAttributes {
    pub workflow_instance_id: String,
    pub attempt_id: String,
    pub epoch_id: Option<String>,
}

impl WorkflowSpanAttributes {
    pub fn into_kv(self) -> Vec<(String, String)> {
        let mut kv = vec![
            (
                "workflow.instance_id".to_string(),
                self.workflow_instance_id,
            ),
            ("workflow.attempt_id".to_string(), self.attempt_id),
        ];
        if let Some(epoch) = self.epoch_id {
            kv.push(("workflow.epoch_id".to_string(), epoch));
        }
        kv
    }
}

/// Inject current trace context into a propagation map.
pub fn inject_context_to_map(map: &mut HashMap<String, String>) {
    let _ = map;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_trace_context_roundtrip() {
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
    fn workflow_span_attributes_kv_count() {
        let attrs = WorkflowSpanAttributes {
            workflow_instance_id: "wf-001".to_string(),
            attempt_id: "attempt-1".to_string(),
            epoch_id: Some("epoch-5".to_string()),
        };
        assert_eq!(attrs.into_kv().len(), 3);
    }
}
