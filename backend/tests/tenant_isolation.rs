//! Tenant Isolation Tests
//!
//! Tests for multi-tenant isolation guarantees per SEC-03 and D-07, D-08, D-09.
//!
//! ═══════════════════════════════════════════════════════════════════════════════
//! ⚠️  CRITICAL: SCOPE WARNING — READ BEFORE RELYING ON THESE TESTS
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! These tests ONLY verify that `TenantId::prefix()` produces correct
//! string-formatting output (the "t:{tenant}" prefix shape). They exercise
//! string comparison logic (`starts_with`, equality, display formatting).
//!
//! THEY DO NOT VERIFY DATABASE-LEVEL ISOLATION. Specifically:
//!   - No PostgreSQL RLS policy is exercised.
//!   - No cross-tenant query is run against a real database.
//!   - No begin_tenant_tx GUC scoping is tested.
//!   - No fail-closed (no-GUC) read path is tested.
//!
//! For actual database-level tenant isolation coverage, see:
//!   - tests/rls_isolation_pg.rs  (LTM — knowledge_entries)
//!   - tests/rls_kg_pg.rs         (KG — entities + relations)
//!   - tests/rls_mm_pg.rs         (MM — multimodal_entries + modality_relations)
//!   - tests/rls_stm_pg.rs        (STM — context_sessions + session_messages)
//!   - tests/tenant_scope_pg.rs   (tenant GUC scoping)
//!
//! These tests provide ZERO evidence of cross-tenant isolation at the storage
//! layer. They must not be read as isolation coverage.
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! These tests verify:
//! - Tenant ID prefix isolation
//! - Cross-tenant access rejection
//! - Isolation violation recording

use backend::tenant::TenantId;

/// Test that different tenants get different prefixes.
#[test]
fn test_tenant_id_prefix_isolation() {
    let tenant_a = TenantId::from_string("tenant_a");
    let tenant_b = TenantId::from_string("tenant_b");

    // Different tenants must have different prefixes
    assert_ne!(tenant_a.prefix(), tenant_b.prefix());

    // Prefixes must start with "t:" format
    assert!(tenant_a.prefix().starts_with("t:"));
    assert!(tenant_b.prefix().starts_with("t:"));

    // Prefixes must contain the tenant identifier
    assert!(tenant_a.prefix().contains("tenant_a"));
    assert!(tenant_b.prefix().contains("tenant_b"));
}

/// Test source_id prefix matching logic.
#[test]
fn test_source_id_prefix_matching() {
    let tenant = TenantId::from_string("org_123");
    let prefix = tenant.prefix(); // "t:org_123"

    // Matching entries (belong to tenant)
    assert!("t:org_123:user_1".starts_with(&prefix));
    assert!("t:org_123:agent_5".starts_with(&prefix));
    assert!("t:org_123".starts_with(&prefix));

    // Non-matching entries (don't belong to tenant)
    assert!(!"t:other_org:user_1".starts_with(&prefix));
    assert!(!"user_1".starts_with(&prefix));
    assert!(!"t:".starts_with(&prefix));
}

/// Test that same tenant can access their own data.
#[test]
fn test_same_tenant_access_allowed() {
    let tenant = TenantId::from_string("tenant_a");
    let source_id = format!("{}:user_1", tenant.prefix());

    // Same tenant should match
    assert!(source_id.starts_with(&tenant.prefix()));
}

/// Test that different tenant cannot access another tenant's data.
#[test]
fn test_cross_tenant_access_rejected() {
    let tenant_a = TenantId::from_string("tenant_a");
    let tenant_b = TenantId::from_string("tenant_b");

    // tenant_b's data
    let cross_tenant_source_id = format!("{}:user_1", tenant_b.prefix());

    // tenant_a should NOT be able to access tenant_b's data
    assert!(!cross_tenant_source_id.starts_with(&tenant_a.prefix()));
}

/// Test isolation violation recording function signature.
/// Note: Full integration test requires metrics setup.
#[test]
fn test_isolation_violation_function_exists() {
    use backend::services::multi_tenant::record_isolation_violation;

    let tenant = TenantId::from_string("malicious_tenant");
    // Function should exist and be callable (doesn't panic)
    record_isolation_violation(tenant.as_str(), "entry_123", "test_violation");
}

/// Test tenant ID equality and comparison.
#[test]
fn test_tenant_id_equality() {
    let tenant1 = TenantId::from_string("tenant_1");
    let tenant2 = TenantId::from_string("tenant_1");
    let tenant3 = TenantId::from_string("tenant_2");

    // Same string should produce equal TenantIds
    assert_eq!(tenant1, tenant2);

    // Different strings should produce unequal TenantIds
    assert_ne!(tenant1, tenant3);

    // as_str() should return the underlying string
    assert_eq!(tenant1.as_str(), "tenant_1");
}

/// Test TenantId display and string conversion.
#[test]
fn test_tenant_id_display() {
    let tenant = TenantId::from_string("test_tenant");
    let display = format!("{}", tenant);
    assert_eq!(display, "test_tenant");

    let from_str = tenant.as_str();
    assert_eq!(from_str, "test_tenant");
}

/// Test tenant prefix format consistency.
#[test]
fn test_tenant_prefix_format() {
    let tenant = TenantId::from_string("my-org-123");
    let prefix = tenant.prefix();

    // Expected format: "t:" + tenant_id
    assert_eq!(prefix, "t:my-org-123");

    // Verify prefix works with various tenant ID formats
    let tenant_with_underscore = TenantId::from_string("tenant_with_underscore");
    assert_eq!(tenant_with_underscore.prefix(), "t:tenant_with_underscore");

    let tenant_with_numbers = TenantId::from_string("tenant123");
    assert_eq!(tenant_with_numbers.prefix(), "t:tenant123");
}
