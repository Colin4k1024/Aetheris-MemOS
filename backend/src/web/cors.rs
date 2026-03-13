//! CORS Middleware using tower-http

use tower_http::cors::{CorsLayer, Any};

/// Create CORS layer
pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_credentials(true)
}
