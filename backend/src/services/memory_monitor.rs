//! Memory behavior monitoring (#124 行为监控 layer).
//!
//! Pure evaluator + one counter — no new infrastructure. The consolidation
//! worker feeds it each tenant's per-cycle stats; the evaluator flags the
//! Epic's anomaly classes:
//!
//! - `write_rate_spike` — belief writes/sec far above the cycle norm
//!   (flood/abuse signal);
//! - `policy_memory_surge` — a sudden wave of quarantined/pending candidates
//!   ("突然出现政策类记忆");
//! - `belief_growth_runaway` — active belief volume growing across cycles
//!   instead of staying stable ("三个月后现行信念数稳定" as a RUNTIME check,
//!   not only a golden test).
//!
//! Every alert increments `memory_anomaly_alerts_total{type}` (real caller:
//! the worker) and logs at WARN. Thresholds are deliberately conservative —
//! this is a tripwire, not a rate limiter.

use crate::services::prometheus_exporter::get_exporter;

/// Anomaly classes — Prometheus label values (bounded set by construction).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalyType {
    WriteRateSpike,
    PolicyMemorySurge,
    BeliefGrowthRunaway,
}

impl AnomalyType {
    pub fn as_str(self) -> &'static str {
        match self {
            AnomalyType::WriteRateSpike => "write_rate_spike",
            AnomalyType::PolicyMemorySurge => "policy_memory_surge",
            AnomalyType::BeliefGrowthRunaway => "belief_growth_runaway",
        }
    }
}

/// Per-tenant inputs for one evaluation. Cheap: everything is already
/// computed by the consolidation cycle or readable from its report.
#[derive(Debug, Clone, Default)]
pub struct MonitorInputs {
    /// Belief writes observed this cycle (gate commits + supersedes).
    pub belief_writes: u64,
    /// Seconds the cycle window covers.
    pub window_seconds: u64,
    /// Quarantined + pending candidates AFTER this cycle.
    pub policy_queue_depth: u64,
    /// Quarantined + pending candidates AFTER the previous cycle.
    pub prev_policy_queue_depth: u64,
    /// Active (current-truth) belief count after this cycle.
    pub active_beliefs: u64,
    /// Active belief count after the previous cycle (first cycle: same value).
    pub prev_active_beliefs: u64,
}

/// Thresholds. Defaults are intentionally simple tripwires; tuning belongs to
/// configuration once real traffic shapes exist.
#[derive(Debug, Clone)]
pub struct MonitorThresholds {
    /// writes/sec above this flags a spike.
    pub max_writes_per_second: f64,
    /// queue growth (delta) above this flags a policy surge.
    pub max_queue_growth_per_cycle: u64,
    /// active-belief growth ratio above this (and non-trivial absolute delta)
    /// flags runaway growth.
    pub max_growth_ratio: f64,
    /// ignore growth below this absolute delta (noise floor).
    pub growth_noise_floor: u64,
}

impl Default for MonitorThresholds {
    fn default() -> Self {
        Self {
            max_writes_per_second: 50.0,
            max_queue_growth_per_cycle: 20,
            max_growth_ratio: 0.5,
            growth_noise_floor: 20,
        }
    }
}

/// Evaluate one cycle. Returns the triggered anomaly classes (may be empty).
/// Pure: same inputs, same outputs — unit-testable without a database.
pub fn evaluate(inputs: &MonitorInputs, t: &MonitorThresholds) -> Vec<AnomalyType> {
    let mut alerts = Vec::new();

    if inputs.window_seconds > 0 {
        let wps = inputs.belief_writes as f64 / inputs.window_seconds as f64;
        if wps > t.max_writes_per_second {
            alerts.push(AnomalyType::WriteRateSpike);
        }
    }

    if inputs
        .policy_queue_depth
        .saturating_sub(inputs.prev_policy_queue_depth)
        > t.max_queue_growth_per_cycle
    {
        alerts.push(AnomalyType::PolicyMemorySurge);
    }

    let delta = inputs
        .active_beliefs
        .saturating_sub(inputs.prev_active_beliefs);
    if delta > t.growth_noise_floor && inputs.prev_active_beliefs > 0 {
        let ratio = delta as f64 / inputs.prev_active_beliefs as f64;
        if ratio > t.max_growth_ratio {
            alerts.push(AnomalyType::BeliefGrowthRunaway);
        }
    }

    alerts
}

/// Record triggered alerts: metric counter + WARN log (the "告警" in
/// 指标和告警). Called by the consolidation worker with real inputs.
pub fn record_alerts(tenant_id: &str, alerts: &[AnomalyType]) {
    for a in alerts {
        get_exporter().inc_memory_anomaly(a.as_str());
        tracing::warn!(
            tenant = tenant_id,
            anomaly = a.as_str(),
            "memory behavior anomaly detected"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> MonitorInputs {
        MonitorInputs {
            belief_writes: 10,
            window_seconds: 3600,
            policy_queue_depth: 3,
            prev_policy_queue_depth: 3,
            active_beliefs: 100,
            prev_active_beliefs: 100,
        }
    }

    #[test]
    fn healthy_cycle_is_silent() {
        assert!(evaluate(&inputs(), &MonitorThresholds::default()).is_empty());
    }

    #[test]
    fn write_rate_spike_fires_on_flood() {
        let mut i = inputs();
        i.belief_writes = 1_000_000; // ~278/s over an hour
        i.window_seconds = 3600;
        assert_eq!(
            evaluate(&i, &MonitorThresholds::default()),
            vec![AnomalyType::WriteRateSpike]
        );
    }

    #[test]
    fn policy_surge_fires_on_queue_jump() {
        let mut i = inputs();
        i.policy_queue_depth = 40;
        i.prev_policy_queue_depth = 5;
        assert_eq!(
            evaluate(&i, &MonitorThresholds::default()),
            vec![AnomalyType::PolicyMemorySurge]
        );
    }

    #[test]
    fn runaway_growth_ignores_noise_and_fires_on_explosion() {
        let t = MonitorThresholds::default();
        // Small absolute delta: below the noise floor even at 100% ratio.
        let mut noise = inputs();
        noise.active_beliefs = 110;
        noise.prev_active_beliefs = 100;
        assert!(evaluate(&noise, &t).is_empty(), "10 edges is noise");

        // Doubling a real store is not noise.
        let mut boom = inputs();
        boom.active_beliefs = 300;
        boom.prev_active_beliefs = 100;
        assert_eq!(evaluate(&boom, &t), vec![AnomalyType::BeliefGrowthRunaway]);
    }

    #[test]
    fn zero_window_does_not_divide_by_zero() {
        let mut i = inputs();
        i.window_seconds = 0;
        i.belief_writes = u64::MAX;
        assert!(evaluate(&i, &MonitorThresholds::default()).is_empty());
    }
}
