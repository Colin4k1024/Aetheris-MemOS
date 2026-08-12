//! HTTP request metrics middleware.
//!
//! Wires the two per-request metrics that were registered but never written
//! (so they read a frozen 0 in `/metrics`): `memory_requests_total{endpoint,status}`
//! and `memory_request_duration_seconds`. Both are emitted by
//! [`PrometheusExporter::record_request`], which this middleware calls once per
//! request.

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;

use crate::services::prometheus_exporter::get_exporter;

/// Endpoints that are monitoring plumbing, not application traffic.
///
/// Recording them is actively harmful: `/metrics` would grow a self-referential
/// series that increments on every scrape, and probe/scrape frequency (not user
/// behaviour) would dominate `memory_requests_total`. They are still served
/// normally — only the metric write is skipped.
fn is_monitoring_endpoint(template: &str) -> bool {
    matches!(template, "/metrics" | "/livez" | "/readyz")
}

/// Record request count (by route template + status) and request latency.
///
/// The `endpoint` label is the **route template** (`MatchedPath`), never the
/// concrete URI. A concrete path embeds unbounded values — tenant ids, entry
/// ids, session ids — and Prometheus creates one time series per distinct label
/// value, so labelling by raw path is the classic high-cardinality outage that
/// takes the monitoring backend down. `MatchedPath` is the pattern axum resolved
/// during routing (e.g. `/api/tenants/{tenant_id}/sessions`), which is a bounded
/// set.
pub async fn track_request_metrics(req: Request, next: Next) -> Response {
    let endpoint = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_owned())
        // No route matched (404 / fallback). Bucket every miss under one sentinel
        // rather than the raw path: the path of an unmatched request is
        // attacker-controlled and unbounded, so echoing it into a label is the
        // same cardinality hazard the template guards against.
        .unwrap_or_else(|| "unmatched".to_owned());

    if is_monitoring_endpoint(&endpoint) {
        return next.run(req).await;
    }

    let start = Instant::now();
    let response = next.run(req).await;
    let status = response.status().as_u16();
    get_exporter().record_request(&endpoint, status, start.elapsed());
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt; // for `oneshot`

    async fn ok_handler() -> &'static str {
        "ok"
    }

    /// Value of the `memory_requests_total` series for a given `endpoint` label
    /// (summed across status codes), or 0.0 if that label is absent.
    fn requests_for_endpoint(reg: &prometheus::Registry, endpoint: &str) -> f64 {
        let mut total = 0.0;
        for mf in reg.gather() {
            if mf.get_name() != "memory_requests_total" {
                continue;
            }
            for m in mf.get_metric() {
                if m.get_label()
                    .iter()
                    .any(|l| l.get_name() == "endpoint" && l.get_value() == endpoint)
                {
                    total += m.get_counter().get_value();
                }
            }
        }
        total
    }

    // These tests run against the global `get_exporter()` singleton (the
    // middleware has no other exporter to reach). Assertions therefore target
    // endpoint labels unique to each test, so other tests writing the shared
    // registry cannot perturb them; deltas (`before + 1.0`) absorb any repeat
    // runs.

    #[tokio::test]
    async fn records_route_template_never_the_concrete_path() {
        // `.nest(..)` + a path parameter mirrors the real router, where the
        // metric must survive both the mount prefix and an unbounded id segment.
        let app = Router::new()
            .nest(
                "/mwtest",
                Router::new().route("/sessions/{tenant_id}", get(ok_handler)),
            )
            .layer(axum::middleware::from_fn(track_request_metrics));

        let template = "/mwtest/sessions/{tenant_id}";
        let before = requests_for_endpoint(get_exporter().registry(), template);

        let res = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/mwtest/sessions/tenant-abc")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), 200);

        // The concrete id must NOT become its own series — that is the outage.
        assert_eq!(
            requests_for_endpoint(get_exporter().registry(), "/mwtest/sessions/tenant-abc"),
            0.0,
            "concrete path leaked into the endpoint label (cardinality explosion)"
        );
        // The template series must have moved — proves the metric has a value,
        // not merely that a series exists.
        assert_eq!(
            requests_for_endpoint(get_exporter().registry(), template),
            before + 1.0,
            "request was not counted under its route template"
        );
    }

    #[tokio::test]
    async fn does_not_record_the_metrics_scrape_endpoint() {
        let app = Router::new()
            .route("/metrics", get(ok_handler))
            .layer(axum::middleware::from_fn(track_request_metrics));

        let before = requests_for_endpoint(get_exporter().registry(), "/metrics");
        let _ = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            requests_for_endpoint(get_exporter().registry(), "/metrics"),
            before,
            "scrape endpoint must not inflate memory_requests_total"
        );
    }
}
