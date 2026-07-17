//! Adaptive Telemetry Pipeline
//!
//! Collects `PerformanceSample` records pairing predicted vs actual performance
//! metrics for future online learning. The predictor (W3.2) currently uses
//! heuristic baseline constants; these samples provide the training data needed
//! to move toward learned coefficients.

use std::sync::{Arc, OnceLock, RwLock};

use serde::{Deserialize, Serialize};

/// A single predicted-vs-actual performance observation.
///
/// Pairs the predictor's output with the realized efficiency/coherence so a
/// future `TrainablePredictor` can fit its coefficients to observed data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSample {
    /// Task complexity score from the analyzer.
    pub task_complexity: f64,
    /// Number of modalities involved in the task.
    pub modality_count: u32,
    /// Reasoning depth label (e.g. "shallow", "medium", "deep").
    pub reasoning_depth: String,
    /// JSON serialization of the `MemoryConfig` the prediction was made for.
    pub memory_config_json: String,
    /// Efficiency the predictor estimated for this config.
    pub predicted_efficiency: f64,
    /// Efficiency actually observed after running the task.
    pub actual_efficiency: f64,
    /// Coherence the predictor estimated for this config.
    pub predicted_coherence: f64,
    /// Coherence actually observed after running the task.
    pub actual_coherence: f64,
    /// ISO-8601 timestamp marking when the sample was recorded.
    pub timestamp: String,
}

/// In-memory store of collected telemetry samples.
///
/// Backed by an `Arc<RwLock<Vec<_>>>` so that prediction paths can record
/// samples without blocking readers. Samples are kept in-process; a future
/// persistence worker can drain the buffer to durable storage.
pub struct AdaptiveTelemetry {
    samples: Arc<RwLock<Vec<PerformanceSample>>>,
}

impl AdaptiveTelemetry {
    /// Create a new empty telemetry buffer.
    pub fn new() -> Self {
        Self {
            samples: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Append a performance sample to the buffer.
    pub fn record_sample(&self, sample: PerformanceSample) {
        // A poisoned lock indicates a panic during interior mutation; treat it
        // as fatal since telemetry must not silently drop samples.
        let mut samples = self.samples.write().expect("telemetry lock poisoned");
        samples.push(sample);
    }

    /// Read-only access to the current sample buffer (cloned snapshot).
    pub fn snapshot(&self) -> Vec<PerformanceSample> {
        let samples = self.samples.read().expect("telemetry lock poisoned");
        samples.clone()
    }

    /// Number of samples currently buffered.
    pub fn len(&self) -> usize {
        let samples = self.samples.read().expect("telemetry lock poisoned");
        samples.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for AdaptiveTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Instance
// ============================================================================

/// Global adaptive telemetry store. Initialized lazily on first access.
static ADAPTIVE_TELEMETRY: OnceLock<AdaptiveTelemetry> = OnceLock::new();

/// Initialize the global adaptive telemetry store.
///
/// Safe to call multiple times; subsequent calls are no-ops since the slot is
/// already populated. Returns a reference to the global instance.
pub fn init_adaptive_telemetry() -> &'static AdaptiveTelemetry {
    ADAPTIVE_TELEMETRY.get_or_init(AdaptiveTelemetry::new)
}

/// Record a performance sample to the global telemetry store.
///
/// Lazily initializes the store on first call so callers do not need to worry
/// about ordering with `init_adaptive_telemetry`.
pub fn record_performance_sample(sample: PerformanceSample) {
    let store = ADAPTIVE_TELEMETRY.get_or_init(AdaptiveTelemetry::new);
    store.record_sample(sample);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sample(predicted: f64, actual: f64) -> PerformanceSample {
        PerformanceSample {
            task_complexity: 0.5,
            modality_count: 2,
            reasoning_depth: "deep".to_string(),
            memory_config_json: "{}".to_string(),
            predicted_efficiency: predicted,
            actual_efficiency: actual,
            predicted_coherence: predicted,
            actual_coherence: actual,
            timestamp: "2026-07-17T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn record_and_snapshot_samples() {
        let telemetry = AdaptiveTelemetry::new();
        assert!(telemetry.is_empty());

        telemetry.record_sample(make_sample(0.8, 0.7));
        telemetry.record_sample(make_sample(0.6, 0.65));

        let snapshot = telemetry.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(telemetry.len(), 2);
        assert!(!telemetry.is_empty());
        assert_eq!(snapshot[0].predicted_efficiency, 0.8);
        assert_eq!(snapshot[1].actual_coherence, 0.65);
    }

    #[test]
    fn default_is_empty() {
        let telemetry = AdaptiveTelemetry::default();
        assert!(telemetry.is_empty());
    }
}
