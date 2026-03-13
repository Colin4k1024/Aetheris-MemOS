//! Demo routes

use axum::{
    routing::get,
    Router,
};

/// Demo hello world handler
async fn hello() -> &'static str {
    "Hello World from salvo"
}

/// Create demo routes
pub fn router() -> Router {
    Router::new()
        .route("/", get(hello))
}
