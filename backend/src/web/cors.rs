//! CORS Middleware using tower-http

use axum::http::{HeaderName, HeaderValue, Method};
use tower_http::cors::CorsLayer;

/// Create CORS layer
pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://127.0.0.1:8000"),
            HeaderValue::from_static("http://localhost:8000"),
            HeaderValue::from_static("http://127.0.0.1:3000"),
            HeaderValue::from_static("http://localhost:3000"),
            HeaderValue::from_static("http://127.0.0.1:5173"),
            HeaderValue::from_static("http://localhost:5173"),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
            Method::PATCH,
        ])
        .allow_headers([
            HeaderName::from_static("authorization"),
            HeaderName::from_static("content-type"),
            HeaderName::from_static("cookie"),
            HeaderName::from_static("x-requested-with"),
        ])
}
