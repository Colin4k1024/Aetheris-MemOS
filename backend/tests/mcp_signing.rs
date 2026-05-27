//! MCP Component Signing Integration Tests
//!
//! Tests for MCP component signing and verification behavior.

use backend::mcp::signing::SigningError;
use backend::mcp::signing::{
    compute_signature, verify_component, verify_unsigned, ComponentSignature, TrustedKeyBundle,
};
use sha2::Digest;

/// Test helper to create a test key bundle
fn test_key_bundle() -> TrustedKeyBundle {
    use std::collections::HashMap;

    let mut keys = HashMap::new();
    // 32-byte test key
    let secret_key =
        hex::decode("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef").unwrap();
    keys.insert("test-issuer".to_string(), secret_key);
    TrustedKeyBundle::new(keys)
}

#[test]
fn test_valid_signature_passes_verification() {
    let bundle = test_key_bundle();
    let artifact = b"test_tool_v1";
    let issuer = "test-issuer";
    let version = "1.0.0";
    let timestamp = 1710000000i64;

    let key = bundle.get_key(issuer).expect("issuer should be trusted");

    // Compute SHA-256 hash of artifact
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, artifact);
    let artifact_hash = hex::encode(hasher.finalize());

    // Compute HMAC-SHA256 signature
    let signature = compute_signature(artifact, issuer, version, timestamp, key)
        .expect("signature computation should succeed");

    let sig = ComponentSignature {
        component_id: "test_tool".to_string(),
        sha256_hash: artifact_hash,
        issuer: issuer.to_string(),
        version: version.to_string(),
        timestamp,
        signature,
    };

    // Verification should pass
    let result = verify_component("test_tool", artifact, &sig, &bundle);
    assert!(
        result.is_ok(),
        "Valid signature should pass verification: {:?}",
        result
    );
}

#[test]
fn test_tampered_artifact_fails_verification() {
    let bundle = test_key_bundle();
    let original_artifact = b"test_tool_v1";
    let tampered_artifact = b"test_tool_v2_tampered";
    let issuer = "test-issuer";
    let version = "1.0.0";
    let timestamp = 1710000000i64;

    let key = bundle.get_key(issuer).expect("issuer should be trusted");

    // Compute SHA-256 hash of original artifact
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, original_artifact);
    let artifact_hash = hex::encode(hasher.finalize());

    // Compute signature with original artifact
    let signature = compute_signature(original_artifact, issuer, version, timestamp, key)
        .expect("signature computation should succeed");

    let sig = ComponentSignature {
        component_id: "test_tool".to_string(),
        sha256_hash: artifact_hash,
        issuer: issuer.to_string(),
        version: version.to_string(),
        timestamp,
        signature,
    };

    // Verification should fail with tampered artifact
    let result = verify_component("test_tool", tampered_artifact, &sig, &bundle);
    assert!(
        matches!(result, Err(SigningError::VerificationFailed(_, _))),
        "Tampered artifact should fail verification: {:?}",
        result
    );
}

#[test]
fn test_unknown_issuer_fails_verification() {
    let bundle = test_key_bundle();
    let artifact = b"test_tool_v1";

    let sig = ComponentSignature {
        component_id: "test_tool".to_string(),
        sha256_hash: "abcd1234".to_string(),
        issuer: "unknown-issuer".to_string(),
        version: "1.0.0".to_string(),
        timestamp: 1710000000,
        signature: "signature".to_string(),
    };

    let result = verify_component("test_tool", artifact, &sig, &bundle);
    assert!(
        matches!(result, Err(SigningError::UnknownIssuer(_))),
        "Unknown issuer should fail verification: {:?}",
        result
    );
}

#[test]
fn test_component_without_signature_is_rejected() {
    let result = verify_unsigned("test_tool");
    assert!(
        matches!(result, Err(SigningError::Unsigned(_))),
        "Unsigned component should be rejected: {:?}",
        result
    );
}

#[test]
fn test_signature_with_wrong_key_fails() {
    use std::collections::HashMap;

    let bundle = test_key_bundle();
    let artifact = b"test_tool_v1";
    let issuer = "test-issuer";
    let version = "1.0.0";
    let timestamp = 1710000000i64;

    // Use wrong key
    let wrong_key =
        hex::decode("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").unwrap();

    let signature = compute_signature(artifact, issuer, version, timestamp, &wrong_key)
        .expect("signature computation should succeed");

    // Get the correct key
    let key = bundle.get_key(issuer).expect("issuer should be trusted");

    // Compute hash with correct key context
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, artifact);
    let artifact_hash = hex::encode(hasher.finalize());

    let sig = ComponentSignature {
        component_id: "test_tool".to_string(),
        sha256_hash: artifact_hash,
        issuer: issuer.to_string(),
        version: version.to_string(),
        timestamp,
        signature,
    };

    // Verification should fail - signature was made with wrong key
    let result = verify_component("test_tool", artifact, &sig, &bundle);
    assert!(
        matches!(result, Err(SigningError::VerificationFailed(_, _))),
        "Wrong key should fail verification: {:?}",
        result
    );
}

#[test]
fn test_expired_signature_handling() {
    let bundle = test_key_bundle();
    let artifact = b"test_tool_v1";
    let issuer = "test-issuer";
    let version = "1.0.0";
    let old_timestamp = 1600000000i64; // Timestamp from 2020

    let key = bundle.get_key(issuer).expect("issuer should be trusted");

    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, artifact);
    let artifact_hash = hex::encode(hasher.finalize());

    let signature = compute_signature(artifact, issuer, version, old_timestamp, key)
        .expect("signature computation should succeed");

    let sig = ComponentSignature {
        component_id: "test_tool".to_string(),
        sha256_hash: artifact_hash,
        issuer: issuer.to_string(),
        version: version.to_string(),
        timestamp: old_timestamp,
        signature,
    };

    // This should still verify - we don't enforce timestamp validation in MVP
    // Timestamp validation can be added in future iteration
    let result = verify_component("test_tool", artifact, &sig, &bundle);
    assert!(
        result.is_ok(),
        "Old timestamp should still verify: {:?}",
        result
    );
}

#[test]
fn test_trusted_key_bundle_from_env() {
    // Clear any existing env vars
    std::env::remove_var("MCP_TRUSTED_ISSUERS");

    let bundle = TrustedKeyBundle::load_from_env();
    assert!(
        !bundle.is_trusted("anything"),
        "Empty bundle should trust nothing"
    );

    // When env vars are set, bundle should load them
    std::env::set_var("MCP_TRUSTED_ISSUERS", r#"["test"]"#);
    std::env::set_var("MCP_KEY_TEST", "0123456789abcdef");

    let bundle = TrustedKeyBundle::load_from_env();
    assert!(
        bundle.is_trusted("test"),
        "Should trust test issuer from env"
    );

    // Cleanup
    std::env::remove_var("MCP_TRUSTED_ISSUERS");
    std::env::remove_var("MCP_KEY_TEST");
}
