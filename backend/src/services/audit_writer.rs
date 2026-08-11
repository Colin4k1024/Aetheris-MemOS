//! Asynchronous, **lossless** audit writer (P1 子项 b / backlog B-1).
//!
//! Governance/audit callbacks enqueue [`AuditEvent`]s via [`record_audit`] on the
//! request hot path — a non-blocking, fire-and-forget enqueue. A single background
//! task batches them within a short time window and persists them to
//! `memory_audit_events` via [`crate::db::audit::insert_event`].
//!
//! # Durability model (disk spill)
//!
//! Audit is a compliance log, so it must not lose events. The queue itself stays
//! bounded (no request-path back-pressure), but when an event cannot be enqueued or
//! cannot be persisted it is **spilled to a local file** instead of being dropped:
//!
//! | Situation                              | Behaviour |
//! |----------------------------------------|-----------|
//! | Enqueue succeeds                       | Batched + inserted by the worker (normal path) |
//! | Queue full / worker gone               | Event appended to `{data_dir}/audit_spill.jsonl` |
//! | INSERT fails (DB down / slow)          | The batch's events appended to the spill file |
//! | Spill write fails (disk full / IO err) | **Last resort:** dropped + counted + `error!` + metric |
//! | Spill file over `AUDIT_SPILL_MAX_BYTES`| **Last resort:** dropped (protects the disk) |
//!
//! On startup the worker **replays** the spill file(s) idempotently
//! (`ON CONFLICT (event_id) DO NOTHING`) and removes each once fully drained, so
//! events buffered during a DB outage are persisted on the next boot. `event_id` is
//! a globally unique ULID, so replaying the same event twice can never create a
//! duplicate audit row.
//!
//! ## Known limitation
//!
//! Replay is triggered **at startup only**. If the DB recovers while the process
//! keeps running, spilled events stay durable on disk but are not re-flushed until
//! the next restart. This preserves the "no loss" invariant (the compliance goal);
//! it does not minimise persistence latency during an in-process recovery. A
//! periodic in-process drain is a possible future addition — [`replay_one_file`] is
//! written to be reusable for it.
//!
//! ## Backend scope
//!
//! The worker (and therefore all spill/replay) runs on a **PostgreSQL** backend
//! only, gated by `crate::db::is_postgres()` at the startup site. On the SQLite dev
//! backend there is no audit table to replay into, so the writer is not started and
//! [`record_audit`] counts events as dropped rather than spilling them into a file
//! that could never be drained (which would only leak disk). See the report / ADR
//! for why SQLite audit persistence is intentionally out of scope.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tokio::sync::mpsc;
use tracing::{error, info, warn};
use ulid::Ulid;

use crate::db::audit::AuditEvent;

/// Global sender — set once at startup via [`init_audit_writer`].
static AUDIT_QUEUE: OnceLock<mpsc::Sender<AuditEvent>> = OnceLock::new();

/// Events dropped as a **last resort** (spill write failed or spill cap exceeded, or
/// enqueued before init on a backend without a writer). This is the counter that a
/// compliance alert should watch: a non-zero value means audit loss actually happened.
static DROPPED: AtomicU64 = AtomicU64::new(0);

/// Events written to the spill file (queue full or INSERT failed). Not lost — pending
/// replay on the next startup.
static SPILLED: AtomicU64 = AtomicU64::new(0);

/// Events successfully replayed from spill files into the database.
static REPLAYED: AtomicU64 = AtomicU64::new(0);

/// Spill lines skipped during replay because they could not be parsed — a truncated
/// tail from a crash mid-write, or corruption. Surfaced separately from [`DROPPED`]
/// so operators can distinguish "we chose to drop" from "a partial write was lost".
static TRUNCATED_SKIPPED: AtomicU64 = AtomicU64::new(0);

/// Serialises spill-file appends and rotation. Held only around synchronous file IO
/// — **never** across an `.await` — so the async worker can take it safely.
static SPILL_LOCK: Mutex<()> = Mutex::new(());

/// Queue capacity before enqueues start spilling to disk (audit is lossless via spill,
/// so we favour a generous buffer over back-pressure on the request path).
const QUEUE_CAPACITY: usize = 8192;

/// Maximum number of events flushed to the database in one batch.
const MAX_BATCH_SIZE: usize = 256;

/// Time window to collect events before flushing (milliseconds).
const FLUSH_WINDOW_MS: u64 = 50;

/// Base name of the active spill file inside the resolved data directory.
const SPILL_FILE_NAME: &str = "audit_spill.jsonl";

/// Suffix marking a rotated spill file that is being (or waiting to be) replayed.
const REPLAYING_SUFFIX: &str = ".replaying.jsonl";

/// Default spill-file size cap (bytes) before the last-resort drop kicks in. Protects
/// the disk when the DB is unavailable for a long time. Override with
/// `AUDIT_SPILL_MAX_BYTES`.
const DEFAULT_SPILL_MAX_BYTES: u64 = 100 * 1024 * 1024;

/// Outcome of a single spill-file append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpillOutcome {
    /// Event was durably appended to the spill file.
    Spilled,
    /// Event could not be spilled (IO error or cap exceeded) — last resort.
    Dropped,
}

/// Outcome of routing one event through [`enqueue_or_spill`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnqueueOutcome {
    /// Event was accepted onto the in-memory queue (normal path).
    Enqueued,
    /// Queue was full/closed but the event was spilled to disk.
    Spilled,
    /// Queue was full/closed and the spill also failed — last resort.
    Dropped,
}

/// Start the background audit writer. Idempotent — subsequent calls are no-ops.
///
/// Must be called after the database pool is initialised, and only on a PostgreSQL
/// backend (gate the call on `crate::db::is_postgres()`); the worker replays any
/// spilled events on startup before it begins draining the queue.
pub fn init_audit_writer() {
    if AUDIT_QUEUE.get().is_some() {
        return;
    }

    let (tx, rx) = mpsc::channel::<AuditEvent>(QUEUE_CAPACITY);
    if AUDIT_QUEUE.set(tx).is_err() {
        // Lost an init race with another caller; that caller owns the worker.
        return;
    }

    tokio::spawn(audit_writer_worker(rx));
    info!(
        "Audit writer started (capacity={}, batch={}, window={}ms, spill_cap={}B)",
        QUEUE_CAPACITY,
        MAX_BATCH_SIZE,
        FLUSH_WINDOW_MS,
        spill_max_bytes()
    );
}

/// Enqueue an audit event without blocking. If the queue is full or the worker is
/// gone, the event is **spilled to disk** for replay rather than dropped, keeping the
/// request hot path free of audit-persistence latency while remaining lossless. Only
/// a failed spill (disk full / cap exceeded) or a missing writer drops + counts.
pub fn record_audit(event: AuditEvent) {
    let Some(sender) = AUDIT_QUEUE.get() else {
        // Writer not started: SQLite dev backend, or called before init. There is no
        // worker to drain a spill file here, so spilling would only leak disk — count
        // as a last-resort drop and warn sparsely.
        note_dropped_uninit();
        return;
    };

    // `None` = resolve the spill target lazily, so the common (queue has room) path
    // never touches the filesystem — only an actual spill pays for path resolution.
    match enqueue_or_spill(sender, event, None) {
        EnqueueOutcome::Enqueued => {}
        EnqueueOutcome::Spilled => note_spilled(1),
        EnqueueOutcome::Dropped => note_dropped_last_resort(1),
    }
}

/// Route one event: try the queue first, spill on failure. Split out from
/// [`record_audit`] so the "queue full → spill" path is unit-testable with a local
/// channel and a temp file. `spill_override` injects `(path, cap)` in tests; in
/// production it is `None`, and the spill target is resolved **lazily** inside the
/// failure branch so the happy path never hits the filesystem.
fn enqueue_or_spill(
    sender: &mpsc::Sender<AuditEvent>,
    event: AuditEvent,
    spill_override: Option<(&Path, u64)>,
) -> EnqueueOutcome {
    match sender.try_send(event) {
        Ok(()) => EnqueueOutcome::Enqueued,
        Err(e) => {
            // Recover the event from Full/Closed and spill it instead of dropping.
            let ev = e.into_inner();
            let outcome = match spill_override {
                Some((path, max_bytes)) => spill_event_to(path, max_bytes, &ev),
                None => spill_event(&ev),
            };
            match outcome {
                SpillOutcome::Spilled => EnqueueOutcome::Spilled,
                SpillOutcome::Dropped => EnqueueOutcome::Dropped,
            }
        }
    }
}

/// Total audit events dropped as a last resort (spill failed / cap hit / no writer).
pub fn dropped_count() -> u64 {
    DROPPED.load(Ordering::Relaxed)
}

/// Total audit events spilled to disk (pending replay).
pub fn spilled_count() -> u64 {
    SPILLED.load(Ordering::Relaxed)
}

/// Total audit events replayed from spill files into the database.
pub fn replayed_count() -> u64 {
    REPLAYED.load(Ordering::Relaxed)
}

/// Total spill lines skipped during replay (truncated tail / corruption).
pub fn truncated_skipped_count() -> u64 {
    TRUNCATED_SKIPPED.load(Ordering::Relaxed)
}

/// Approximate number of in-flight (unconsumed) events, or `None` before init.
pub fn queue_depth() -> Option<usize> {
    AUDIT_QUEUE.get().map(|tx| QUEUE_CAPACITY - tx.capacity())
}

// ---------------------------------------------------------------------------
// Counter + metric bridges
// ---------------------------------------------------------------------------

fn note_spilled(n: u64) {
    if n == 0 {
        return;
    }
    SPILLED.fetch_add(n, Ordering::Relaxed);
    let exporter = crate::services::prometheus_exporter::get_exporter();
    for _ in 0..n {
        exporter.inc_audit_spilled();
    }
}

fn note_replayed(n: u64) {
    if n == 0 {
        return;
    }
    REPLAYED.fetch_add(n, Ordering::Relaxed);
    crate::services::prometheus_exporter::get_exporter().inc_audit_replayed_by(n as f64);
}

fn note_truncated_skipped(n: u64) {
    if n == 0 {
        return;
    }
    TRUNCATED_SKIPPED.fetch_add(n, Ordering::Relaxed);
    crate::services::prometheus_exporter::get_exporter().inc_audit_truncated_skipped_by(n as f64);
}

fn note_dropped_last_resort(n: u64) {
    if n == 0 {
        return;
    }
    let dropped = DROPPED.fetch_add(n, Ordering::Relaxed) + n;
    let exporter = crate::services::prometheus_exporter::get_exporter();
    for _ in 0..n {
        exporter.inc_audit_dropped();
    }
    // A last-resort drop is the compliance-alert signal and is expected to be rare,
    // so log every occurrence at ERROR. Each call drops at most one batch, so this
    // cannot flood per-event; the metric `audit_dropped_total` is the durable signal.
    error!(
        "Audit LAST RESORT drop: spill write failed or cap exceeded; {} audit events lost so far",
        dropped
    );
}

fn note_dropped_uninit() {
    let dropped = DROPPED.fetch_add(1, Ordering::Relaxed) + 1;
    crate::services::prometheus_exporter::get_exporter().inc_audit_dropped();
    if dropped % 100 == 1 {
        warn!(
            "Audit writer not initialised (non-PG backend or pre-init call); dropped {} audit events so far",
            dropped
        );
    }
}

// ---------------------------------------------------------------------------
// Background worker
// ---------------------------------------------------------------------------

/// Background worker: replays spilled events on startup, then drains the channel into
/// time-windowed batches and flushes each.
async fn audit_writer_worker(mut rx: mpsc::Receiver<AuditEvent>) {
    // Persist anything buffered on disk during a previous outage before taking new
    // work. Best-effort: a still-unavailable DB leaves the spill files for next boot.
    replay_spilled_on_startup().await;

    loop {
        // Block until the first event of the next batch arrives.
        let first = match rx.recv().await {
            Some(ev) => ev,
            None => {
                info!("Audit queue closed, writer exiting");
                return;
            }
        };

        // Collect additional events within the flush window (bounded by batch size).
        let mut batch = vec![first];
        let deadline = tokio::time::sleep(Duration::from_millis(FLUSH_WINDOW_MS));
        tokio::pin!(deadline);

        loop {
            if batch.len() >= MAX_BATCH_SIZE {
                break;
            }
            tokio::select! {
                biased;
                ev = rx.recv() => match ev {
                    Some(ev) => batch.push(ev),
                    None => break,
                },
                _ = &mut deadline => break,
            }
        }

        flush_batch(batch).await;
    }
}

/// Persist a batch. On any per-event INSERT failure the event is **spilled** (not
/// dropped) so a DB outage never loses audit records; startup replay retries them.
async fn flush_batch(batch: Vec<AuditEvent>) {
    use crate::db::{DatabasePool, DATABASE_POOL};

    let pool = match DATABASE_POOL.get() {
        Some(DatabasePool::Postgres(p)) => p,
        _ => {
            // The worker only runs under PG, so this is defensive. Spill rather than
            // drop, so a transient pool gap does not lose events.
            let mut spilled = 0u64;
            let mut dropped = 0u64;
            for event in &batch {
                match spill_event(event) {
                    SpillOutcome::Spilled => spilled += 1,
                    SpillOutcome::Dropped => dropped += 1,
                }
            }
            note_spilled(spilled);
            note_dropped_last_resort(dropped);
            error!(
                "Audit writer: no PostgreSQL pool available; spilled {} / dropped {} audit events",
                spilled, dropped
            );
            return;
        }
    };

    let mut spilled = 0u64;
    let mut dropped = 0u64;
    for event in &batch {
        if let Err(e) = crate::db::audit::insert_event(pool, event).await {
            error!(
                "Audit writer: failed to persist event {}: {}; spilling to disk",
                event.event_id, e
            );
            match spill_event(event) {
                SpillOutcome::Spilled => spilled += 1,
                SpillOutcome::Dropped => dropped += 1,
            }
        }
    }
    note_spilled(spilled);
    note_dropped_last_resort(dropped);
}

// ---------------------------------------------------------------------------
// Spill file: paths, append, size cap
// ---------------------------------------------------------------------------

/// Resolved path of the active spill file. Uses the same cross-platform data
/// directory as the vector-guard signature file.
fn spill_file_path() -> PathBuf {
    let mut dir = crate::config::storage_utils::resolve_data_directory();
    dir.push(SPILL_FILE_NAME);
    dir
}

/// Spill-file size cap in bytes (`AUDIT_SPILL_MAX_BYTES`, else the default).
fn spill_max_bytes() -> u64 {
    std::env::var("AUDIT_SPILL_MAX_BYTES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(DEFAULT_SPILL_MAX_BYTES)
}

/// Production spill: append `event` to the active spill file with the configured cap.
fn spill_event(event: &AuditEvent) -> SpillOutcome {
    spill_event_to(&spill_file_path(), spill_max_bytes(), event)
}

/// Append one event as a JSON line to `path`, enforcing `max_bytes`. Pure with
/// respect to global state (path + cap are injected) so it is unit-testable against a
/// temp directory. Serialisation happens before the lock to keep the critical section
/// to the file IO only.
fn spill_event_to(path: &Path, max_bytes: u64, event: &AuditEvent) -> SpillOutcome {
    let line = match serde_json::to_string(event) {
        Ok(s) => s,
        Err(e) => {
            error!(
                "Audit spill: failed to serialise event {}: {}",
                event.event_id, e
            );
            return SpillOutcome::Dropped;
        }
    };

    // Guard the check-then-append so the size cap and the write are atomic. Recover
    // from a poisoned lock rather than panicking — this is the last line of defence.
    let _guard = SPILL_LOCK.lock().unwrap_or_else(|p| p.into_inner());

    // Size cap: refuse to grow an unbounded spill file when the DB is long gone.
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() >= max_bytes {
            return SpillOutcome::Dropped;
        }
    }

    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            error!(
                "Audit spill: cannot create data directory {:?}: {}",
                parent, e
            );
            return SpillOutcome::Dropped;
        }
    }

    // Append one line + newline, then flush. A crash mid-write truncates only this
    // tail line, which replay skips (see `parse_spill_content`).
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut f) => match writeln!(f, "{}", line).and_then(|_| f.flush()) {
            Ok(()) => SpillOutcome::Spilled,
            Err(e) => {
                error!("Audit spill: write to {:?} failed: {}", path, e);
                SpillOutcome::Dropped
            }
        },
        Err(e) => {
            error!("Audit spill: open {:?} failed: {}", path, e);
            SpillOutcome::Dropped
        }
    }
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

/// Parsed contents of a spill file: fully-decoded events plus the count of lines that
/// could not be parsed (truncated tail from a crash, or corruption).
struct ParsedSpill {
    events: Vec<AuditEvent>,
    skipped: u64,
}

/// Parse spill-file contents line by line. Unparseable lines are **skipped, not
/// fatal** — a process killed mid-append leaves a partial final line. Pure and
/// database-free so truncation handling is directly unit-testable.
fn parse_spill_content(content: &str) -> ParsedSpill {
    let mut events = Vec::new();
    let mut skipped = 0u64;
    for (idx, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<AuditEvent>(line) {
            Ok(ev) => events.push(ev),
            Err(e) => {
                skipped += 1;
                warn!(
                    "Audit replay: skipping unparseable spill line {} (truncated tail or corruption): {}",
                    idx + 1,
                    e
                );
            }
        }
    }
    ParsedSpill { events, skipped }
}

/// Data-directory used for rotation/listing (kept as a seam for tests).
fn spill_dir() -> PathBuf {
    crate::config::storage_utils::resolve_data_directory()
}

/// Rotate the active spill file to a uniquely-named `*.replaying.jsonl` snapshot so
/// new spills during replay go to a fresh active file and are never lost. Atomic
/// rename under the spill lock; a no-op when there is nothing to replay.
fn rotate_active_spill_in(dir: &Path) {
    let active = dir.join(SPILL_FILE_NAME);
    let _guard = SPILL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    match std::fs::metadata(&active) {
        Ok(m) if m.len() > 0 => {
            let rotated = dir.join(format!("audit_spill.{}{}", Ulid::new(), REPLAYING_SUFFIX));
            if let Err(e) = std::fs::rename(&active, &rotated) {
                error!("Audit replay: could not rotate {:?}: {}", active, e);
            } else {
                info!("Audit replay: rotated spill file to {:?}", rotated);
            }
        }
        _ => {} // no active spill, or empty
    }
}

/// List `*.replaying.jsonl` snapshots in `dir` (rotated this boot, plus any left by a
/// crashed prior replay). Returns an empty vec on any IO error.
fn list_replaying_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("audit_spill.") && name.ends_with(REPLAYING_SUFFIX) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Replay all spill files at startup: rotate the active file, then drain every
/// `*.replaying.jsonl` snapshot into the database idempotently.
async fn replay_spilled_on_startup() {
    let dir = spill_dir();
    rotate_active_spill_in(&dir);
    for file in list_replaying_files_in(&dir) {
        replay_one_file(&file).await;
    }
}

/// Replay one rotated spill file into the database. Each event is inserted with
/// `ON CONFLICT (event_id) DO NOTHING`, so replaying an event that had actually
/// committed (or re-running an interrupted replay) never duplicates a row. On full
/// success the file is removed; on any INSERT failure it is left in place for the
/// next startup (already-inserted events dedupe on retry).
async fn replay_one_file(path: &Path) {
    use crate::db::{DatabasePool, DATABASE_POOL};

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            error!("Audit replay: cannot read {:?}: {}", path, e);
            return;
        }
    };

    let pool = match DATABASE_POOL.get() {
        Some(DatabasePool::Postgres(p)) => p,
        _ => {
            warn!(
                "Audit replay: no PostgreSQL pool; leaving {:?} for a later startup",
                path
            );
            return;
        }
    };

    let ParsedSpill { events, skipped } = parse_spill_content(&content);
    note_truncated_skipped(skipped);

    let mut replayed = 0u64;
    let mut all_ok = true;
    for event in &events {
        match crate::db::audit::insert_event_idempotent(pool, event).await {
            Ok(()) => replayed += 1,
            Err(e) => {
                error!(
                    "Audit replay: insert failed for {}: {}; retaining {:?} for retry",
                    event.event_id, e, path
                );
                all_ok = false;
                break;
            }
        }
    }
    note_replayed(replayed);

    if all_ok {
        match std::fs::remove_file(path) {
            Ok(()) => info!(
                "Audit replay: drained {:?} ({} events, {} skipped)",
                path, replayed, skipped
            ),
            Err(e) => error!(
                "Audit replay: drained {:?} but could not remove it: {}",
                path, e
            ),
        }
    } else {
        warn!(
            "Audit replay: {:?} only partially replayed ({} of {}); left for next startup",
            path,
            replayed,
            events.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique temp directory for a test, so spill files never touch the real data
    /// directory. Removed automatically via [`TempDir`]'s Drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let mut p = std::env::temp_dir();
            p.push(format!("aetheris_audit_spill_test_{}_{}", tag, Ulid::new()));
            std::fs::create_dir_all(&p).expect("create temp dir");
            TempDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
        fn file(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn record_audit_before_init_drops_and_counts() {
        // No test initialises the writer, so record_audit takes the uninitialised
        // branch and drops the event. Use a before/after delta so the assertion is
        // robust regardless of test execution order within the binary.
        let before = dropped_count();
        record_audit(AuditEvent::new("memory.write", "ltm_entry"));
        assert!(
            dropped_count() >= before + 1,
            "uninitialised record_audit must drop and count the event"
        );
    }

    #[test]
    fn batching_constants_are_sane() {
        assert!(MAX_BATCH_SIZE <= QUEUE_CAPACITY);
        assert!(MAX_BATCH_SIZE > 0);
        assert!(FLUSH_WINDOW_MS > 0);
    }

    #[test]
    fn spill_event_appends_one_json_line_per_event() {
        let dir = TempDir::new("append");
        let path = dir.file(SPILL_FILE_NAME);

        let e1 = AuditEvent::new("memory.write", "ltm_entry").tenant("t-1");
        let e2 = AuditEvent::new("memory.search", "kg_entity").actor("user-9");
        assert_eq!(
            spill_event_to(&path, DEFAULT_SPILL_MAX_BYTES, &e1),
            SpillOutcome::Spilled
        );
        assert_eq!(
            spill_event_to(&path, DEFAULT_SPILL_MAX_BYTES, &e2),
            SpillOutcome::Spilled
        );

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "one line per spilled event");

        // Round-trips back to the same events.
        let parsed = parse_spill_content(&content);
        assert_eq!(parsed.skipped, 0);
        assert_eq!(parsed.events.len(), 2);
        assert_eq!(parsed.events[0].event_id, e1.event_id);
        assert_eq!(parsed.events[1].event_id, e2.event_id);
    }

    #[test]
    fn parse_skips_truncated_tail_line_instead_of_failing() {
        // Simulate a crash mid-append: a valid line followed by a truncated JSON line.
        let good = AuditEvent::new("memory.write", "ltm_entry");
        let good_json = serde_json::to_string(&good).unwrap();
        let content = format!("{}\n{{\"event_id\":\"partial\",\"tenant", good_json);

        let parsed = parse_spill_content(&content);
        assert_eq!(parsed.events.len(), 1, "the one complete event survives");
        assert_eq!(parsed.events[0].event_id, good.event_id);
        assert_eq!(
            parsed.skipped, 1,
            "the truncated tail line is skipped, not fatal"
        );
    }

    #[test]
    fn spill_cap_exceeded_drops_instead_of_growing() {
        let dir = TempDir::new("cap");
        let path = dir.file(SPILL_FILE_NAME);

        // First write into an empty file is always allowed (len 0 < cap).
        let e1 = AuditEvent::new("memory.write", "ltm_entry");
        assert_eq!(spill_event_to(&path, 1, &e1), SpillOutcome::Spilled);

        // File is now non-empty (>= cap of 1 byte): the next event is a last-resort drop.
        let e2 = AuditEvent::new("memory.write", "ltm_entry");
        assert_eq!(
            spill_event_to(&path, 1, &e2),
            SpillOutcome::Dropped,
            "over-cap spill must drop rather than grow the file unbounded"
        );

        // The dropped event must NOT have been appended.
        let parsed = parse_spill_content(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(parsed.events.len(), 1);
    }

    #[test]
    fn enqueue_or_spill_routes_full_queue_to_disk() {
        let dir = TempDir::new("route");
        let path = dir.file(SPILL_FILE_NAME);

        // Capacity-1 channel with a live receiver held open.
        let (tx, _rx) = mpsc::channel::<AuditEvent>(1);

        // First fits on the queue.
        let a = AuditEvent::new("memory.write", "ltm_entry");
        assert_eq!(
            enqueue_or_spill(&tx, a, Some((&path, DEFAULT_SPILL_MAX_BYTES))),
            EnqueueOutcome::Enqueued
        );

        // Second finds the queue full → spilled to disk (not dropped).
        let b = AuditEvent::new("memory.write", "ltm_entry");
        let b_id = b.event_id.clone();
        assert_eq!(
            enqueue_or_spill(&tx, b, Some((&path, DEFAULT_SPILL_MAX_BYTES))),
            EnqueueOutcome::Spilled
        );

        let parsed = parse_spill_content(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(parsed.events.len(), 1);
        assert_eq!(parsed.events[0].event_id, b_id);
    }

    #[test]
    fn enqueue_or_spill_closed_queue_spills_to_disk() {
        let dir = TempDir::new("closed");
        let path = dir.file(SPILL_FILE_NAME);

        let (tx, rx) = mpsc::channel::<AuditEvent>(4);
        drop(rx); // worker gone → sends fail as Closed

        let ev = AuditEvent::new("memory.write", "ltm_entry");
        let id = ev.event_id.clone();
        assert_eq!(
            enqueue_or_spill(&tx, ev, Some((&path, DEFAULT_SPILL_MAX_BYTES))),
            EnqueueOutcome::Spilled,
            "a closed queue must spill, not drop"
        );
        let parsed = parse_spill_content(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(parsed.events.len(), 1);
        assert_eq!(parsed.events[0].event_id, id);
    }

    #[test]
    fn rotate_and_list_moves_active_to_replaying_snapshot() {
        let dir = TempDir::new("rotate");
        let active = dir.file(SPILL_FILE_NAME);

        // Nothing to rotate yet.
        rotate_active_spill_in(dir.path());
        assert!(list_replaying_files_in(dir.path()).is_empty());

        // Spill an event, then rotate.
        let ev = AuditEvent::new("memory.write", "ltm_entry");
        assert_eq!(
            spill_event_to(&active, DEFAULT_SPILL_MAX_BYTES, &ev),
            SpillOutcome::Spilled
        );
        rotate_active_spill_in(dir.path());

        // Active file is gone; exactly one replaying snapshot exists with the event.
        assert!(!active.exists(), "active file renamed away by rotation");
        let replaying = list_replaying_files_in(dir.path());
        assert_eq!(replaying.len(), 1);
        let parsed = parse_spill_content(&std::fs::read_to_string(&replaying[0]).unwrap());
        assert_eq!(parsed.events.len(), 1);
        assert_eq!(parsed.events[0].event_id, ev.event_id);
    }

    #[test]
    fn rotate_is_noop_for_empty_active_file() {
        let dir = TempDir::new("rotate_empty");
        let active = dir.file(SPILL_FILE_NAME);
        std::fs::write(&active, b"").unwrap(); // zero-length

        rotate_active_spill_in(dir.path());
        assert!(
            list_replaying_files_in(dir.path()).is_empty(),
            "an empty active spill file should not produce a replay snapshot"
        );
    }
}
