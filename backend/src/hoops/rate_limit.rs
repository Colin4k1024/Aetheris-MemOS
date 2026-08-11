//! Rate Limiting Middleware
//!
//! A simple in-memory rate limiter using the sliding window algorithm.

use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
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
    trusted_proxies: Vec<IpAddr>,
    requests: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig, trusted_proxies: Vec<IpAddr>) -> Self {
        Self {
            config,
            trusted_proxies,
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

pub fn rate_limit_state(
    max_requests: u32,
    window_seconds: u64,
    trusted_proxies: Vec<IpAddr>,
) -> Arc<RateLimiter> {
    Arc::new(RateLimiter::new(
        RateLimitConfig::new(max_requests, window_seconds),
        trusted_proxies,
    ))
}

/// Determine the client key for rate limiting from the peer address and
/// optional `X-Forwarded-For` header.
///
/// # Trusted-proxy semantics
///
/// * **Empty allowlist**: the XFF header is **never** trusted. Always use the
///   direct peer IP from the TCP connection.
/// * **Untrusted peer**: if the peer IP is NOT in the allowlist, the request
///   came directly from a non-proxy — use the peer IP and ignore XFF.
/// * **Trusted peer**: the peer IS a trusted proxy. Parse XFF and walk the
///   entries from **rightmost to leftmost**, skipping any entry that is itself
///   a trusted proxy, and use the first non-trusted address found. If none is
///   found, fall back to the peer IP.
///
///   XFF is appended left-to-right: the leftmost value is supplied by the
///   client (forgeable), and each subsequent proxy appends what it observed.
///   The rightmost non-proxy entry is the original client address as seen by
///   the first trusted proxy. **Do not** naively take the leftmost entry —
///   that reintroduces the spoofing vulnerability.
///
/// * **Missing peer**: returns a sentinel constant. This branch should be
///   unreachable when `ConnectInfo` is installed.
pub fn client_key(
    peer_ip: Option<SocketAddr>,
    xff: Option<&str>,
    trusted_proxies: &[IpAddr],
) -> String {
    let peer = match peer_ip {
        Some(addr) => addr,
        None => {
            tracing::error!(
                "rate_limit: ConnectInfo missing — middleware misconfigured; using sentinel key"
            );
            return "misconfigured-no-connect-info".to_string();
        }
    };

    let peer_ip_addr = peer.ip();

    // Empty allowlist → never trust XFF.
    if trusted_proxies.is_empty() {
        return peer_ip_addr.to_string();
    }

    // Peer is not a trusted proxy → use direct peer IP, ignore XFF.
    if !trusted_proxies.contains(&peer_ip_addr) {
        return peer_ip_addr.to_string();
    }

    // Peer is a trusted proxy → consult XFF, walking right-to-left.
    let xff_str = match xff {
        Some(s) => s,
        None => return peer_ip_addr.to_string(),
    };

    for entry in xff_str.split(',').rev() {
        let ip_str = entry.trim();
        if ip_str.is_empty() {
            continue;
        }
        match ip_str.parse::<IpAddr>() {
            Ok(ip) => {
                if !trusted_proxies.contains(&ip) {
                    return ip.to_string();
                }
                // Trusted proxy entry — skip and continue leftwards.
            }
            Err(_) => {
                // An unparseable entry must NEVER become the bucket key. XFF is
                // appended left-to-right, so entries to the left of the trusted
                // proxy's own append are client-supplied — returning the raw text
                // would let an attacker mint a fresh bucket per request with a
                // random string, which is the very bypass this function exists to
                // close. Fall back to the peer IP, which cannot be forged.
                tracing::warn!(
                    entry = %ip_str,
                    "rate_limit: unparseable XFF entry — falling back to peer IP"
                );
                return peer_ip_addr.to_string();
            }
        }
    }

    // All XFF entries are trusted proxies — fall back to peer IP.
    peer_ip_addr.to_string()
}

/// Rate limit middleware.
///
/// Client identity is derived from the trusted TCP connection (`ConnectInfo`)
/// when available, falling back to `X-Forwarded-For` only for trusted proxy
/// deployments. This prevents rate-limit bypass via XFF header spoofing.
pub async fn rate_limit_middleware(
    State(limiter): State<Arc<RateLimiter>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let peer = req
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0);
    let xff = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok());

    let client_ip = client_key(peer, xff, &limiter.trusted_proxies);

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
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    // --- RateLimiter tests ---

    #[tokio::test]
    async fn test_rate_limiter_creation() {
        let config = RateLimitConfig::new(100, 60);
        let limiter = RateLimiter::new(config, vec![]);
        assert_eq!(limiter.config.max_requests, 100);
        assert_eq!(limiter.config.window_seconds, 60);
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_requests() {
        let config = RateLimitConfig::new(5, 1);
        let limiter = RateLimiter::new(config, vec![]);

        for _ in 0..5 {
            assert!(limiter.check_and_record("test_client").await);
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_excess() {
        let config = RateLimitConfig::new(3, 1);
        let limiter = RateLimiter::new(config, vec![]);

        for _ in 0..3 {
            assert!(limiter.check_and_record("test_client2").await);
        }

        assert!(!limiter.check_and_record("test_client2").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_different_clients() {
        let config = RateLimitConfig::new(2, 1);
        let limiter = RateLimiter::new(config, vec![]);

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
        let limiter = RateLimiter::new(config, vec![]);

        assert_eq!(limiter.remaining("test").await, 3);
        limiter.check_and_record("test").await;
        assert_eq!(limiter.remaining("test").await, 2);
        limiter.check_and_record("test").await;
        assert_eq!(limiter.remaining("test").await, 1);
    }

    // --- client_key tests ---

    fn peer(ip: &str) -> Option<SocketAddr> {
        Some(SocketAddr::new(ip.parse().unwrap(), 443))
    }

    #[test]
    fn empty_allowlist_ignores_spoofed_xff() {
        // Attacker sends a spoofed XFF — with empty allowlist it must be ignored.
        let key = client_key(peer("10.0.0.5"), Some("1.2.3.4"), &[]);
        assert_eq!(key, "10.0.0.5");
    }

    #[test]
    fn empty_allowlist_uses_peer_when_no_xff() {
        let key = client_key(peer("10.0.0.5"), None, &[]);
        assert_eq!(key, "10.0.0.5");
    }

    #[test]
    fn untrusted_peer_ignores_xff() {
        let proxies = vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))];
        // Peer is 10.0.0.5, not in the proxy list.
        let key = client_key(peer("10.0.0.5"), Some("1.2.3.4"), &proxies);
        assert_eq!(key, "10.0.0.5");
    }

    #[test]
    fn trusted_peer_with_single_xff_entry() {
        let proxies = vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))];
        // Peer is the trusted proxy, XFF has a single client entry.
        let key = client_key(peer("10.0.0.1"), Some("1.2.3.4"), &proxies);
        assert_eq!(key, "1.2.3.4");
    }

    #[test]
    fn trusted_peer_with_chain_of_proxies_resolves_rightmost_non_trusted() {
        let proxies = vec![
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
        ];
        // XFF: client, proxy2, proxy1  (left-to-right append order)
        // The rightmost non-trusted entry is "1.2.3.4" (the client).
        let key = client_key(
            peer("10.0.0.1"),
            Some("1.2.3.4, 10.0.0.2, 10.0.0.1"),
            &proxies,
        );
        assert_eq!(key, "1.2.3.4");
    }

    #[test]
    fn trusted_peer_all_xff_entries_are_proxies_falls_back_to_peer() {
        let proxies = vec![
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
        ];
        let key = client_key(peer("10.0.0.1"), Some("10.0.0.2, 10.0.0.1"), &proxies);
        assert_eq!(key, "10.0.0.1");
    }

    #[test]
    fn trusted_peer_no_xff_falls_back_to_peer() {
        let proxies = vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))];
        let key = client_key(peer("10.0.0.1"), None, &proxies);
        assert_eq!(key, "10.0.0.1");
    }

    #[test]
    fn missing_peer_returns_sentinel() {
        let key = client_key(None, Some("1.2.3.4"), &[]);
        assert_eq!(key, "misconfigured-no-connect-info");
    }

    /// Regression guard for a bypass: an unparseable XFF entry must never become
    /// the bucket key. Returning the raw text would let an attacker behind a
    /// pass-through proxy mint a fresh bucket per request with a random string,
    /// defeating the limiter this function exists to protect.
    #[test]
    fn trusted_peer_with_unparseable_xff_falls_back_to_peer() {
        let proxy: IpAddr = "10.0.0.1".parse().unwrap();
        for forged in [
            "not-an-ip",
            "garbage-8f3a91",
            "<script>",
            "1.2.3.4.5",
            "999.999.999.999",
        ] {
            let key = client_key(peer("10.0.0.1"), Some(forged), &[proxy]);
            assert_eq!(
                key, "10.0.0.1",
                "forged XFF {forged:?} must fall back to the peer IP, never become the key"
            );
        }
    }

    /// A forged unparseable entry to the LEFT of a genuine one must not shadow
    /// the genuine rightmost address.
    #[test]
    fn trusted_peer_prefers_rightmost_valid_over_forged_left() {
        let proxy: IpAddr = "10.0.0.1".parse().unwrap();
        let key = client_key(peer("10.0.0.1"), Some("garbage-abc, 203.0.113.7"), &[proxy]);
        assert_eq!(key, "203.0.113.7");
    }
}
