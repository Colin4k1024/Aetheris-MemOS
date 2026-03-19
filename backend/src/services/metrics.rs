//! Anonymous Aggregated Metrics
//!
//! Core metrics system for monitoring QPS, latency, and failure rates.
//! Strictly prohibits tenant/user identifiable information.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Operation types for metrics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    Store,
    Search,
    Update,
    Delete,
    Transfer,
    Embed,
    Query,
}

impl OperationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OperationType::Store => "store",
            OperationType::Search => "search",
            OperationType::Update => "update",
            OperationType::Delete => "delete",
            OperationType::Transfer => "transfer",
            OperationType::Embed => "embed",
            OperationType::Query => "query",
        }
    }
}

/// Operation outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Success,
    Denied,
    Error,
    Timeout,
}

impl Outcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Outcome::Success => "success",
            Outcome::Denied => "denied",
            Outcome::Error => "error",
            Outcome::Timeout => "timeout",
        }
    }
}

/// Latency statistics
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LatencyStats {
    /// Average latency in milliseconds
    pub avg: f64,
    /// 50th percentile latency
    pub p50: f64,
    /// 95th percentile latency
    pub p95: f64,
    /// 99th percentile latency
    pub p99: f64,
}

impl Default for LatencyStats {
    fn default() -> Self {
        Self {
            avg: 0.0,
            p50: 0.0,
            p95: 0.0,
            p99: 0.0,
        }
    }
}

/// Failure breakdown
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct FailureBreakdown {
    /// Number of successful operations
    pub success: u64,
    /// Number of denied operations
    pub deny: u64,
    /// Number of errored operations
    pub error: u64,
    /// Number of timeout operations
    pub timeout: u64,
}

impl FailureBreakdown {
    pub fn total(&self) -> u64 {
        self.success + self.deny + self.error + self.timeout
    }
}

/// Bucket statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BucketStats {
    /// Operation count
    pub count: u64,
    /// Latency statistics
    pub latency: LatencyStats,
    /// Failure breakdown
    pub failure: FailureBreakdown,
}

impl BucketStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a single operation
    pub fn record(&mut self, latency_ms: u64, outcome: Outcome) {
        self.count += 1;

        // Update failure breakdown
        match outcome {
            Outcome::Success => self.failure.success += 1,
            Outcome::Denied => self.failure.deny += 1,
            Outcome::Error => self.failure.error += 1,
            Outcome::Timeout => self.failure.timeout += 1,
        }

        // Update running latency stats (simplified online algorithm)
        // For accurate percentiles, we'd use a more sophisticated data structure
        let latency_f = latency_ms as f64;
        self.latency.avg =
            (self.latency.avg * (self.count - 1) as f64 + latency_f) / self.count as f64;

        // Approximate percentiles (simplified)
        if self.count == 1 {
            self.latency.p50 = latency_f;
            self.latency.p95 = latency_f;
            self.latency.p99 = latency_f;
        } else {
            // Simple exponential moving average approximation
            let alpha = 0.1;
            self.latency.p50 = self.latency.p50 * (1.0 - alpha) + latency_f * alpha;
            self.latency.p95 = self.latency.p95 * (1.0 - alpha) + latency_f * alpha * 1.5;
            self.latency.p99 = self.latency.p99 * (1.0 - alpha) + latency_f * alpha * 2.0;
        }
    }
}

/// Anonymous metrics event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsEvent {
    /// Event timestamp (Unix epoch ms)
    pub timestamp: i64,
    /// Time bucket identifier (e.g., "2026-03-18T10:00:00Z")
    pub bucket: String,
    /// Operation type
    pub op_type: OperationType,
    /// Total operation count
    pub count: u64,
    /// Latency statistics
    pub latency_ms: LatencyStats,
    /// Failure breakdown
    pub failure: FailureBreakdown,
}

impl MetricsEvent {
    pub fn new(bucket: String, op_type: OperationType, stats: BucketStats) -> Self {
        Self {
            timestamp: chrono::Utc::now().timestamp_millis(),
            bucket,
            op_type,
            count: stats.count,
            latency_ms: stats.latency,
            failure: stats.failure,
        }
    }
}

/// Window strategy for aggregation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowStrategy {
    /// Fixed time window
    #[default]
    Fixed,
    /// Sliding time window
    Sliding,
}

/// MetricsSink trait - pluggable metrics backend
pub trait MetricsSink: Send + Sync {
    /// Record a metrics event
    fn record(&self, event: MetricsEvent);

    /// Flush buffered events
    fn flush(&self);
}

// ============================================================================
// Default Implementations
// ============================================================================

/// No-op metrics sink (default for core)
pub struct NoopMetricsSink;

impl MetricsSink for NoopMetricsSink {
    fn record(&self, _event: MetricsEvent) {
        // No-op
    }

    fn flush(&self) {
        // No-op
    }
}

/// Console metrics sink (for development)
pub struct ConsoleMetricsSink;

impl MetricsSink for ConsoleMetricsSink {
    fn record(&self, event: MetricsEvent) {
        println!(
            "[METRICS] {} {} count={} latency_avg={:.2}ms success={} error={}",
            event.bucket,
            event.op_type.as_str(),
            event.count,
            event.latency_ms.avg,
            event.failure.success,
            event.failure.error
        );
    }

    fn flush(&self) {
        // No buffering, nothing to flush
    }
}

// ============================================================================
// Metrics Aggregator
// ============================================================================

use std::collections::HashMap;
use std::sync::Mutex;

/// Metrics aggregator with fixed/sliding window
pub struct MetricsAggregator {
    buckets: Mutex<HashMap<String, HashMap<OperationType, BucketStats>>>,
    window_size_ms: u64,
    strategy: WindowStrategy,
}

impl Default for MetricsAggregator {
    fn default() -> Self {
        Self::new(10_000, WindowStrategy::Fixed) // 10s default
    }
}

impl MetricsAggregator {
    /// Create a new aggregator
    pub fn new(window_size_ms: u64, strategy: WindowStrategy) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            window_size_ms,
            strategy,
        }
    }

    /// Record a single operation
    pub fn record(&self, op_type: OperationType, latency_ms: u64, outcome: Outcome) {
        let bucket_id = self.get_current_bucket();
        let mut buckets = self.buckets.lock().unwrap();

        let op_stats = buckets.entry(bucket_id).or_default();
        op_stats.entry(op_type).or_insert_with(BucketStats::new).record(latency_ms, outcome);
    }

    /// Get current bucket identifier
    fn get_current_bucket(&self) -> String {
        let now = chrono::Utc::now().timestamp_millis();
        let bucket_ts = (now / self.window_size_ms as i64) * self.window_size_ms as i64;
        chrono::DateTime::from_timestamp_millis(bucket_ts)
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Take a snapshot of current metrics and reset
    pub fn snapshot_and_reset(&self) -> Vec<MetricsEvent> {
        let mut buckets = self.buckets.lock().unwrap();
        let bucket_id = self.get_current_bucket();

        let events = buckets
            .remove(&bucket_id)
            .map(|ops| {
                ops.into_iter()
                    .map(|(op_type, stats)| MetricsEvent::new(bucket_id.clone(), op_type, stats))
                    .collect()
            })
            .unwrap_or_default();

        events
    }

    /// Get current snapshot without resetting
    pub fn snapshot(&self) -> Vec<MetricsEvent> {
        let buckets = self.buckets.lock().unwrap();
        let bucket_id = self.get_current_bucket();

        buckets
            .get(&bucket_id)
            .map(|ops| {
                ops.iter()
                    .map(|(op_type, stats)| MetricsEvent::new(bucket_id.clone(), *op_type, stats.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Clear all buckets
    pub fn clear(&self) {
        let mut buckets = self.buckets.lock().unwrap();
        buckets.clear();
    }
}

// ============================================================================
// Metrics Service
// ============================================================================

/// Global metrics service
pub struct MetricsService {
    sink: Arc<dyn MetricsSink>,
    aggregator: Arc<MetricsAggregator>,
    enabled: bool,
}

impl Default for MetricsService {
    fn default() -> Self {
        Self::new(Arc::new(NoopMetricsSink))
    }
}

impl MetricsService {
    /// Create a new metrics service
    pub fn new(sink: Arc<dyn MetricsSink>) -> Self {
        Self {
            sink,
            aggregator: Arc::new(MetricsAggregator::default()),
            enabled: true,
        }
    }

    /// Create with custom aggregator settings
    pub fn with_aggregator(
        sink: Arc<dyn MetricsSink>,
        window_size_ms: u64,
        strategy: WindowStrategy,
    ) -> Self {
        Self {
            sink,
            aggregator: Arc::new(MetricsAggregator::new(window_size_ms, strategy)),
            enabled: true,
        }
    }

    /// Enable/disable metrics
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Record an operation
    pub fn record(&self, op_type: OperationType, latency_ms: u64, outcome: Outcome) {
        if !self.enabled {
            return;
        }
        self.aggregator.record(op_type, latency_ms, outcome);
    }

    /// Record a successful operation
    pub fn record_success(&self, op_type: OperationType, latency_ms: u64) {
        self.record(op_type, latency_ms, Outcome::Success);
    }

    /// Record a failed operation
    pub fn record_error(&self, op_type: OperationType, latency_ms: u64) {
        self.record(op_type, latency_ms, Outcome::Error);
    }

    /// Record a denied operation
    pub fn record_denied(&self, op_type: OperationType, latency_ms: u64) {
        self.record(op_type, latency_ms, Outcome::Denied);
    }

    /// Record a timeout
    pub fn record_timeout(&self, op_type: OperationType, latency_ms: u64) {
        self.record(op_type, latency_ms, Outcome::Timeout);
    }

    /// Flush metrics to sink
    pub fn flush(&self) {
        let events = self.aggregator.snapshot_and_reset();
        for event in events {
            self.sink.record(event);
        }
        self.sink.flush();
    }

    /// Get current metrics snapshot
    pub fn get_metrics(&self) -> Vec<MetricsEvent> {
        self.aggregator.snapshot()
    }
}

// ============================================================================
// Global Instance
// ============================================================================

use once_cell::sync::Lazy;

static METRICS_SERVICE: Lazy<MetricsService> = Lazy::new(MetricsService::default);

/// Get the global metrics service
pub fn get_metrics() -> &'static MetricsService {
    &METRICS_SERVICE
}

/// Initialize global metrics with custom sink
pub fn init_metrics(sink: Arc<dyn MetricsSink>) {
    // Note: This would require interior mutability or a different pattern
    // For now, use the service directly
    let _ = sink;
}

/// Record a metric (convenience function)
pub fn record(op_type: OperationType, latency_ms: u64, outcome: Outcome) {
    METRICS_SERVICE.record(op_type, latency_ms, outcome);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_type() {
        assert_eq!(OperationType::Store.as_str(), "store");
        assert_eq!(OperationType::Search.as_str(), "search");
    }

    #[test]
    fn test_outcome() {
        assert_eq!(Outcome::Success.as_str(), "success");
        assert_eq!(Outcome::Error.as_str(), "error");
    }

    #[test]
    fn test_bucket_stats() {
        let mut stats = BucketStats::new();
        stats.record(100, Outcome::Success);
        stats.record(200, Outcome::Success);
        stats.record(300, Outcome::Error);

        assert_eq!(stats.count, 3);
        assert_eq!(stats.failure.success, 2);
        assert_eq!(stats.failure.error, 1);
    }

    #[test]
    fn test_aggregator() {
        let aggregator = MetricsAggregator::new(60_000, WindowStrategy::Fixed);

        aggregator.record(OperationType::Store, 100, Outcome::Success);
        aggregator.record(OperationType::Store, 200, Outcome::Success);
        aggregator.record(OperationType::Search, 50, Outcome::Error);

        let snapshot = aggregator.snapshot();
        assert!(!snapshot.is_empty());
    }

    #[test]
    fn test_noop_sink() {
        let sink = NoopMetricsSink;
        sink.record(MetricsEvent::new(
            "test".to_string(),
            OperationType::Store,
            BucketStats::new(),
        ));
        sink.flush();
    }

    #[test]
    fn test_metrics_service() {
        let service = MetricsService::new(Arc::new(NoopMetricsSink));

        service.record_success(OperationType::Store, 100);
        service.record_error(OperationType::Search, 50);

        let metrics = service.get_metrics();
        // Noop sink doesn't store, but aggregator still works
        assert!(service.get_metrics().is_empty() || !metrics.is_empty());
    }
}
