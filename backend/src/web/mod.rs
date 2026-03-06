//! Web Framework Layer (Axum)
//!
//! This module provides the web framework layer using Axum.

pub mod cors;
pub mod jwt;
pub mod rate_limit;

pub use cors::cors_layer;
pub use jwt::auth_middleware;
pub use rate_limit::rate_limit_middleware as rate_limit_layer;

use axum::{
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use hyper::http::StatusCode;

/// JSON result type
pub type JsonResult<T> = Result<Json<T>, AppError>;

/// Empty result type
pub type EmptyResult = Result<Empty, AppError>;

/// Empty response
#[derive(Debug, Clone, Copy)]
pub struct Empty;

impl IntoResponse for Empty {
    fn into_response(self) -> Response {
        ().into_response()
    }
}

/// JSON OK helper
pub fn json_ok<T: Serialize>(data: T) -> JsonResult<T> {
    Ok(Json(data))
}

/// Application error
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Anyhow(#[from] anyhow::Error),

    #[error("{0}")]
    Validation(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Unauthorized(String),

    #[error("{0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Anyhow(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Validation(e) => (StatusCode::BAD_REQUEST, e),
            AppError::NotFound(e) => (StatusCode::NOT_FOUND, e),
            AppError::Unauthorized(e) => (StatusCode::UNAUTHORIZED, e),
            AppError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
        };

        let body = serde_json::json!({ "error": message });
        (status, Json(body)).into_response()
    }
}
