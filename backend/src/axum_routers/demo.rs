//! Demo routes

use axum::{routing::get, Router};

/// Demo hello world handler
#[utoipa::path(get, path = "/", tag = "Demo")]
pub async fn hello() -> &'static str {
    "Hello World from axum"
}

/// Create demo routes
pub fn router() -> Router {
    Router::new().route("/", get(hello))
}
