//! Circuit Breaker for External Providers

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

pub struct CircuitBreaker {
    failure_count: AtomicU32,
    last_failure_epoch: AtomicU64,
    half_open_probing: AtomicBool,
    threshold: u32,
    recovery_ms: u64,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, recovery_ms: u64) -> Self {
        Self {
            failure_count: AtomicU32::new(0),
            last_failure_epoch: AtomicU64::new(0),
            half_open_probing: AtomicBool::new(false),
            threshold,
            recovery_ms,
        }
    }

    pub fn state(&self) -> CircuitState {
        let failures = self.failure_count.load(Ordering::SeqCst);
        if failures < self.threshold {
            return CircuitState::Closed;
        }

        let last_failure = self.last_failure_epoch.load(Ordering::SeqCst);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if now - last_failure >= self.recovery_ms {
            CircuitState::HalfOpen
        } else {
            CircuitState::Open
        }
    }

    pub fn is_allowed(&self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => {
                // Only one probe request at a time in HalfOpen
                !self.half_open_probing.swap(true, Ordering::SeqCst)
            }
        }
    }

    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
        self.half_open_probing.store(false, Ordering::SeqCst);
    }

    pub fn record_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::SeqCst);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_failure_epoch.store(now, Ordering::SeqCst);
        self.half_open_probing.store(false, Ordering::SeqCst);
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, 30_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_closed() {
        let cb = CircuitBreaker::new(3, 1000);
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());
    }

    #[test]
    fn opens_after_threshold() {
        let cb = CircuitBreaker::new(3, 60_000);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.is_allowed());
    }

    #[test]
    fn resets_on_success() {
        let cb = CircuitBreaker::new(3, 1000);
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn half_open_after_recovery() {
        let cb = CircuitBreaker::new(3, 0);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        // recovery_ms = 0, so immediately half-open
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        assert!(cb.is_allowed());
    }

    #[test]
    fn half_open_allows_single_probe_only() {
        let cb = CircuitBreaker::new(3, 0);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        // First probe allowed
        assert!(cb.is_allowed());
        // Second probe rejected while first is in-flight
        assert!(!cb.is_allowed());
    }

    #[test]
    fn half_open_resets_probe_on_success() {
        let cb = CircuitBreaker::new(3, 0);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_allowed());
        cb.record_success();
        // After success, circuit is closed again
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());
    }
}
