//! Prompt Injection Probe Tests

use backend::services::prompt_injection_probe::ProbeResult;

#[test]
fn test_probe_result_variants() {
    // Test Clean variant
    let clean = ProbeResult::Clean;
    assert!(matches!(clean, ProbeResult::Clean));

    // Test Flagged variant
    let flagged = ProbeResult::Flagged {
        reason: "test".to_string(),
        confidence: 0.8,
    };
    assert!(matches!(flagged, ProbeResult::Flagged { .. }));

    // Test Quarantined variant
    let quarantined = ProbeResult::Quarantined;
    assert!(matches!(quarantined, ProbeResult::Quarantined));
}

#[test]
fn test_probe_result_helpers() {
    let clean = ProbeResult::Clean;
    assert!(clean.is_clean());
    assert!(!clean.is_flagged());
    assert!(!clean.is_quarantined());

    let flagged = ProbeResult::Flagged {
        reason: "test".to_string(),
        confidence: 0.8,
    };
    assert!(!flagged.is_clean());
    assert!(flagged.is_flagged());
    assert!(!flagged.is_quarantined());

    let quarantined = ProbeResult::Quarantined;
    assert!(!quarantined.is_clean());
    assert!(!quarantined.is_flagged());
    assert!(quarantined.is_quarantined());
}

#[test]
fn test_probe_result_serde() {
    let result = ProbeResult::Flagged {
        reason: "keyword match".to_string(),
        confidence: 0.95,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("Flagged"));
    assert!(json.contains("keyword match"));
    assert!(json.contains("0.95"));
}
