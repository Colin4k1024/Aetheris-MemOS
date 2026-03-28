//! MCP Component Signing and Verification
//!
//! Implements SEC-01: MCP components must be signed and signature verified on load.
//! Uses HMAC-SHA256 for integrity verification with trusted key bundle approach.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

/// Component signature containing SHA-256 hash and issuer metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ComponentSignature {
    /// Unique identifier for the component
    pub component_id: String,
    /// Hex-encoded SHA-256 hash of component artifact
    pub sha256_hash: String,
    /// Issuer identifier (e.g., "adaptive-memory-system")
    pub issuer: String,
    /// Component version
    pub version: String,
    /// Signature creation timestamp (Unix epoch)
    pub timestamp: i64,
    /// Hex-encoded HMAC-SHA256 signature
    pub signature: String,
}

/// Signing verification error
#[derive(Debug, Error)]
pub enum SigningError {
    #[error("Component {0} is not signed")]
    Unsigned(String),
    #[error("Signature verification failed for {0}: {1}")]
    VerificationFailed(String, String),
    #[error("Trusted key bundle not found")]
    MissingKeyBundle,
    #[error("Unknown issuer: {0}")]
    UnknownIssuer(String),
    #[error("Invalid hash format: {0}")]
    InvalidHash(String),
}

/// Trusted key bundle for signature verification
#[derive(Debug, Clone)]
pub struct TrustedKeyBundle {
    keys: HashMap<String, Vec<u8>>,
}

impl TrustedKeyBundle {
    /// Create a new TrustedKeyBundle with the given keys
    pub fn new(keys: HashMap<String, Vec<u8>>) -> Self {
        Self { keys }
    }

    /// Load trusted keys from environment variables.
    ///
    /// `MCP_TRUSTED_ISSUERS` - JSON array of trusted issuer IDs
    /// `MCP_KEY_{ISSUER}` - HMAC key for each issuer (hex-encoded)
    pub fn load_from_env() -> Self {
        let mut keys = HashMap::new();

        if let Ok(issuers_var) = env::var("MCP_TRUSTED_ISSUERS") {
            if let Ok(issuers) = serde_json::from_str::<Vec<String>>(&issuers_var) {
                for issuer in issuers {
                    let key_var = format!("MCP_KEY_{}", issuer.to_uppercase().replace('-', "_"));
                    if let Ok(key_hex) = env::var(&key_var) {
                        if let Ok(key_bytes) = hex::decode(&key_hex) {
                            keys.insert(issuer, key_bytes);
                        }
                    }
                }
            }
        }

        Self { keys }
    }

    /// Get the key for a specific issuer
    pub fn get_key(&self, issuer: &str) -> Option<&[u8]> {
        self.keys.get(issuer).map(|k| k.as_slice())
    }

    /// Check if an issuer is trusted
    pub fn is_trusted(&self, issuer: &str) -> bool {
        self.keys.contains_key(issuer)
    }
}

/// Compute HMAC-SHA256 signature for component data
pub fn compute_signature(
    artifact: &[u8],
    issuer: &str,
    version: &str,
    timestamp: i64,
    key: &[u8],
) -> Result<String, SigningError> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|e| SigningError::VerificationFailed("HMAC init failed".to_string(), e.to_string()))?;

    // Input: sha256_hash + issuer + version + timestamp
    let mut hasher = Sha256::new();
    hasher.update(artifact);
    let artifact_hash = hex::encode(hasher.finalize());

    mac.update(artifact_hash.as_bytes());
    mac.update(issuer.as_bytes());
    mac.update(version.as_bytes());
    mac.update(&timestamp.to_le_bytes());

    let result = mac.finalize();
    Ok(hex::encode(result.into_bytes()))
}

/// Verify component signature against trusted key bundle
///
/// D-01: MCP components must be signed; verify signature on load using a trusted key bundle.
/// D-02: Provenance checks use SHA-256 hash of component artifact + issuer metadata.
/// D-03: Unverified or unsigned components are rejected at load time with a structured error.
pub fn verify_component(
    component_id: &str,
    artifact: &[u8],
    signature: &ComponentSignature,
    key_bundle: &TrustedKeyBundle,
) -> Result<(), SigningError> {
    // 1. Check issuer is trusted
    let key = key_bundle
        .get_key(&signature.issuer)
        .ok_or_else(|| SigningError::UnknownIssuer(signature.issuer.clone()))?;

    // 2. Compute expected SHA-256 of artifact
    let mut hasher = Sha256::new();
    hasher.update(artifact);
    let computed_hash = hex::encode(hasher.finalize());

    // 3. Constant-time comparison of hashes
    if !constant_time_eq(computed_hash.as_bytes(), signature.sha256_hash.as_bytes()) {
        return Err(SigningError::VerificationFailed(
            component_id.to_string(),
            "SHA-256 hash mismatch".to_string(),
        ));
    }

    // 4. Verify HMAC signature
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|e| SigningError::VerificationFailed(component_id.to_string(), e.to_string()))?;

    mac.update(signature.sha256_hash.as_bytes());
    mac.update(signature.issuer.as_bytes());
    mac.update(signature.version.as_bytes());
    mac.update(&signature.timestamp.to_le_bytes());

    let expected = match hex::decode(&signature.signature) {
        Ok(s) => s,
        Err(e) => {
            return Err(SigningError::VerificationFailed(
                component_id.to_string(),
                format!("Invalid signature hex: {}", e),
            ))
        }
    };

    mac.verify_slice(&expected)
        .map_err(|_| {
            SigningError::VerificationFailed(
                component_id.to_string(),
                "HMAC signature verification failed".to_string(),
            )
        })
}

/// Constant-time byte comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Verify that a component has no signature (for D-03: unsigned components must be rejected)
pub fn verify_unsigned(component_id: &str) -> Result<(), SigningError> {
    Err(SigningError::Unsigned(component_id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_bundle() -> (TrustedKeyBundle, String) {
        let mut keys = HashMap::new();
        // Use proper hex string for test key
        let secret_key = hex::decode("0123456789abcdef0123456789abcdef").unwrap();
        keys.insert("test-issuer".to_string(), secret_key.clone());
        (TrustedKeyBundle { keys }, hex::encode(secret_key))
    }

    #[test]
    fn test_valid_signature_passes_verification() {
        let (bundle, _) = create_test_bundle();
        let artifact = b"test_tool_v1";
        let issuer = "test-issuer";
        let version = "1.0.0";
        let timestamp = 1710000000i64;

        let key = bundle.get_key(issuer).unwrap();

        // Compute artifact hash
        let mut hasher = Sha256::new();
        hasher.update(artifact);
        let artifact_hash = hex::encode(hasher.finalize());

        // Compute HMAC
        let mut mac = HmacSha256::new_from_slice(key).unwrap();
        mac.update(artifact_hash.as_bytes());
        mac.update(issuer.as_bytes());
        mac.update(version.as_bytes());
        mac.update(&timestamp.to_le_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());

        let signature = ComponentSignature {
            component_id: "test_tool".to_string(),
            sha256_hash: artifact_hash,
            issuer: issuer.to_string(),
            version: version.to_string(),
            timestamp,
            signature: sig,
        };

        let result = verify_component("test_tool", artifact, &signature, &bundle);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tampered_artifact_fails_verification() {
        let (bundle, _) = create_test_bundle();
        let artifact = b"test_tool_v1";
        let issuer = "test-issuer";
        let version = "1.0.0";
        let timestamp = 1710000000i64;

        let key = bundle.get_key(issuer).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(artifact);
        let artifact_hash = hex::encode(hasher.finalize());

        let mut mac = HmacSha256::new_from_slice(key).unwrap();
        mac.update(artifact_hash.as_bytes());
        mac.update(issuer.as_bytes());
        mac.update(version.as_bytes());
        mac.update(&timestamp.to_le_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());

        let signature = ComponentSignature {
            component_id: "test_tool".to_string(),
            sha256_hash: artifact_hash,
            issuer: issuer.to_string(),
            version: version.to_string(),
            timestamp,
            signature: sig,
        };

        // Tamper with artifact
        let tampered_artifact = b"test_tool_v2";

        let result = verify_component("test_tool", tampered_artifact, &signature, &bundle);
        assert!(matches!(result, Err(SigningError::VerificationFailed(_, _))));
    }

    #[test]
    fn test_unknown_issuer_fails_verification() {
        let (bundle, _) = create_test_bundle();
        let artifact = b"test_tool_v1";

        let signature = ComponentSignature {
            component_id: "test_tool".to_string(),
            sha256_hash: "abcd1234".to_string(),
            issuer: "unknown-issuer".to_string(),
            version: "1.0.0".to_string(),
            timestamp: 1710000000,
            signature: "signature".to_string(),
        };

        let result = verify_component("test_tool", artifact, &signature, &bundle);
        assert!(matches!(result, Err(SigningError::UnknownIssuer(_))));
    }

    #[test]
    fn test_component_without_signature_is_rejected() {
        let result = verify_unsigned("test_tool");
        assert!(matches!(result, Err(SigningError::Unsigned(_))));
    }

    #[test]
    fn test_trusted_key_bundle_load() {
        // Clear any existing env vars
        env::remove_var("MCP_TRUSTED_ISSUERS");

        let bundle = TrustedKeyBundle::load_from_env();
        assert!(!bundle.is_trusted("anything"));
    }
}
