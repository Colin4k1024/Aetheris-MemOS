use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
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
            _ => 1006,
        }
    }

    fn api_message(&self) -> String {
        match self {
            Self::Public(msg)
            | Self::Internal(msg)
            | Self::Unauthorized(msg)
            | Self::Forbidden(msg)
            | Self::DatabaseConnection(msg)
            | Self::DatabaseQuery(msg)
            | Self::DatabaseTransaction(msg)
            | Self::NotFound(msg)
            | Self::BadRequest(msg)
            | Self::Serialization(msg)
            | Self::Deserialization(msg) => msg.clone(),
            Self::Anyhow(e) => e.to_string(),
            Self::SqlxError(e) => e.to_string(),
            Self::Validation(e) => e.to_string(),
        }
    }
}
