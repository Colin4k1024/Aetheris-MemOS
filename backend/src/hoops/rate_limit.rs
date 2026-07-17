//! Rate Limiting Middleware
//!
//! A simple in-memory rate limiter using the sliding window algorithm.

use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Rate limiter configuration
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// Maximum requests allowed per window
    pub max_requests: u32,
    /// Time window duration in seconds
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

/// Hard cap on distinct rate-limit keys tracked at once. The key can come from a
/// client-controlled header, so an attacker could otherwise rotate it to grow the
/// map without bound; once we exceed this we drop keys whose hits have all expired.
const MAX_TRACKED_KEYS: usize = 100_000;

/// Rate limiter state
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

    /// Check if request is allowed and record it
    pub async fn check_and_record(&self, key: &str) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(self.config.window_seconds);

        let mut requests = self.requests.write().await;

        // Bound memory: if the key space has blown up (e.g. a spoofed/rotated header
        // key), evict every key whose hits have all aged out of the window so the map
        // cannot grow without limit.
        if requests.len() > MAX_TRACKED_KEYS {
            requests.retain(|_, timestamps| {
                timestamps.retain(|&ts| now.duration_since(ts) < window);
                !timestamps.is_empty()
            });
        }

        let timestamps = requests.entry(key.to_string()).or_insert_with(Vec::new);
        timestamps.retain(|&ts| now.duration_since(ts) < window);

        if timestamps.len() >= self.config.max_requests as usize {
            return false;
        }

        timestamps.push(now);
        true
    }

    /// Get remaining requests for a key
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

pub fn rate_limit_state(max_requests: u32, window_seconds: u64) -> Arc<RateLimiter> {
    Arc::new(RateLimiter::new(RateLimitConfig::new(
        max_requests,
        window_seconds,
    )))
}

/// Rate limit middleware.
///
/// Client identity is derived from the trusted TCP connection (`ConnectInfo`) when
/// available, falling back to `X-Forwarded-For` only for trusted proxy deployments.
/// This prevents rate-limit bypass via XFF header spoofing.
pub async fn rate_limit_middleware(
    State(limiter): State<Arc<RateLimiter>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Prefer the peer address from the trusted TCP connection. ConnectInfo is
    // only populated when the router is started with
    // `into_make_service_with_connect_info::<SocketAddr>()`.
    let client_ip = req
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| {
            // XFF fallback: only trusted when behind a reverse proxy that
            // sanitizes this header. Client-controllable otherwise.
            req.headers()
                .get("x-forwarded-for")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.split(',').next())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        });

    if !limiter.check_and_record(&client_ip).await {
        return Ok((
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "code": 1005,
                "message": "Rate limit exceeded. Please try again later.",
                "error": "Rate limit exceeded. Please try again later."
            })),
        )
            .into_response());
    }

    let remaining = limiter.remaining(&client_ip).await;
    let mut response = next.run(req).await;
    response.headers_mut().insert(
        "X-RateLimit-Limit",
        HeaderValue::from_str(&limiter.config.max_requests.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    response.headers_mut().insert(
        "X-RateLimit-Remaining",
        HeaderValue::from_str(&remaining.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_creation() {
        let config = RateLimitConfig::new(100, 60);
        let limiter = RateLimiter::new(config);
        assert_eq!(limiter.config.max_requests, 100);
        assert_eq!(limiter.config.window_seconds, 60);
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_requests() {
        let config = RateLimitConfig::new(5, 1);
        let limiter = RateLimiter::new(config);

        for _ in 0..5 {
            assert!(limiter.check_and_record("test_client").await);
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_excess() {
        let config = RateLimitConfig::new(3, 1);
        let limiter = RateLimiter::new(config);

        for _ in 0..3 {
            assert!(limiter.check_and_record("test_client2").await);
        }

        assert!(!limiter.check_and_record("test_client2").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_different_clients() {
        let config = RateLimitConfig::new(2, 1);
        let limiter = RateLimiter::new(config);

        assert!(limiter.check_and_record("client_a").await);
        assert!(limiter.check_and_record("client_b").await);
        assert!(limiter.check_and_record("client_a").await);
        assert!(limiter.check_and_record("client_b").await);
        assert!(!limiter.check_and_record("client_a").await);
        assert!(!limiter.check_and_record("client_b").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_remaining() {
        let config = RateLimitConfig::new(3, 1);
        let limiter = RateLimiter::new(config);

        assert_eq!(limiter.remaining("test").await, 3);
        limiter.check_and_record("test").await;
        assert_eq!(limiter.remaining("test").await, 2);
        limiter.check_and_record("test").await;
        assert_eq!(limiter.remaining("test").await, 1);
    }
}
