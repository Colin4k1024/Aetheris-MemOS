//! Prometheus Metrics Exporter
//!
//! Exports internal metrics in Prometheus text format for scraping.

use std::time::Duration;

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::services::metrics;

/// Prometheus metrics exporter state
pub struct PrometheusExporter {
    /// Active STM sessions count
    stm_sessions_active: prometheus::Gauge,
    /// Total LTM entries count
    ltm_entries_total: prometheus::Gauge,
    /// Weight adjustment operations counter
    weight_adjustments_total: prometheus::Counter,
    /// STM to LTM transfer operations counter
    transfer_operations_total: prometheus::Counter,
    /// Search duration histogram (in seconds)
    search_duration_seconds: prometheus::Histogram,
    /// Request duration histogram (in seconds)
    request_duration_seconds: prometheus::Histogram,
    /// Requests counter by endpoint and status
    requests_total: prometheus::CounterVec,
    /// Tenant quota usage ratio
    tenant_quota_usage_ratio: prometheus::GaugeVec,
    /// Prometheus registry for metric collection
    registry: prometheus::Registry,
}

impl Default for PrometheusExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl PrometheusExporter {
    /// Create a new Prometheus exporter
    pub fn new() -> Self {
        let registry = prometheus::Registry::new();

        let stm_sessions_active = prometheus::Gauge::new(
            "memory_stm_sessions_active",
            "Number of active STM sessions",
        )
        .expect("gauge creation failed");

        let ltm_entries_total =
            prometheus::Gauge::new("memory_ltm_entries_total", "Total number of LTM entries")
                .expect("gauge creation failed");

        let weight_adjustments_total = prometheus::Counter::new(
            "memory_weight_adjustments_total",
            "Total number of weight adjustment operations",
        )
        .expect("counter creation failed");

        let transfer_operations_total = prometheus::Counter::new(
            "memory_transfer_operations_total",
            "Total number of STM to LTM transfer operations",
        )
        .expect("counter creation failed");

        let search_duration_seconds = prometheus::Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "memory_search_duration_seconds",
                "Search operation duration in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
        )
        .expect("histogram creation failed");

        let request_duration_seconds = prometheus::Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "memory_request_duration_seconds",
                "HTTP request duration in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
        )
        .expect("histogram creation failed");

        let requests_total = prometheus::CounterVec::new(
            prometheus::Opts::new(
                "memory_requests_total",
                "Total requests by endpoint and status",
            ),
            &["endpoint", "status"],
        )
        .expect("countervec creation failed");

        let tenant_quota_usage_ratio = prometheus::GaugeVec::new(
            prometheus::Opts::new(
                "tenant_quota_usage_ratio",
                "Quota usage ratio per tenant (0.0 to 1.0)",
            ),
            &["tenant"],
        )
        .expect("gauagevec creation failed");

        // Register all metrics with the registry
        registry
            .register(Box::new(stm_sessions_active.clone()))
            .expect("failed to register stm_sessions_active");
        registry
            .register(Box::new(ltm_entries_total.clone()))
            .expect("failed to register ltm_entries_total");
        registry
            .register(Box::new(weight_adjustments_total.clone()))
            .expect("failed to register weight_adjustments_total");
        registry
            .register(Box::new(transfer_operations_total.clone()))
            .expect("failed to register transfer_operations_total");
        registry
            .register(Box::new(search_duration_seconds.clone()))
            .expect("failed to register search_duration_seconds");
        registry
            .register(Box::new(request_duration_seconds.clone()))
            .expect("failed to register request_duration_seconds");
        registry
            .register(Box::new(requests_total.clone()))
            .expect("failed to register requests_total");
        registry
            .register(Box::new(tenant_quota_usage_ratio.clone()))
            .expect("failed to register tenant_quota_usage_ratio");

        Self {
            stm_sessions_active,
            ltm_entries_total,
            weight_adjustments_total,
            transfer_operations_total,
            search_duration_seconds,
            request_duration_seconds,
            requests_total,
            tenant_quota_usage_ratio,
            registry,
        }
    }

    /// Record a request completion
    pub fn record_request(&self, endpoint: &str, status: u16, duration: Duration) {
        self.requests_total
            .with_label_values(&[endpoint, &status.to_string()])
            .inc();
        self.request_duration_seconds
            .observe(duration.as_secs_f64());
    }

    /// Record search operation duration
    pub fn record_search_duration(&self, duration_secs: f64) {
        self.search_duration_seconds.observe(duration_secs);
    }

    /// Set active STM sessions count
    pub fn set_stm_sessions_active(&self, count: f64) {
        self.stm_sessions_active.set(count);
    }

    /// Set total LTM entries count
    pub fn set_ltm_entries_total(&self, count: f64) {
        self.ltm_entries_total.set(count);
    }

    /// Increment weight adjustment counter
    pub fn increment_weight_adjustments(&self) {
        self.weight_adjustments_total.inc();
    }

    /// Increment transfer operations counter
    pub fn increment_transfer_operations(&self) {
        self.transfer_operations_total.inc();
    }

    /// Set tenant quota usage ratio
    pub fn set_tenant_quota_usage(&self, tenant: &str, ratio: f64) {
        self.tenant_quota_usage_ratio
            .with_label_values(&[tenant])
            .set(ratio);
    }

    /// Convert internal metrics to Prometheus format
    pub fn export_internal_metrics(&self) -> String {
        let mut output = String::new();

        // Export internal metrics from the metrics service
        let internal_metrics = metrics::get_metrics().get_metrics();

        for event in internal_metrics {
            let metric_name = format!("memory_internal_{}_total", event.op_type.as_str());
            let help_text = format!("Internal metric for {} operations", event.op_type.as_str());

            output.push_str(&format!("# HELP {} {}\n", metric_name, help_text));
            output.push_str(&format!("# TYPE {} counter\n", metric_name));

            let total = event.failure.total();
            output.push_str(&format!(
                "{}{{bucket=\"{}\"}} {}\n",
                metric_name, event.bucket, total
            ));
        }

        output
    }

    /// Generate Prometheus text format output
    pub fn generate_prometheus_output(&self) -> String {
        let mut output = String::new();

        // Gather metrics from the registry
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        if let Ok(encoded) = encoder.encode_to_string(&metric_families) {
            output.push_str(&encoded);
        }

        // Export internal metrics from metrics service
        output.push_str(&self.export_internal_metrics());

        output
    }
}

/// Global Prometheus exporter instance
static PROMETHEUS_EXPORTER: std::sync::OnceLock<PrometheusExporter> = std::sync::OnceLock::new();

/// Get the global Prometheus exporter instance
pub fn get_exporter() -> &'static PrometheusExporter {
    PROMETHEUS_EXPORTER.get_or_init(PrometheusExporter::new)
}

/// Initialize the Prometheus exporter with custom configuration
pub fn init_exporter() -> &'static PrometheusExporter {
    get_exporter()
}

/// Metrics endpoint handler returning Prometheus text format
pub async fn metrics_handler() -> Response {
    let exporter = get_exporter();
    let output = exporter.generate_prometheus_output();

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "text/plain; version=0.0.4".parse().unwrap(),
    );

    (StatusCode::OK, headers, output).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prometheus_exporter_creation() {
        let exporter = PrometheusExporter::new();
        assert_eq!(exporter.stm_sessions_active.get(), 0.0);
        assert_eq!(exporter.ltm_entries_total.get(), 0.0);
    }

    #[test]
    fn test_set_stm_sessions() {
        let exporter = PrometheusExporter::new();
        exporter.set_stm_sessions_active(5.0);
        assert_eq!(exporter.stm_sessions_active.get(), 5.0);
    }

    #[test]
    fn test_set_ltm_entries() {
        let exporter = PrometheusExporter::new();
        exporter.set_ltm_entries_total(100.0);
        assert_eq!(exporter.ltm_entries_total.get(), 100.0);
    }

    #[test]
    fn test_increment_counters() {
        let exporter = PrometheusExporter::new();
        exporter.increment_weight_adjustments();
        exporter.increment_weight_adjustments();
        exporter.increment_transfer_operations();
        // No panic means success
    }

    #[test]
    fn test_tenant_quota_usage() {
        let exporter = PrometheusExporter::new();
        exporter.set_tenant_quota_usage("tenant1", 0.75);
        exporter.set_tenant_quota_usage("tenant2", 0.50);
        // No panic means success
    }

    #[test]
    fn test_generate_prometheus_output() {
        let exporter = PrometheusExporter::new();
        exporter.set_stm_sessions_active(10.0);
        exporter.set_ltm_entries_total(200.0);

        let output = exporter.generate_prometheus_output();

        assert!(output.contains("memory_stm_sessions_active"));
        assert!(output.contains("memory_ltm_entries_total"));
    }

    #[test]
    fn test_record_search_duration() {
        let exporter = PrometheusExporter::new();
        exporter.record_search_duration(0.05);
        exporter.record_search_duration(0.12);
        // No panic means success
    }

    #[test]
    fn test_record_request() {
        let exporter = PrometheusExporter::new();
        exporter.record_request("/api/memory", 200, Duration::from_millis(50));
        exporter.record_request("/api/memory", 500, Duration::from_millis(200));
        // No panic means success
    }

    #[test]
    fn test_global_exporter_singleton() {
        let exporter1 = get_exporter();
        let exporter2 = get_exporter();
        assert!(std::ptr::eq(exporter1, exporter2));
    }
}
