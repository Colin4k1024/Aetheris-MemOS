use axum::http::StatusCode;
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

        (
            status,
            Json(ErrorBody {
                code,
                message: message.clone(),
                error: message,
            }),
        )
            .into_response()
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
}
