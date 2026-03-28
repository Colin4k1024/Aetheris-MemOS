//! Integration tests for SelfHealingService

use backend::services::self_healing::{RecoveryStrategy, SelfHealingService};

#[test]
fn test_health_check() {
    let service = SelfHealingService::new();
    let status = service.check_health();

    assert!(status.overall_healthy);
    assert_eq!(status.layers.len(), 4);

    let layer_names: Vec<&str> = status.layers.iter().map(|l| l.layer.as_str()).collect();
    assert!(layer_names.contains(&"stm"));
    assert!(layer_names.contains(&"ltm"));
    assert!(layer_names.contains(&"kg"));
    assert!(layer_names.contains(&"mm"));
}

#[test]
fn test_recovery_attempts_limited() {
    let service = SelfHealingService::new();
    assert_eq!(service.get_max_attempts(), 3);
}

#[test]
fn test_recovery_strategies_all_succeed() {
    let service = SelfHealingService::new();

    assert!(service
        .attempt_recovery(RecoveryStrategy::RestartLayer, "fault-1")
        .is_ok());
    assert!(service
        .attempt_recovery(RecoveryStrategy::ClearStale, "fault-2")
        .is_ok());
    assert!(service
        .attempt_recovery(RecoveryStrategy::ReloadBackup, "fault-3")
        .is_ok());
}

#[test]
fn test_health_status_contains_timestamp() {
    let service = SelfHealingService::new();
    let status = service.check_health();

    assert!(!status.timestamp.is_empty());
}

#[test]
fn test_all_layers_healthy_by_default() {
    let service = SelfHealingService::new();
    let status = service.check_health();

    for layer in &status.layers {
        assert!(layer.healthy, "Layer {} should be healthy", layer.layer);
        assert!(
            layer.error.is_none(),
            "Layer {} should have no error",
            layer.layer
        );
    }
}
