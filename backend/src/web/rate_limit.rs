//! Rate Limiting Middleware

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

/// Rate limiter configuration
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    pub max_requests: u32,
    pub window_seconds: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window_seconds: 60,
        }
    }
}

impl RateLimitConfig {
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            max_requests,
            window_seconds,
        }
    }
}

/// Rate limiter state
#[derive(Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    requests: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn check_and_record(&self, key: &str) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(self.config.window_seconds);

        let mut requests = self.requests.write().await;

        let timestamps = requests.entry(key.to_string()).or_insert_with(Vec::new);

        // Remove expired timestamps
        timestamps.retain(|&ts| now.duration_since(ts) < window);

        // Check if limit is exceeded
        if timestamps.len() >= self.config.max_requests as usize {
            return false;
        }

        // Record this request
        timestamps.push(now);
        true
    }

    pub async fn remaining(&self, key: &str) -> u32 {
        let now = Instant::now();
        let window = Duration::from_secs(self.config.window_seconds);

        let requests = self.requests.read().await;

        if let Some(timestamps) = requests.get(key) {
            let valid_count = timestamps
                .iter()
                .filter(|&&ts| now.duration_since(ts) < window)
                .count();
            self.config.max_requests.saturating_sub(valid_count as u32)
        } else {
            self.config.max_requests
        }
    }
}

/// Rate limit middleware function
pub async fn rate_limit_middleware(
    limiter: axum::extract::State<RateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    let key = extract_client_key(&request);

    if limiter.check_and_record(&key).await {
        next.run(request).await
    } else {
        let body = serde_json::json!({ "error": "Rate limit exceeded. Please try again later." });
        let mut res = axum::Json(body).into_response();
        *res.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        res
    }
}

fn extract_client_key(request: &Request) -> String {
    // Try API key first
    if let Some(api_key) = request.headers().get("X-API-Key") {
        if let Ok(key) = api_key.to_str() {
            if !key.is_empty() {
                return format!("api_key:{}", key);
            }
        }
    }

    // Try User ID
    if let Some(user_id) = request.headers().get("X-User-ID") {
        if let Ok(key) = user_id.to_str() {
            if !key.is_empty() {
                return format!("user:{}", key);
            }
        }
    }

    // Fallback to client IP
    if let Some(remote_addr) = request.headers().get("x-forwarded-for") {
        if let Ok(ip) = remote_addr.to_str() {
            return format!("ip:{}", ip.split(',').next().unwrap_or("unknown"));
        }
    }

    if let Some(remote_addr) = request.headers().get("x-real-ip") {
        if let Ok(ip) = remote_addr.to_str() {
            return format!("ip:{}", ip);
        }
    }

    "ip:unknown".to_string()
}
