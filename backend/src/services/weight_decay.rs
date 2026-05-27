use std::f64::consts::E;

use crate::kernel::types::MemoryEntry;

/// Service that applies exponential decay to memory weights based on entry age.
#[derive(Debug, Clone)]
pub struct WeightDecayService {
    lambda: f64,
}

impl WeightDecayService {
    pub fn new(lambda: f64) -> Self {
        Self { lambda }
    }

    /// Apply exponential decay: w(t) = w0 * e^(-lambda * t)
    /// where t is the age in seconds.
    pub fn apply_decay(&self, entry: &MemoryEntry, age_seconds: u64) -> f64 {
        let w0 = entry.metadata.importance.max(1.0);
        let t = age_seconds as f64;
        w0 * E.powf(-self.lambda * t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::types::{LayerType, MemoryContent, MemoryMetadata};
    use std::collections::HashMap;

    fn make_entry(importance: f64) -> MemoryEntry {
        MemoryEntry {
            id: crate::kernel::types::MemoryId::new(),
            layer: LayerType::Stm,
            content: MemoryContent::Text("test".to_string()),
            metadata: MemoryMetadata {
                user_id: None,
                session_id: None,
                agent_id: None,
                tags: vec![],
                importance,
                access_count: 0,
                last_accessed: None,
                expires_at: None,
                source: None,
                extra: HashMap::new(),
            },
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn test_exponential_decay_formula() {
        let service = WeightDecayService::new(0.01);
        let entry = make_entry(1.0);

        // At t=0, decay factor should be 1.0
        let decayed = service.apply_decay(&entry, 0);
        assert!((decayed - 1.0).abs() < 1e-9);

        // At t=100, w(t) = 1.0 * e^(-0.01 * 100) = e^-1 ≈ 0.3679
        let decayed = service.apply_decay(&entry, 100);
        let expected = (-0.01 * 100.0_f64).exp();
        assert!((decayed - expected).abs() < 1e-6);

        // At t=693 (half-life ~69.3 for lambda=0.01), decay factor ≈ 0.001
        let decayed = service.apply_decay(&entry, 693);
        let expected = (-0.01 * 693.0_f64).exp();
        assert!((decayed - expected).abs() < 1e-3);
    }

    #[test]
    fn test_decay_with_different_importance() {
        let service = WeightDecayService::new(0.01);
        let entry = make_entry(2.0);

        let decayed = service.apply_decay(&entry, 0);
        assert!((decayed - 2.0).abs() < 1e-9);

        // w(t) = 2.0 * e^(-0.01 * 100) = 2 * e^-1
        let decayed = service.apply_decay(&entry, 100);
        let expected = 2.0 * (-0.01 * 100.0_f64).exp();
        assert!((decayed - expected).abs() < 1e-6);
    }

    #[test]
    fn test_higher_lambda_faster_decay() {
        let slow = WeightDecayService::new(0.01);
        let fast = WeightDecayService::new(0.1);
        let entry = make_entry(1.0);

        let slow_decay = slow.apply_decay(&entry, 100);
        let fast_decay = fast.apply_decay(&entry, 100);

        assert!(
            fast_decay < slow_decay,
            "higher lambda should produce faster decay"
        );
    }
}
