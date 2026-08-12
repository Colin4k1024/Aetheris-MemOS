use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use utoipa::ToSchema;

/// Recovery strategy for self-healing
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStrategy {
    RestartLayer,
    ClearStale,
    ReloadBackup,
}

impl Default for RecoveryStrategy {
    fn default() -> Self {
        Self::RestartLayer
    }
}

/// Layer health status
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LayerHealth {
    pub layer: String,
    pub healthy: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

/// Overall health status
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthStatus {
    pub overall_healthy: bool,
    pub layers: Vec<LayerHealth>,
    pub timestamp: String,
    /// `false` — this payload does **not** contact PostgreSQL, Qdrant or any
    /// other external dependency. Present so that an operator or dashboard
    /// cannot mistake it for a readiness signal; use `/readyz` for that.
    pub is_dependency_probe: bool,
    /// Human-readable statement of what this payload does and does not cover.
    pub note: String,
}

/// Self-healing service for autonomous diagnosis and constrained self-healing
#[derive(Debug)]
pub struct SelfHealingService {
    max_attempts: u32,
    base_backoff_ms: u64,
}

impl Default for SelfHealingService {
    fn default() -> Self {
        Self::new()
    }
}

impl SelfHealingService {
    pub fn new() -> Self {
        Self {
            max_attempts: 3,
            base_backoff_ms: 100,
        }
    }

    /// Report per-layer status of the in-process memory layers.
    ///
    /// # This is not a dependency probe
    ///
    /// The `layers/*` backends are currently `RwLock<HashMap>` in-process stubs
    /// (see backlog item A-6 / D-2 — wiring them to `db::*Repository` is
    /// pending), so there is nothing to round-trip against and no latency to
    /// measure. `healthy: true` is therefore accurate but uninformative: an
    /// in-process map does not fail.
    ///
    /// Earlier versions also reported fabricated 1–4 ms latencies, which made
    /// this look like a real probe and led operators to trust it. The latencies
    /// are now `None` and [`HealthStatus::is_dependency_probe`] is `false`.
    ///
    /// For actual liveness/readiness use `/livez` and `/readyz`
    /// (`routers/probes.rs`), which round-trip PostgreSQL and Qdrant.
    pub fn check_health(&self) -> HealthStatus {
        let layers = vec![
            self.check_stm_health(),
            self.check_ltm_health(),
            self.check_kg_health(),
            self.check_mm_health(),
        ];

        let overall_healthy = layers.iter().all(|l| l.healthy);

        HealthStatus {
            overall_healthy,
            layers,
            timestamp: chrono_lite_now(),
            is_dependency_probe: false,
            note: "In-process memory-layer status only; the layer backends are \
                   in-memory stubs, so no external dependency is contacted. Use \
                   /readyz for a real PostgreSQL + Qdrant readiness probe."
                .to_string(),
        }
    }

    /// In-process layer status. `latency_ms` is deliberately `None` — see
    /// [`Self::check_health`]; reporting a number here would be fabricated.
    fn layer_status(layer: &str) -> LayerHealth {
        LayerHealth {
            layer: layer.to_string(),
            healthy: true,
            latency_ms: None,
            error: None,
        }
    }

    fn check_stm_health(&self) -> LayerHealth {
        Self::layer_status("stm")
    }

    fn check_ltm_health(&self) -> LayerHealth {
        Self::layer_status("ltm")
    }

    fn check_kg_health(&self) -> LayerHealth {
        Self::layer_status("kg")
    }

    fn check_mm_health(&self) -> LayerHealth {
        Self::layer_status("mm")
    }

    /// Attempt recovery with exponential backoff
    pub fn attempt_recovery(
        &self,
        strategy: RecoveryStrategy,
        fault_id: &str,
    ) -> Result<bool, RecoveryError> {
        let mut attempt = 0;
        let mut backoff = self.base_backoff_ms;

        while attempt < self.max_attempts {
            let start = Instant::now();

            // Simulate recovery attempt
            let success = self.execute_recovery(strategy, fault_id)?;

            if success {
                return Ok(true);
            }

            // Exponential backoff: 100ms, 200ms, 400ms
            std::thread::sleep(Duration::from_millis(backoff));
            backoff *= 2;
            attempt += 1;
            let _ = start.elapsed(); // Silence unused warning
        }

        Err(RecoveryError::MaxAttemptsReached {
            fault_id: fault_id.to_string(),
            attempts: self.max_attempts,
        })
    }

    fn execute_recovery(
        &self,
        strategy: RecoveryStrategy,
        fault_id: &str,
    ) -> Result<bool, RecoveryError> {
        match strategy {
            RecoveryStrategy::RestartLayer => {
                tracing::info!(fault_id, "attempting restart_layer recovery");
                Ok(true)
            }
            RecoveryStrategy::ClearStale => {
                tracing::info!(fault_id, "attempting clear_stale recovery");
                Ok(true)
            }
            RecoveryStrategy::ReloadBackup => {
                tracing::info!(fault_id, "attempting reload_backup recovery");
                Ok(true)
            }
        }
    }

    pub fn get_max_attempts(&self) -> u32 {
        self.max_attempts
    }

    pub fn get_base_backoff_ms(&self) -> u64 {
        self.base_backoff_ms
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("max recovery attempts ({attempts}) reached for fault: {fault_id}")]
    MaxAttemptsReached { fault_id: String, attempts: u32 },

    #[error("recovery failed: {0}")]
    ExecutionFailed(String),
}

fn chrono_lite_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_returns_all_layers() {
        let service = SelfHealingService::new();
        let status = service.check_health();

        assert!(status.layers.len() == 4);
        assert_eq!(status.layers.iter().filter(|l| l.healthy).count(), 4);
        assert!(status.overall_healthy);
    }

    #[test]
    fn test_recovery_attempts_limited() {
        let service = SelfHealingService::new();
        assert_eq!(service.get_max_attempts(), 3);
    }

    #[test]
    fn test_recovery_strategies() {
        let service = SelfHealingService::new();

        assert!(service
            .attempt_recovery(RecoveryStrategy::RestartLayer, "test-fault")
            .is_ok());
        assert!(service
            .attempt_recovery(RecoveryStrategy::ClearStale, "test-fault")
            .is_ok());
        assert!(service
            .attempt_recovery(RecoveryStrategy::ReloadBackup, "test-fault")
            .is_ok());
    }
}
