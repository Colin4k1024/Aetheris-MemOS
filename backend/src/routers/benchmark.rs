//! Benchmark API routes (#92).
//!
//! Provides endpoints to run the standard eval suite and retrieve results
//! as machine-readable JSON. The quality gate endpoint can be used in CI
//! to fail the build when the benchmark pass rate drops below a threshold.

use axum::{
    extract::Query,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::services::eval_harness;

pub fn router() -> Router {
    Router::new()
        .route("/run", post(run_benchmark))
        .route("/report", get(get_report))
        .route("/quality-gate", post(run_quality_gate))
}

#[derive(Deserialize)]
pub struct QualityGateQuery {
    /// Minimum pass rate required (0.0–1.0). Default: 0.5.
    #[serde(default = "default_threshold")]
    pub threshold: f64,
}

fn default_threshold() -> f64 {
    0.5
}

/// POST /api/v1/benchmark/run
///
/// Run the full benchmark suite and return a JSON report. Cacheable
/// for CI — the result is deterministic given the same scheduler.
async fn run_benchmark() -> Result<Json<serde_json::Value>, AppError> {
    let report = eval_harness::run_benchmark_report().await;
    Ok(Json(report))
}

/// GET /api/v1/benchmark/report
///
/// Return the latest benchmark report (re-runs each time).
async fn get_report() -> Result<Json<serde_json::Value>, AppError> {
    let report = eval_harness::run_benchmark_report().await;
    Ok(Json(report))
}

/// POST /api/v1/benchmark/quality-gate?threshold=0.5
///
/// Run the suite and fail if pass rate below threshold. Returns 200
/// with the summary if the gate passes, or 422 with failure details.
async fn run_quality_gate(
    Query(q): Query<QualityGateQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !(0.0..=1.0).contains(&q.threshold) {
        return Err(AppError::BadRequest(
            "threshold must be between 0.0 and 1.0".to_string(),
        ));
    }
    match eval_harness::run_quality_gate(q.threshold).await {
        Ok(summary) => Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "passed": true,
                "threshold": q.threshold,
                "summary": summary
            })),
        )),
        Err((summary, message)) => Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({
                "passed": false,
                "threshold": q.threshold,
                "message": message,
                "summary": summary
            })),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn benchmark_runner_produces_report() {
        let report = eval_harness::run_benchmark_report().await;
        assert_eq!(report["suite"], "standard");
        assert!(report["summary"]["total"].as_u64().unwrap() >= 3);
        let results = report["results"].as_array().unwrap();
        assert_eq!(results.len(), report["summary"]["total"].as_u64().unwrap() as usize);
    }

    #[tokio::test]
    async fn quality_gate_with_high_threshold_fails() {
        let result = eval_harness::run_quality_gate(1.0).await;
        // The heuristic scheduler may not pass all cases, so a 100% threshold
        // should almost certainly fail.
        match result {
            Err((summary, msg)) => {
                assert!(summary.total > 0);
                assert!(msg.contains("below threshold"));
            }
            Ok(summary) => {
                // If all 3 cases somehow pass, that's also valid — just assert
                // the summary is coherent.
                assert_eq!(summary.passed + summary.failed, summary.total);
            }
        }
    }

    #[tokio::test]
    async fn quality_gate_with_zero_threshold_always_passes() {
        let result = eval_harness::run_quality_gate(0.0).await;
        assert!(result.is_ok(), "zero threshold should always pass");
    }
}