//! Vector Space Collapse Prevention (Issue #59).
//!
//! Stores a **signature** (`model_name + dimension`) for each Qdrant collection
//! so that the system can detect when the embedding model is changed between
//! restarts — a situation that causes semantically meaningless nearest-neighbour
//! results ("vector space collapse").
//!
//! # Behaviour
//!
//! | Stored signature vs. current config | Result |
//! |--------------------------------------|--------|
//! | Same model **and** same dimension    | OK — proceed normally |
//! | Different dimension                  | **Error** — write/read aborted; your index is incompatible |
//! | Same dimension, different model      | **Warning** — results may be degraded; proceed but log |
//! | No stored signature                  | **Create** new signature and continue |
//!
//! The signature is persisted as JSON in `{data_dir}/vector_signatures.json`.
//! This file is written once at collection creation time and checked every
//! startup; it is human-readable and safe to delete if you intentionally want
//! to rebuild the index from scratch.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Persisted model/dimension binding for a single Qdrant collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSignature {
    /// Embedding model name, e.g. `"nomic-embed-text"`.
    pub model: String,
    /// Vector dimension expected by this collection.
    pub dimension: usize,
    /// Collection name (redundant, kept for human readability).
    pub collection_name: String,
    /// ISO-8601 timestamp when this signature was first written.
    pub created_at: String,
}

/// Loaded guard state — the expected dimension / model for reads and writes.
#[derive(Debug, Clone)]
pub struct VectorGuard {
    pub collection_name: String,
    pub expected_model: String,
    pub expected_dimension: usize,
}

// ---------------------------------------------------------------------------
// Global guard
// ---------------------------------------------------------------------------

static GUARD: OnceLock<VectorGuard> = OnceLock::new();

/// Initialise the guard from the current configuration.
///
/// 1. Reads `{data_dir}/vector_signatures.json`.
/// 2. If no entry exists for the configured collection, **creates** one.
/// 3. If an entry exists:
///    - Dimension mismatch → returns `Err` (abort startup).
///    - Model mismatch (same dimension) → logs `WARN` and continues.
///    - Exact match → silent OK.
pub fn init() -> Result<()> {
    let cfg = crate::config::get();
    let collection_name = cfg.qdrant.collection_name.clone();
    let current_model = cfg.embedding.model.clone();
    let current_dimension = cfg.qdrant.vector_dimension;

    // Validate that configured dimension is non-zero
    if current_dimension == 0 {
        bail!("qdrant.vector_dimension must be > 0; check config");
    }

    let sig_path = signature_file_path();

    match load_signatures(&sig_path) {
        Ok(mut sigs) => {
            if let Some(stored) = sigs.get(&collection_name) {
                // --- Compare stored vs. current ---
                if stored.dimension != current_dimension {
                    error!(
                        collection = %collection_name,
                        stored_model = %stored.model,
                        stored_dimension = stored.dimension,
                        current_model = %current_model,
                        current_dimension,
                        "VECTOR SPACE COLLAPSE DETECTED: dimension mismatch! \
                         The collection was built with a different embedding model. \
                         Drop the Qdrant collection and re-index to recover."
                    );
                    bail!(
                        "Vector dimension mismatch for collection '{}': stored={}, current={}. \
                         The collection must be re-indexed before the server can start.",
                        collection_name,
                        stored.dimension,
                        current_dimension
                    );
                }
                if stored.model != current_model {
                    warn!(
                        collection = %collection_name,
                        stored_model = %stored.model,
                        current_model = %current_model,
                        dimension = current_dimension,
                        "Embedding model changed: vectors in '{}' were created with '{}' but \
                         the current model is '{}'. Both produce {}-d vectors so insert/search \
                         will not fail, but cross-model results will be semantically degraded. \
                         Consider re-indexing the collection.",
                        collection_name, stored.model, current_model, current_dimension
                    );
                } else {
                    info!(
                        collection = %collection_name,
                        model = %current_model,
                        dimension = current_dimension,
                        "Vector space signature OK"
                    );
                }
            } else {
                // No entry yet — first run; create and persist the signature.
                let sig = VectorSignature {
                    model: current_model.clone(),
                    dimension: current_dimension,
                    collection_name: collection_name.clone(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                sigs.insert(collection_name.clone(), sig);
                if let Err(e) = save_signatures(&sig_path, &sigs) {
                    // Non-fatal: warn rather than abort — the system can still run.
                    warn!(
                        "Could not persist vector signature to {:?}: {}",
                        sig_path, e
                    );
                } else {
                    info!(
                        collection = %collection_name,
                        model = %current_model,
                        dimension = current_dimension,
                        "Vector space signature created"
                    );
                }
            }
        }
        Err(e) => {
            // Could not read the file (first run or I/O error) — create fresh.
            warn!(
                "Could not load vector signatures from {:?}: {}. \
                 Creating a new signature file.",
                sig_path, e
            );
            let mut sigs = HashMap::new();
            sigs.insert(
                collection_name.clone(),
                VectorSignature {
                    model: current_model.clone(),
                    dimension: current_dimension,
                    collection_name: collection_name.clone(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                },
            );
            if let Err(e2) = save_signatures(&sig_path, &sigs) {
                warn!("Could not write new vector signature file: {}", e2);
            }
        }
    }

    let _ = GUARD.set(VectorGuard {
        collection_name,
        expected_model: current_model,
        expected_dimension: current_dimension,
    });

    Ok(())
}

/// Return the active `VectorGuard`, if `init()` has been called.
pub fn get() -> Option<&'static VectorGuard> {
    GUARD.get()
}

// ---------------------------------------------------------------------------
// Validation helpers (called by QdrantClient)
// ---------------------------------------------------------------------------

/// Validate the dimension of a batch of vectors before writing.
///
/// Returns `Err` if any vector has the wrong size so the caller receives a
/// clear error instead of a cryptic Qdrant gRPC failure.
pub fn validate_write(vectors: &[Vec<f32>]) -> Result<()> {
    let guard = match GUARD.get() {
        Some(g) => g,
        None => return Ok(()), // guard not yet initialised — skip check
    };
    for (i, v) in vectors.iter().enumerate() {
        if v.len() != guard.expected_dimension {
            bail!(
                "Vector {} has dimension {} but collection '{}' expects {}. \
                 Embedding model mismatch? Check your config.",
                i,
                v.len(),
                guard.collection_name,
                guard.expected_dimension
            );
        }
    }
    Ok(())
}

/// Validate the dimension of a query vector before a similarity search.
pub fn validate_read(query: &[f32]) -> Result<()> {
    let guard = match GUARD.get() {
        Some(g) => g,
        None => return Ok(()),
    };
    if query.len() != guard.expected_dimension {
        bail!(
            "Query vector has dimension {} but collection '{}' expects {}. \
             Hardware-routed model may have changed the embedding dimension.",
            query.len(),
            guard.collection_name,
            guard.expected_dimension
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Signature persistence helpers
// ---------------------------------------------------------------------------

fn signature_file_path() -> PathBuf {
    let mut dir = crate::config::storage_utils::resolve_data_directory();
    dir.push("vector_signatures.json");
    dir
}

fn load_signatures(path: &PathBuf) -> Result<HashMap<String, VectorSignature>> {
    let content = std::fs::read_to_string(path)?;
    let map: HashMap<String, VectorSignature> = serde_json::from_str(&content)?;
    Ok(map)
}

fn save_signatures(path: &PathBuf, sigs: &HashMap<String, VectorSignature>) -> Result<()> {
    // Ensure the parent directory exists.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(sigs)?;
    std::fs::write(path, json)?;
    Ok(())
}
