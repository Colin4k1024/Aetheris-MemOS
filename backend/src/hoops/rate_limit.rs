//! Rate Limiting Middleware
//!
//! A simple in-memory rate limiter using the sliding window algorithm.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use salvo::prelude::*;

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

        // Get or create the request history for this key
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

/// Create a rate limiting hoop
pub fn rate_limit_hoop(max_requests: u32, window_seconds: u64) -> RateLimitHoop {
    RateLimitHoop {
        limiter: Arc::new(RateLimiter::new(RateLimitConfig::new(max_requests, window_seconds))),
    }
}

/// Rate limit middleware handler
#[derive(Clone)]
pub struct RateLimitHoop {
    limiter: Arc<RateLimiter>,
}

#[async_trait]
impl Handler for RateLimitHoop {
    async fn handle(&self, req: &mut Request, res: &mut Response, _ctrl: &mut FlowCtrl) {
        // Use client IP as the rate limit key
        // In production, you might want to use API key or user ID
        let client_ip = req
            .remote_addr()
            .map(|addr| addr.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        if self.limiter.check_and_record(&client_ip).await {
            // Request allowed - add rate limit headers
            let remaining = self.limiter.remaining(&client_ip).await;
            res.headers_mut()
                .insert("X-RateLimit-Limit", self.limiter.config.max_requests.to_string().parse().unwrap());
            res.headers_mut()
                .insert("X-RateLimit-Remaining", remaining.to_string().parse().unwrap());
        } else {
            // Rate limit exceeded
            res.status_code(StatusCode::TOO_MANY_REQUESTS);
            res.render(Text::Plain("Rate limit exceeded. Please try again later."));
        }
    }
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

        // First 5 requests should be allowed
        for _ in 0..5 {
            assert!(limiter.check_and_record("test_client").await);
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_excess() {
        let config = RateLimitConfig::new(3, 1);
        let limiter = RateLimiter::new(config);

        // First 3 requests should be allowed
        for _ in 0..3 {
            assert!(limiter.check_and_record("test_client2").await);
        }

        // 4th request should be blocked
        assert!(!limiter.check_and_record("test_client2").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_different_clients() {
        let config = RateLimitConfig::new(2, 1);
        let limiter = RateLimiter::new(config);

        // Different clients should have independent counts
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
