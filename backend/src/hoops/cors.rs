use salvo::http::Method;
use salvo::cors::{Cors, CorsHandler};

pub fn cors_hoop() -> CorsHandler {
    Cors::new()
        // 允许的前端地址（开发和生产环境）
        .allow_origin([
            "http://127.0.0.1:8000",
            "http://localhost:8000",
            "http://127.0.0.1:3000",
            "http://localhost:3000",
            "http://127.0.0.1:5173",
            "http://localhost:5173",
            // 可以根据需要添加生产环境地址
        ])
        // 允许的 HTTP 方法
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
            Method::PATCH,
        ])
        // 允许的请求头
        .allow_headers(vec![
            "authorization",
            "Content-Type",
            "Cookie",
            "X-Requested-With",
        ])
        // 允许发送 credentials（cookie）
        .allow_credentials(true)
        .into_handler()
}
