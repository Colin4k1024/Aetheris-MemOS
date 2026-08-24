//! Axum Router Module
//!
//! This module provides Axum-based API routes to replace Salvo.

pub mod distributed;

use std::time::Instant;

use axum::{
    extract::{MatchedPath, Request as AxumRequest},
    middleware::{self, Next},
    response::Response,
    Router,
};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::{services::prometheus_exporter::get_exporter, web::cors_layer};

/// Create the main Axum router.
///
/// ## Public routes (no auth)
/// - `/api-doc/openapi.json` - OpenAPI spec
/// - `/scalar`, `/scalar/` - API docs UI
/// - `/login`, `/register` - Auth page handlers
/// - `/api/login` - Login API endpoint
/// - `/` - Demo hello
///
/// ## Protected routes (auth required via httpOnly cookie or Bearer header)
/// All other routes require a valid JWT. The auth middleware is applied
/// in `protected::protected_router()`.
///
/// ## Observability layers (outermost → innermost)
/// 1. Prometheus middleware — records route template, status, and duration
/// 2. CORS hoop
/// 3. `TraceLayer` — creates a per-request span with method/path/status/latency
pub fn create_router() -> Router {
    let cors = cors_layer();

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(
            DefaultMakeSpan::new()
                .level(Level::INFO)
                .include_headers(false),
        )
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    crate::routers::root()
        .layer(trace_layer)
        .layer(cors)
        .layer(middleware::from_fn(prometheus_metrics_middleware))
}

async fn prometheus_metrics_middleware(request: AxumRequest, next: Next) -> Response {
    let endpoint = request
        .extensions()
        .get::<MatchedPath>()
        .map(|matched_path| matched_path.as_str().to_owned())
        .unwrap_or_else(|| "unmatched".to_owned());
    let started_at = Instant::now();

    let response = next.run(request).await;
    get_exporter().record_request(&endpoint, response.status().as_u16(), started_at.elapsed());

    response
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use super::*;
    use crate::services::prometheus_exporter::get_exporter;

    #[tokio::test]
    async fn records_completed_requests_with_the_matched_path() {
        crate::config::init();

        let app = create_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/?request_id=unbounded-value")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::OK);

        let output = get_exporter().generate_prometheus_output();
        assert!(output.contains("memory_requests_total{endpoint=\"/\",status=\"200\"}"));
        // The exporter is a process-global singleton shared across the whole
        // test binary, so the duration count is not guaranteed to be exactly 1
        // here — only to have grown by at least the one request this test made.
        // Assert a positive count rather than the brittle exact-`1` form.
        let duration_count = output
            .lines()
            .find(|l| l.starts_with("memory_request_duration_seconds_count"))
            .and_then(|l| l.rsplit(' ').next())
            .and_then(|v| v.parse::<u64>().ok())
            .expect("memory_request_duration_seconds_count line present");
        assert!(duration_count >= 1, "duration count should be >= 1, got {duration_count}");
        assert!(!output.contains("request_id=unbounded-value"));
    }
}
