//! Metrics Router
//!
//! Anonymous aggregated metrics API endpoints.

use axum::Json;
use serde::Serialize;

use crate::services::metrics as metrics_service;
use crate::services::metrics::MetricsEvent;
use crate::{json_ok, JsonResult};

/// Metrics response
#[derive(Serialize)]
pub struct MetricsResponse {
    pub metrics: Vec<MetricsEvent>,
}

/// Get current metrics snapshot
pub async fn get_metrics() -> JsonResult<MetricsResponse> {
    let metrics = metrics_service::get_metrics().get_metrics();
    json_ok(MetricsResponse { metrics })
}
