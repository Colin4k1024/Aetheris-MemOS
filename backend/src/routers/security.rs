//! Security Router - Prompt Injection Probe API
//!
//! Provides endpoints for checking text against the prompt injection probe network.

use axum::Json;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::AppError;
use crate::services::embedding::EmbeddingService;
use crate::services::prompt_injection_probe::{ProbeResult, PromptInjectionProbe};
use crate::{json_ok, JsonResult};

/// Global prompt injection probe instance.
/// If EmbeddingService fails to init (missing Ollama/config), probe is unavailable.
static PROBE: Lazy<Option<Arc<PromptInjectionProbe>>> = Lazy::new(|| {
    match EmbeddingService::new() {
        Ok(embedding) => Some(Arc::new(PromptInjectionProbe::new(Arc::new(embedding)))),
        Err(e) => {
            tracing::warn!("Prompt injection probe unavailable: {}", e);
            None
        }
    }
});

fn get_probe() -> Result<&'static PromptInjectionProbe, AppError> {
    PROBE
        .as_deref()
        .ok_or_else(|| AppError::Internal("prompt injection probe is not available".into()))
}

/// Request body for prompt probe check
#[derive(Debug, Deserialize)]
pub struct ProbeCheckRequest {
    /// The text to check for prompt injection
    pub text: String,
}

/// Response body for prompt probe check
#[derive(Debug, Serialize)]
pub struct ProbeCheckResponse {
    /// The result of the probe check
    pub result: ProbeResult,
}

/// POST /api/v1/security/prompt-probe/check
///
/// Check text against the prompt injection probe network.
/// Runs all 3 layers: keyword blocklist, embedding similarity, and tool invocation patterns.
pub async fn check_prompt_probe(
    Json(request): Json<ProbeCheckRequest>,
) -> JsonResult<ProbeCheckResponse> {
    let probe = get_probe()?;
    let result = probe.check(&request.text).await;
    json_ok(ProbeCheckResponse { result })
}

/// POST /api/v1/security/prompt-probe/check-input
///
/// Specifically check input text (optimized for user prompts).
pub async fn check_prompt_probe_input(
    Json(request): Json<ProbeCheckRequest>,
) -> JsonResult<ProbeCheckResponse> {
    let probe = get_probe()?;
    let result = probe.check_input(&request.text).await;
    json_ok(ProbeCheckResponse { result })
}

/// POST /api/v1/security/prompt-probe/check-output
///
/// Specifically check LLM output text (includes tool invocation pattern detection).
pub async fn check_prompt_probe_output(
    Json(request): Json<ProbeCheckRequest>,
) -> JsonResult<ProbeCheckResponse> {
    let probe = get_probe()?;
    let result = probe.check_output(&request.text).await;
    json_ok(ProbeCheckResponse { result })
}
