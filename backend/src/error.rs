use axum::http::{header::RETRY_AFTER, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("public: `{0}`")]
    Public(String),
    #[error("internal: `{0}`")]
    Internal(String),
    #[error("unauthorized: `{0}`")]
    Unauthorized(String),
    #[error("forbidden: `{0}`")]
    Forbidden(String),
    #[error("anyhow error:`{0}`")]
    Anyhow(#[from] anyhow::Error),
    #[error("sqlx::Error:`{0}`")]
    SqlxError(#[from] sqlx::Error),
    #[error("validation error:`{0}`")]
    Validation(#[from] validator::ValidationErrors),
    #[error("database connection error: `{0}`")]
    DatabaseConnection(String),
    #[error("database query error: `{0}`")]
    DatabaseQuery(String),
    #[error("database transaction error: `{0}`")]
    DatabaseTransaction(String),
    #[error("not found: `{0}`")]
    NotFound(String),
    #[error("bad request: `{0}`")]
    BadRequest(String),
    #[error("serialization error: `{0}`")]
    Serialization(String),
    #[error("deserialization error: `{0}`")]
    Deserialization(String),
    /// A required downstream dependency (embedding backend, etc.) is unavailable
    /// — maps to 503 (retryable). The `String` carries detail for LOGS ONLY;
    /// `api_message` returns a generic message so an internal endpoint is never
    /// leaked to the client. Kept dependency-agnostic on purpose (Qdrant, Neo4j,
    /// audit DB can reuse it — name the dependency inside the `String`).
    #[error("dependency unavailable: `{0}`")]
    DependencyUnavailable(String),
}
impl AppError {
    pub fn public<S: Into<String>>(msg: S) -> Self {
        Self::Public(msg.into())
    }

    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self::Internal(msg.into())
    }
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: i32,
    message: String,
    // Backward-compatible alias for existing clients.
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.api_code();
        let message = self.api_message();

        match &self {
            Self::Internal(msg) => tracing::error!(msg = msg, "internal error"),
            Self::DatabaseConnection(msg)
            | Self::DatabaseQuery(msg)
            | Self::DatabaseTransaction(msg) => tracing::error!(msg = msg, "database error"),
            Self::Validation(e) => tracing::warn!(error = ?e, "validation error"),
            Self::NotFound(msg) => tracing::warn!(msg = msg, "resource not found"),
            Self::BadRequest(msg) => tracing::warn!(msg = msg, "bad request"),
            Self::DependencyUnavailable(msg) => {
                tracing::warn!(msg = msg, "dependency unavailable")
            }
            _ => {}
        }

        let mut response = (
            status,
            Json(ErrorBody {
                code,
                message: message.clone(),
                error: message,
            }),
        )
            .into_response();

        // 503 means "retry later", but a bare 503 leaves the caller guessing the
        // interval — so N clients hammer a still-recovering dependency in a tight
        // loop (a self-inflicted load spike). Attach a conservative Retry-After
        // here at the single response-construction point, so every 503 variant is
        // covered without per-variant duplication. Non-503 responses are left
        // untouched: a Retry-After on a permanent 4xx/500 would wrongly invite a
        // retry. A static integer (no server-side jitter) keeps the response
        // cacheable and deterministically testable — spreading retries is the
        // client's job, not ours.
        if status == StatusCode::SERVICE_UNAVAILABLE {
            if let Some(secs) = self.retry_after_seconds() {
                response
                    .headers_mut()
                    .insert(RETRY_AFTER, HeaderValue::from(secs));
            }
        }

        response
    }
}

impl AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) | Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::DatabaseConnection(_) | Self::DatabaseQuery(_) | Self::DatabaseTransaction(_) => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            Self::DependencyUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Suggested `Retry-After` value (in seconds) for the retryable (503)
    /// variants. Per-variant on purpose: a DB pool hiccup usually clears in a
    /// few seconds, whereas an external dependency (embedding backend) may be
    /// restarting or loading a model and needs a longer breather. Values are
    /// deliberately non-tiny so a fleet of clients does not re-storm a
    /// still-recovering dependency 1s later. Non-503 variants return `None` —
    /// they are not retryable, so they must not carry a `Retry-After`.
    fn retry_after_seconds(&self) -> Option<u16> {
        match self {
            // Transient DB layer failures — pool/connection recovery is usually quick.
            Self::DatabaseConnection(_) | Self::DatabaseQuery(_) | Self::DatabaseTransaction(_) => {
                Some(5)
            }
            // External dependency (e.g. embedding backend) — cold start / model
            // load can take noticeably longer, so back off further.
            Self::DependencyUnavailable(_) => Some(10),
            _ => None,
        }
    }

    fn api_code(&self) -> i32 {
        match self {
            Self::BadRequest(_) | Self::Validation(_) => 1001,
            Self::Unauthorized(_) => 1002,
            Self::Forbidden(_) => 1003,
            Self::NotFound(_) => 1004,
            Self::DatabaseConnection(_) | Self::DatabaseQuery(_) | Self::DatabaseTransaction(_) => {
                1007
            }
            Self::DependencyUnavailable(_) => 1008,
            _ => 1006,
        }
    }

    fn api_message(&self) -> String {
        match self {
            Self::Public(msg)
            | Self::Unauthorized(msg)
            | Self::Forbidden(msg)
            | Self::NotFound(msg)
            | Self::BadRequest(msg)
            | Self::Serialization(msg)
            | Self::Deserialization(msg) => msg.clone(),
            Self::Validation(e) => e.to_string(),
            // Generic, endpoint-free message. The variant's `String` (which may
            // contain an internal URL) is for logs only — never returned here.
            Self::DependencyUnavailable(_) => {
                "A required dependency is temporarily unavailable, please retry".to_string()
            }
            Self::Internal(_)
            | Self::DatabaseConnection(_)
            | Self::DatabaseQuery(_)
            | Self::DatabaseTransaction(_)
            | Self::Anyhow(_)
            | Self::SqlxError(_) => "An internal error occurred".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_unavailable_maps_to_503() {
        let e = AppError::DependencyUnavailable("embedding backend down".to_string());
        assert_eq!(e.status_code(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(e.api_code(), 1008);
    }

    #[test]
    fn dependency_unavailable_message_does_not_leak_endpoint() {
        // The variant String carries an internal endpoint for logs; the
        // client-facing api_message must NOT contain it (repo security rule:
        // never expose internal paths/endpoints in error responses).
        let e = AppError::DependencyUnavailable(
            "embedding backend unreachable at http://internal-host:11434/api/embeddings"
                .to_string(),
        );
        let msg = e.api_message();
        assert!(!msg.contains("http"), "api_message leaked a URL: {msg}");
        assert!(
            !msg.contains("internal-host"),
            "api_message leaked a host: {msg}"
        );
        assert!(!msg.contains("11434"), "api_message leaked a port: {msg}");
    }

    /// Parse the `Retry-After` header off a response into whole seconds,
    /// asserting it is ASCII and integer when present.
    fn retry_after_secs(resp: &Response) -> Option<u64> {
        resp.headers().get(RETRY_AFTER).map(|v| {
            v.to_str()
                .expect("Retry-After must be ASCII")
                .parse::<u64>()
                .expect("Retry-After must be an integer number of seconds")
        })
    }

    #[test]
    fn all_503_variants_carry_positive_retry_after() {
        // Every variant that maps to 503 must emit a parseable, positive
        // Retry-After on the *actual response* — not just have code that says so.
        let cases: Vec<AppError> = vec![
            AppError::DatabaseConnection("pool exhausted".into()),
            AppError::DatabaseQuery("statement timeout".into()),
            AppError::DatabaseTransaction("deadlock".into()),
            AppError::DependencyUnavailable("embedding backend down".into()),
        ];
        for e in cases {
            // Sanity: the variant really is a 503 before we assert the header.
            assert_eq!(e.status_code(), StatusCode::SERVICE_UNAVAILABLE);
            let resp = e.into_response();
            assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
            let secs = retry_after_secs(&resp).expect("503 must carry Retry-After");
            assert!(secs > 0, "Retry-After must be a positive number of seconds");
        }
    }

    #[test]
    fn non_503_responses_have_no_retry_after() {
        // A Retry-After on a permanent 4xx/500 would wrongly invite the caller to
        // retry — so these must NOT carry the header.
        let cases: Vec<AppError> = vec![
            AppError::BadRequest("bad".into()),   // 400
            AppError::Unauthorized("no".into()),  // 401
            AppError::Forbidden("nope".into()),   // 403
            AppError::NotFound("missing".into()), // 404
            AppError::Internal("boom".into()),    // 500
        ];
        for e in cases {
            let status = e.status_code();
            assert_ne!(status, StatusCode::SERVICE_UNAVAILABLE);
            let resp = e.into_response();
            assert!(
                retry_after_secs(&resp).is_none(),
                "non-503 ({status}) must NOT carry Retry-After"
            );
        }
    }

    #[test]
    fn retry_after_seconds_are_per_variant_and_conservative() {
        // Documents intent and guards drift: DB layer backs off less than an
        // external dependency, and non-retryable variants suggest nothing.
        assert_eq!(
            AppError::DatabaseConnection("x".into()).retry_after_seconds(),
            Some(5)
        );
        assert_eq!(
            AppError::DatabaseQuery("x".into()).retry_after_seconds(),
            Some(5)
        );
        assert_eq!(
            AppError::DatabaseTransaction("x".into()).retry_after_seconds(),
            Some(5)
        );
        assert_eq!(
            AppError::DependencyUnavailable("x".into()).retry_after_seconds(),
            Some(10)
        );
        assert_eq!(AppError::BadRequest("x".into()).retry_after_seconds(), None);
    }

    #[test]
    fn content_type_stays_json_after_adding_header() {
        // Regression guard for the shared response path: attaching Retry-After
        // must not disturb the Content-Type of any error response.
        use axum::http::header::CONTENT_TYPE;
        for e in [
            AppError::DependencyUnavailable("x".into()),
            AppError::BadRequest("x".into()),
        ] {
            let resp = e.into_response();
            let ct = resp
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default();
            assert!(
                ct.starts_with("application/json"),
                "error body must stay JSON, got {ct:?}"
            );
        }
    }

    #[tokio::test]
    async fn error_body_shape_unchanged_after_adding_header() {
        // Strongest guard that changing the tail expression of `into_response`
        // did not alter the JSON envelope (code/message/error) every variant
        // returns. Uses a 503 variant so both the body and the new header path
        // are exercised together.
        let e = AppError::DependencyUnavailable("embedding down".into());
        let expected_code = e.api_code();
        let resp = e.into_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("body must be readable");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("body must be JSON");
        assert_eq!(v["code"], expected_code);
        assert!(v["message"].is_string());
        // Backward-compatible alias must still mirror `message`.
        assert_eq!(v["message"], v["error"]);
    }
}
