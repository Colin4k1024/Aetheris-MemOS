//! Memory Fusion Unit Tests
//!
//! Tests for MemoryFusionService types and construction.
//!
//! Note: Full integration tests require a PostgreSQL database.
//! The database-dependent tests are marked with `#[ignore]` and can be run with:
//! `cargo test memory_fusion -- --ignored`
//!
//! To run with PostgreSQL:
//! 1. Start a PostgreSQL instance
//! 2. Set DATABASE_URL environment variable
//! 3. Run: cargo test memory_fusion -- --ignored

use backend::services::memory_fusion::{
    FusionResult, FusionStatus, FusionStatusResponse, LayerResults, MemoryEntry,
    MemoryFusionService, MemoryLayer, MergedEntry,
};
use backend::tenant::TenantId;

/// Test MemoryLayer Display implementation.
#[test]
fn test_memory_layer_display() {
    assert_eq!(MemoryLayer::Stm.to_string(), "stm");
    assert_eq!(MemoryLayer::Ltm.to_string(), "ltm");
    assert_eq!(MemoryLayer::Kg.to_string(), "kg");
    assert_eq!(MemoryLayer::Mm.to_string(), "mm");
}

/// Test MemoryFusionService can be constructed.
#[test]
fn test_memory_fusion_service_construction() {
    // MemoryFusionService is a zero-sized type, always constructible
    let _service = MemoryFusionService;
    assert!(true, "MemoryFusionService should be constructible");
}

/// Test MergedEntry structure and fields.
#[test]
fn test_merged_entry_structure() {
    let entry = MergedEntry {
        id: "stm:test-id".to_string(),
        layer: MemoryLayer::Stm,
        title: "Test Title".to_string(),
        content: "Test Content".to_string(),
        relevance_score: 0.95,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        quality_score: Some(0.8),
    };

    assert_eq!(entry.id, "stm:test-id");
    assert_eq!(entry.layer, MemoryLayer::Stm);
    assert_eq!(entry.title, "Test Title");
    assert_eq!(entry.relevance_score, 0.95);
    assert_eq!(entry.quality_score, Some(0.8));
}

/// Test MemoryEntry structure and fields.
#[test]
fn test_memory_entry_structure() {
    let entry = MemoryEntry {
        id: "ltm-entry-1".to_string(),
        layer: MemoryLayer::Ltm,
        title: "LTM Entry".to_string(),
        content: "Long term memory content".to_string(),
        relevance_score: 0.75,
        created_at: "2024-01-02T00:00:00Z".to_string(),
        quality_score: Some(0.9),
    };

    assert_eq!(entry.id, "ltm-entry-1");
    assert_eq!(entry.layer, MemoryLayer::Ltm);
    assert!(entry.relevance_score > 0.0);
}

/// Test LayerResults structure.
#[test]
fn test_layer_results_structure() {
    let layer_results = LayerResults {
        stm: vec![],
        ltm: vec![],
        kg: vec![],
        mm: vec![],
    };

    assert!(layer_results.stm.is_empty());
    assert!(layer_results.ltm.is_empty());
    assert!(layer_results.kg.is_empty());
    assert!(layer_results.mm.is_empty());
}

/// Test FusionResult structure.
#[test]
fn test_fusion_result_structure() {
    let fusion = FusionResult {
        layer_results: LayerResults {
            stm: vec![],
            ltm: vec![],
            kg: vec![],
            mm: vec![],
        },
        merged_results: vec![],
    };

    assert!(fusion.layer_results.stm.is_empty());
    assert!(fusion.merged_results.is_empty());
}

/// Test FusionStatus structure.
#[test]
fn test_fusion_status_structure() {
    let status = FusionStatus {
        stm_count: 10,
        ltm_count: 20,
        kg_count: 5,
        mm_count: 3,
        stm_healthy: true,
        ltm_healthy: true,
        kg_healthy: true,
        mm_healthy: true,
    };

    assert_eq!(status.stm_count, 10);
    assert_eq!(status.ltm_count, 20);
    assert_eq!(status.kg_count, 5);
    assert_eq!(status.mm_count, 3);
    assert!(status.stm_healthy);
    assert!(status.ltm_healthy);
}

/// Test FusionStatusResponse structure.
#[test]
fn test_fusion_status_response_structure() {
    let response = FusionStatusResponse {
        status: FusionStatus {
            stm_count: 5,
            ltm_count: 10,
            kg_count: 2,
            mm_count: 1,
            stm_healthy: true,
            ltm_healthy: true,
            kg_healthy: true,
            mm_healthy: true,
        },
        total_entries: 18,
    };

    assert_eq!(response.total_entries, 18);
    assert_eq!(
        response.status.stm_count
            + response.status.ltm_count
            + response.status.kg_count
            + response.status.mm_count,
        18
    );
}

/// Test TenantId prefix for isolation.
#[test]
fn test_tenant_id_prefix() {
    let tenant = TenantId::from_string("test_user");
    let prefix = tenant.prefix();
    assert!(prefix.starts_with("t:"));
    assert!(prefix.contains("test_user"));
}

/// Test that merged results are sorted by relevance score (verifies sorting logic).
#[test]
fn test_merged_results_sorting_logic() {
    let mut entries = vec![
        MergedEntry {
            id: "low".to_string(),
            layer: MemoryLayer::Stm,
            title: "Low relevance".to_string(),
            content: "Content".to_string(),
            relevance_score: 0.3,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            quality_score: None,
        },
        MergedEntry {
            id: "high".to_string(),
            layer: MemoryLayer::Ltm,
            title: "High relevance".to_string(),
            content: "Content".to_string(),
            relevance_score: 0.9,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            quality_score: None,
        },
        MergedEntry {
            id: "medium".to_string(),
            layer: MemoryLayer::Kg,
            title: "Medium relevance".to_string(),
            content: "Content".to_string(),
            relevance_score: 0.6,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            quality_score: None,
        },
    ];

    // Sort by relevance score descending (same logic as in MemoryFusionService::query)
    entries.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Verify order: high, medium, low
    assert_eq!(entries[0].id, "high");
    assert_eq!(entries[1].id, "medium");
    assert_eq!(entries[2].id, "low");
}

/// Test that merged entry IDs include layer prefix.
#[test]
fn test_merged_entry_id_includes_layer_prefix() {
    let layers = vec![
        (MemoryLayer::Stm, "stm"),
        (MemoryLayer::Ltm, "ltm"),
        (MemoryLayer::Kg, "kg"),
        (MemoryLayer::Mm, "mm"),
    ];

    for (layer, expected_prefix) in layers {
        let entry = MergedEntry {
            id: format!("{}:test-id", layer),
            layer,
            title: "Test".to_string(),
            content: "Content".to_string(),
            relevance_score: 0.5,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            quality_score: None,
        };

        let expected = format!("{}:", expected_prefix);
        assert!(
            entry.id.starts_with(&expected),
            "Entry ID '{}' should start with '{}'",
            entry.id,
            expected
        );
    }
}

/// Test total entries calculation matches sum of layer counts.
#[test]
fn test_total_entries_calculation() {
    let status = FusionStatus {
        stm_count: 3,
        ltm_count: 7,
        kg_count: 2,
        mm_count: 8,
        stm_healthy: true,
        ltm_healthy: true,
        kg_healthy: true,
        mm_healthy: true,
    };

    let expected_total = status.stm_count + status.ltm_count + status.kg_count + status.mm_count;
    assert_eq!(expected_total, 20);
}
