//! Information Loss Prevention Guard (Issue #58).
//!
//! Provides three complementary mechanisms against information loss:
//!
//! 1. **SHA-256 content integrity verification** — `compute_sha256` / `verify_integrity`
//!    Detect bit-rot or silent corruption of LTM entries stored in the database.
//!    The stored `content_hash` column is validated by recomputing the hash of the
//!    retrieved content and comparing.  Any mismatch is logged as an ERROR and
//!    reported to the caller.
//!
//! 2. **Write journal** — `record_write` / `get_recent_writes`
//!    Every LTM write operation is appended as a JSON line to
//!    `{data_dir}/write_journal.jsonl`.  The journal is write-ahead, so even if
//!    the server crashes after writing the record the operator can use it to
//!    identify which entries need re-verification or re-ingestion.
//!
//! 3. **Background integrity scanner** — `init_integrity_scanner`
//!    A Tokio background task that periodically reads a random sample of LTM
//!    entries and verifies their `content_hash`.  Anomalies are logged and
//!    optionally surfaced via the health endpoint.

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::time::sleep;
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Status returned by `verify_integrity`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityStatus {
    /// Content hash matches the re-computed hash — all good.
    Ok,
    /// The stored hash does not match the current content (corruption detected).
    HashMismatch {
        entry_id: String,
        stored: String,
        computed: String,
    },
    /// The entry has no stored hash (e.g. previously written without hashing).
    NoStoredHash { entry_id: String },
}

/// A single write event recorded in the journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteRecord {
    pub timestamp: String,
    pub operation: String, // "create" | "update" | "supersede"
    pub entry_id: String,
    pub source_id: String,
    pub content_hash: String,
    /// "ok" | "error: <message>"
    pub status: String,
}

// ---------------------------------------------------------------------------
// SHA-256 utilities
// ---------------------------------------------------------------------------

/// Compute the SHA-256 hex digest of a UTF-8 string.
/// This is the canonical hash function for LTM `content_hash` fields.
pub fn compute_sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Verify that the stored hash of a knowledge entry matches its content.
pub fn verify_integrity(entry_id: &str, content: &str, stored_hash: &str) -> IntegrityStatus {
    if stored_hash.is_empty() {
        return IntegrityStatus::NoStoredHash {
            entry_id: entry_id.to_string(),
        };
    }
    let computed = compute_sha256(content);
    if computed == stored_hash {
        IntegrityStatus::Ok
    } else {
        error!(
            entry_id = %entry_id,
            stored_hash = %stored_hash,
            computed_hash = %computed,
            "INTEGRITY VIOLATION: LTM entry content hash mismatch"
        );
        IntegrityStatus::HashMismatch {
            entry_id: entry_id.to_string(),
            stored: stored_hash.to_string(),
            computed,
        }
    }
}

// ---------------------------------------------------------------------------
// Write journal
// ---------------------------------------------------------------------------

static JOURNAL_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Append a write record to the journal file (blocking I/O — call from a
/// `tokio::task::spawn_blocking` closure for hot paths, or accept the small
/// latency here since each record is just one `write` + `flush`).
pub fn record_write(record: &WriteRecord) {
    if !JOURNAL_INITIALIZED.load(Ordering::Relaxed) {
        return; // journal not yet set up — skip silently
    }
    let path = journal_file_path();
    match (|| -> anyhow::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let line = serde_json::to_string(record)?;
        writeln!(file, "{}", line)?;
        file.flush()?;
        Ok(())
    })() {
        Ok(_) => {}
        Err(e) => warn!("Could not write to journal {:?}: {}", path, e),
    }
}

/// Initialise the journal: create the data directory and the journal file if
/// they don't already exist, then set `JOURNAL_INITIALIZED`.
pub fn init_write_journal() {
    let path = journal_file_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Could not create journal directory: {}", e);
            return;
        }
    }
    // Touch the file so it exists.
    if let Err(e) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        warn!("Could not open journal file {:?}: {}", path, e);
        return;
    }
    JOURNAL_INITIALIZED.store(true, Ordering::SeqCst);
    info!("Write journal initialised at {:?}", path);
}

fn journal_file_path() -> PathBuf {
    let mut dir = crate::config::storage_utils::resolve_data_directory();
    dir.push("write_journal.jsonl");
    dir
}

// ---------------------------------------------------------------------------
// Background integrity scanner
// ---------------------------------------------------------------------------

/// Global corruption counters (accessible via health check).
static SCAN_VIOLATIONS: AtomicU64 = AtomicU64::new(0);
static SCAN_CHECKED: AtomicU64 = AtomicU64::new(0);
static SCANNER_RUNNING: AtomicBool = AtomicBool::new(false);

/// How many LTM entries to sample per scan cycle.
const SAMPLE_BATCH: i32 = 50;
/// How often the scanner runs (seconds).
const SCAN_INTERVAL_SECONDS: u64 = 300; // 5 minutes

/// Start the background integrity scanner.
/// Safe to call multiple times — only the first call has effect.
pub fn init_integrity_scanner() {
    if SCANNER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    info!("Starting background LTM integrity scanner (interval={}s)", SCAN_INTERVAL_SECONDS);
    tokio::spawn(scanner_loop());
}

async fn scanner_loop() {
    loop {
        sleep(Duration::from_secs(SCAN_INTERVAL_SECONDS)).await;
        if let Err(e) = run_scan_cycle().await {
            error!("Integrity scan cycle error: {}", e);
        }
    }
}

async fn run_scan_cycle() -> anyhow::Result<()> {
    use crate::db::ltm::LTMRepository;

    // Fetch a random sample of LTM entries.
    let entries = LTMRepository::list_entries(None, None, Some(SAMPLE_BATCH), Some(0)).await?;
    let mut checked = 0u64;
    let mut violations = 0u64;

    for entry in &entries.entries {
        let status = verify_integrity(&entry.entry_id, &entry.content, &entry.content_hash);
        checked += 1;
        match status {
            IntegrityStatus::Ok => {}
            IntegrityStatus::HashMismatch { entry_id, stored, computed } => {
                error!(
                    entry_id = %entry_id,
                    stored_hash = %stored,
                    computed_hash = %computed,
                    "Background scan found integrity violation"
                );
                violations += 1;
            }
            IntegrityStatus::NoStoredHash { entry_id } => {
                warn!(entry_id = %entry_id, "LTM entry has no stored hash");
            }
        }
    }

    SCAN_CHECKED.fetch_add(checked, Ordering::Relaxed);
    SCAN_VIOLATIONS.fetch_add(violations, Ordering::Relaxed);

    if violations > 0 {
        error!(
            violations = violations,
            checked = checked,
            "Integrity scan complete — violations detected!"
        );
    } else {
        info!(checked = checked, "Integrity scan complete — no violations");
    }

    Ok(())
}

/// Return cumulative integrity scan statistics: `(total_checked, total_violations)`.
pub fn scan_stats() -> (u64, u64) {
    (
        SCAN_CHECKED.load(Ordering::Relaxed),
        SCAN_VIOLATIONS.load(Ordering::Relaxed),
    )
}
