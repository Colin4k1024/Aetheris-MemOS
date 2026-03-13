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
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) | Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::DatabaseConnection(_) | Self::DatabaseQuery(_) | Self::DatabaseTransaction(_) => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

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
                error: self.to_string(),
            }),
        )
            .into_response()
    }
}
