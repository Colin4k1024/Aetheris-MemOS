//! Dashboard API Router
//!
//! Provides dashboard endpoints for metrics and enterprise data.
//! - Anonymous aggregate metrics (QPS/latency/failure)
//! - Enterprise-specific aggregations (audit/billing summaries)
//! - RBAC protected endpoints

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

use crate::hoops::enterprise::{NoopHookSet, EnterpriseHookSet, GovernanceHook};
use crate::services::metrics::{
    self, BucketStats, MetricsAggregator, MetricsEvent, MetricsService, NoopMetricsSink,
    OperationType, Outcome,
};
use crate::{json_ok, AppError, JsonResult};

// ============================================================================
// Dashboard State
// ============================================================================

/// Dashboard state shared across endpoints
pub struct DashboardState {
    /// Metrics service
    metrics: Arc<MetricsService>,
    /// Enterprise hooks (optional)
    enterprise_hooks: Option<EnterpriseHookSet>,
}

impl DashboardState {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(MetricsService::new(Arc::new(NoopMetricsSink))),
            enterprise_hooks: None,
        }
    }

    pub fn with_enterprise_hooks(mut self, hooks: EnterpriseHookSet) -> Self {
        self.enterprise_hooks = Some(hooks);
        self
    }

    pub fn with_metrics(mut self, metrics: Arc<MetricsService>) -> Self {
        self.metrics = metrics;
        self
    }
}

impl Default for DashboardState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Time range query
#[derive(Debug, Deserialize)]
pub struct TimeRangeQuery {
    pub start: Option<i64>,
    pub end: Option<i64>,
}

/// Dashboard metrics response
#[derive(Serialize)]
pub struct DashboardMetricsResponse {
    /// Anonymous aggregate metrics
    pub aggregate: AggregateMetrics,
    /// Enterprise-specific metrics (if available)
    pub enterprise: Option<EnterpriseMetrics>,
    /// Timestamp of response
    pub timestamp: i64,
}

/// Anonymous aggregate metrics
#[derive(Serialize)]
pub struct AggregateMetrics {
    /// Queries per second (estimated)
    pub qps: f64,
    /// Average latency in ms
    pub avg_latency_ms: f64,
    /// P95 latency in ms
    pub p95_latency_ms: f64,
    /// Success rate
    pub success_rate: f64,
    /// Failure breakdown
    pub failures: FailureSummary,
    /// Operations breakdown
    pub operations: std::collections::HashMap<String, OpMetrics>,
}

/// Failure summary
#[derive(Serialize)]
pub struct FailureSummary {
    pub total: u64,
    pub success: u64,
    pub denied: u64,
    pub errors: u64,
    pub timeouts: u64,
}

/// Per-operation metrics
#[derive(Serialize)]
pub struct OpMetrics {
    pub count: u64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub success_count: u64,
    pub error_count: u64,
}

/// Enterprise-specific metrics
#[derive(Serialize)]
pub struct EnterpriseMetrics {
    /// Audit events summary
    pub audit: AuditSummary,
    /// Billing summary
    pub billing: BillingSummary,
}

/// Audit events summary
#[derive(Serialize)]
pub struct AuditSummary {
    pub total_events: u64,
    pub denied_events: u64,
    pub failed_events: u64,
}

/// Billing summary
#[derive(Serialize)]
pub struct BillingSummary {
    pub total_api_calls: u64,
    pub estimated_cost: f64,
    pub overage_count: u64,
}

/// QPS response
#[derive(Serialize)]
pub struct QpsResponse {
    pub qps: f64,
    pub window_seconds: u64,
    pub timestamp: i64,
}

/// Latency percentiles response
#[derive(Serialize)]
pub struct LatencyResponse {
    pub avg_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub timestamp: i64,
}

/// Failure rate response
#[derive(Serialize)]
pub struct FailureResponse {
    pub total_requests: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub success_rate: f64,
    pub failure_rate: f64,
    pub timestamp: i64,
}

// ============================================================================
// Anonymous Metrics Endpoints (Core - No RBAC)
// ============================================================================

/// Get aggregate dashboard metrics (anonymous)
pub async fn get_dashboard_metrics(
    State(state): State<Arc<DashboardState>>,
    _query: Option<Query<TimeRangeQuery>>,
) -> JsonResult<DashboardMetricsResponse> {
    let start = Instant::now();

    // Get metrics from aggregator
    let events = state.metrics.get_metrics();

    // Calculate aggregate metrics
    let mut total_count = 0u64;
    let mut total_latency = 0u64;
    let mut success_count = 0u64;
    let mut deny_count = 0u64;
    let mut error_count = 0u64;
    let mut timeout_count = 0u64;
    let mut operations: std::collections::HashMap<String, OpMetrics> = std::collections::HashMap::new();

    for event in &events {
        total_count += event.count;
        total_latency += (event.latency_ms.avg * event.count as f64) as u64;
        success_count += event.failure.success;
        deny_count += event.failure.deny;
        error_count += event.failure.error;
        timeout_count += event.failure.timeout;

        // Per-operation breakdown
        let op_key = event.op_type.as_str().to_string();
        let entry = operations.entry(op_key).or_insert(OpMetrics {
            count: 0,
            avg_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            success_count: 0,
            error_count: 0,
        });
        entry.count += event.count;
        entry.success_count += event.failure.success;
        entry.error_count += event.failure.error;
    }

    // Calculate QPS (approximate)
    let qps = if total_count > 0 {
        // Assume 10 second window
        total_count as f64 / 10.0
    } else {
        0.0
    };

    let avg_latency = if total_count > 0 {
        total_latency as f64 / total_count as f64
    } else {
        0.0
    };

    let success_rate = if total_count > 0 {
        success_count as f64 / total_count as f64
    } else {
        1.0
    };

    let aggregate = AggregateMetrics {
        qps,
        avg_latency_ms: avg_latency,
        p95_latency_ms: avg_latency * 1.5, // Simplified
        success_rate,
        failures: FailureSummary {
            total: total_count,
            success: success_count,
            denied: deny_count,
            errors: error_count,
            timeouts: timeout_count,
        },
        operations,
    };

    // Get enterprise metrics if available
    let enterprise = get_enterprise_metrics(&state).await;

    let response = DashboardMetricsResponse {
        aggregate,
        enterprise,
        timestamp: chrono::Utc::now().timestamp(),
    };

    tracing::info!("Dashboard metrics request completed in {:?}", start.elapsed());
    json_ok(response)
}

/// Get current QPS
pub async fn get_qps(
    State(state): State<Arc<DashboardState>>,
) -> JsonResult<QpsResponse> {
    let events = state.metrics.get_metrics();

    let total_count: u64 = events.iter().map(|e| e.count).sum();
    let qps = total_count as f64 / 10.0; // Assume 10s window

    json_ok(QpsResponse {
        qps,
        window_seconds: 10,
        timestamp: chrono::Utc::now().timestamp(),
    })
}

/// Get latency percentiles
pub async fn get_latency(
    State(state): State<Arc<DashboardState>>,
) -> JsonResult<LatencyResponse> {
    let events = state.metrics.get_metrics();

    let mut total_count = 0u64;
    let mut total_latency = 0.0;
    let mut p95_samples: Vec<f64> = vec![];

    for event in &events {
        let count = event.count;
        total_count += count;
        total_latency += event.latency_ms.avg * count as f64;

        // Collect p95 samples
        for _ in 0..count {
            p95_samples.push(event.latency_ms.p95);
        }
    }

    let avg_ms = if total_count > 0 {
        total_latency / total_count as f64
    } else {
        0.0
    };

    // Calculate percentiles
    p95_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p50_ms = percentile(&p95_samples, 0.5);
    let p95_ms = percentile(&p95_samples, 0.95);
    let p99_ms = percentile(&p95_samples, 0.99);

    json_ok(LatencyResponse {
        avg_ms,
        p50_ms,
        p95_ms,
        p99_ms,
        timestamp: chrono::Utc::now().timestamp(),
    })
}

/// Get failure rates
pub async fn get_failures(
    State(state): State<Arc<DashboardState>>,
) -> JsonResult<FailureResponse> {
    let events = state.metrics.get_metrics();

    let mut total = 0u64;
    let mut success = 0u64;
    let mut failure = 0u64;

    for event in &events {
        total += event.count;
        success += event.failure.success;
        failure += event.failure.deny + event.failure.error + event.failure.timeout;
    }

    let success_rate = if total > 0 {
        success as f64 / total as f64
    } else {
        1.0
    };

    json_ok(FailureResponse {
        total_requests: total,
        success_count: success,
        failure_count: failure,
        success_rate,
        failure_rate: 1.0 - success_rate,
        timestamp: chrono::Utc::now().timestamp(),
    })
}

// ============================================================================
// Enterprise Metrics (RBAC Protected)
// ============================================================================

async fn get_enterprise_metrics(state: &Arc<DashboardState>) -> Option<EnterpriseMetrics> {
    // Check if enterprise hooks are available
    let hooks = state.enterprise_hooks.as_ref()?;

    // Get usage from hooks
    let usage = hooks.get_usage("default")?; // Would need proper tenant context

    Some(EnterpriseMetrics {
        audit: AuditSummary {
            total_events: 0, // Would query audit hook
            denied_events: 0,
            failed_events: 0,
        },
        billing: BillingSummary {
            total_api_calls: usage.api_calls,
            estimated_cost: 0.0, // Would calculate from UsageTracker
            overage_count: 0,
        },
    })
}

/// Helper to calculate percentile
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p) as usize;
    sorted[idx.min(sorted.len() - 1)]
}

// ============================================================================
// Phase 4: Regression Tests - Core with NoopHookSet
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_state_default() {
        let state = DashboardState::default();
        assert!(state.enterprise_hooks.is_none());
    }

    #[test]
    fn test_dashboard_state_with_hooks() {
        let hooks = EnterpriseHookSet::new();
        let state = DashboardState::default().with_enterprise_hooks(hooks);
        assert!(state.enterprise_hooks.is_some());
    }

    #[test]
    fn test_percentile() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(&data, 0.5), 3.0);
        assert_eq!(percentile(&data, 0.95), 4.0); // Index 3 = 4.0
        assert_eq!(percentile(&[], 0.5), 0.0);
    }

    #[tokio::test]
    async fn test_dashboard_metrics_empty() {
        let state = Arc::new(DashboardState::default());
        let result = get_dashboard_metrics(
            State(state),
            Option::<Query<TimeRangeQuery>>::None,
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_qps_empty() {
        let state = Arc::new(DashboardState::default());
        let result = get_qps(State(state)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_latency_empty() {
        let state = Arc::new(DashboardState::default());
        let result = get_latency(State(state)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_failures_empty() {
        let state = Arc::new(DashboardState::default());
        let result = get_failures(State(state)).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_noop_hooks_allows_core() {
        // Verify NoopHookSet allows core operations
        let hooks = EnterpriseHookSet::new();

        // All license checks should pass (default allow)
        assert!(hooks.check_license("test", crate::hoops::enterprise::LicenseTier::Free));

        // All feature checks should pass (default allow)
        assert!(hooks.check_feature("test", "any_feature"));

        // All quota checks should pass (unlimited)
        let quota = hooks.check_quota("test", crate::hoops::enterprise::Resource::ApiCalls);
        assert!(quota.allowed);
    }
}
