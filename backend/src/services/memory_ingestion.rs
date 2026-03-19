//! Proactive Layered Memory Ingestion Pipeline (Issue #50).
//!
//! Implements a three-tier memory model:
//!
//! ```text
//!  ┌─────────────────────────────────────────────────────────────┐
//!  │           Incoming Content (from MemoryStorageService)       │
//!  └────────────────────────┬────────────────────────────────────┘
//!                           │  classify_tier()
//!             ┌─────────────┼──────────────┐
//!             ▼             ▼              ▼
//!         Working       Episodic       Semantic
//!        (STM only)   (STM + LTM tag)  (direct LTM)
//!             └─────────────┼──────────────┘
//!                           │  ReflectionDaemon (background)
//!                           ▼
//!                   Importance Promotion
//!                  (STM → LTM when score
//!                    crosses threshold)
//! ```
//!
//! The **ReflectionDaemon** runs as a background Tokio task. Each cycle it:
//! 1. Reads all active STM sessions and their messages.
//! 2. For messages whose `importance_score >= min_promotion_score` (and whose
//!    content is not yet in LTM), creates a `knowledge_entry` via
//!    `MemoryStorageService::store_ltm`.
//! 3. Enforces the **sliding-window** limit: if a session exceeds
//!    `stm_sliding_window_size` messages, the oldest messages above the cap
//!    are archived (deleted from the hot table after promoting if they qualify,
//!    or simply deleted if they don't).
//!
//! The daemon is idempotent — duplicate promotion attempts are silently ignored
//! because `LTMRepository::create_knowledge_entry` reuses `content_hash`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::db::stm::STMRepository;
use crate::services::memory_storage::MemoryStorageService;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Ingestion tier for a piece of incoming content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestionTier {
    /// Working memory — transient, lives only in STM.
    /// Typical TTL < 1 hour; low importance expected.
    Working,
    /// Episodic memory — short/medium-lived events.
    /// Stored as STM and eligible for later LTM promotion.
    Episodic,
    /// Semantic memory — factual / persistent knowledge.
    /// Ingested directly into LTM (bypasses STM).
    Semantic,
}

/// Configuration for the ingestion pipeline and reflection daemon.
#[derive(Debug, Clone)]
pub struct IngestionConfig {
    /// STM messages above this importance score are considered Episodic.
    pub episodic_importance_threshold: f32,
    /// STM messages above this importance score are promoted directly to LTM (Semantic).
    pub semantic_importance_threshold: f32,
    /// Maximum number of messages to retain per STM session (sliding window).
    /// Messages beyond this cap (oldest first) are evicted after promotion.
    pub stm_sliding_window_size: usize,
    /// How often the reflection daemon runs (seconds).
    pub reflection_interval_seconds: u64,
    /// Minimum importance score for a STM message to be promoted to LTM.
    pub min_promotion_score: f32,
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            episodic_importance_threshold: 0.4,
            semantic_importance_threshold: 0.75,
            stm_sliding_window_size: 200,
            reflection_interval_seconds: 120,
            min_promotion_score: 0.6,
        }
    }
}

// ---------------------------------------------------------------------------
// Tier classification (pure — no I/O)
// ---------------------------------------------------------------------------

/// Classify incoming content into an [`IngestionTier`] based on estimated
/// importance and any explicit content signals.
///
/// `importance_score` is in `[0.0, 1.0]`.  If the caller has not scored the
/// content yet, pass `0.5` as a neutral default.
pub fn classify_tier(importance_score: f32, content: &str, cfg: &IngestionConfig) -> IngestionTier {
    // Keyword signals for Semantic (factual / persistent) content
    let content_lower = content.to_lowercase();
    let semantic_keywords = [
        "remember this",
        "important fact",
        "always",
        "never forget",
        "key insight",
        "rule:",
        "policy:",
        "definition:",
        "fact:",
    ];
    if semantic_keywords.iter().any(|kw| content_lower.contains(kw)) {
        return IngestionTier::Semantic;
    }

    // Score-based classification
    if importance_score >= cfg.semantic_importance_threshold {
        IngestionTier::Semantic
    } else if importance_score >= cfg.episodic_importance_threshold {
        IngestionTier::Episodic
    } else {
        IngestionTier::Working
    }
}

// ---------------------------------------------------------------------------
// Global ingestion config
// ---------------------------------------------------------------------------

static INGESTION_CONFIG: OnceLock<IngestionConfig> = OnceLock::new();
static DAEMON_RUNNING: AtomicBool = AtomicBool::new(false);

/// Return the active [`IngestionConfig`], if the daemon has been started.
pub fn get_config() -> Option<&'static IngestionConfig> {
    INGESTION_CONFIG.get()
}

// ---------------------------------------------------------------------------
// Background Reflection Daemon
// ---------------------------------------------------------------------------

/// Start the proactive reflection daemon.
///
/// Safe to call from multiple places — only the first call has any effect.
pub fn init_reflection_daemon(cfg: IngestionConfig) {
    if DAEMON_RUNNING.swap(true, Ordering::SeqCst) {
        return; // already running
    }
    let _ = INGESTION_CONFIG.set(cfg.clone());

    info!(
        interval_s = cfg.reflection_interval_seconds,
        promotion_threshold = cfg.min_promotion_score,
        sliding_window = cfg.stm_sliding_window_size,
        "Starting memory reflection daemon"
    );

    let running = Arc::new(AtomicBool::new(true));
    tokio::spawn(reflection_loop(cfg, running));
}

async fn reflection_loop(cfg: IngestionConfig, running: Arc<AtomicBool>) {
    while running.load(Ordering::Relaxed) {
        sleep(Duration::from_secs(cfg.reflection_interval_seconds)).await;

        if let Err(e) = run_reflection_cycle(&cfg).await {
            error!("Reflection cycle error: {}", e);
        }
    }
}

/// Run one full reflection cycle.
///
/// 1. Collect active user IDs from STM.
/// 2. For each user, iterate sessions and messages.
/// 3. Promote high-importance messages to LTM.
/// 4. Enforce the sliding-window limit per session.
async fn run_reflection_cycle(cfg: &IngestionConfig) -> anyhow::Result<()> {
    let user_ids = match STMRepository::get_active_user_ids().await {
        Ok(ids) => ids,
        Err(e) => {
            warn!("Failed to fetch active user IDs: {}", e);
            return Ok(());
        }
    };

    if user_ids.is_empty() {
        return Ok(());
    }

    let mut total_promoted: usize = 0;
    let mut total_evicted: usize = 0;

    for user_id in &user_ids {
        // Fetch active agent IDs for this user
        let agent_ids = match STMRepository::get_active_agent_ids(user_id).await {
            Ok(ids) => ids,
            Err(_) => continue,
        };

        for agent_id in &agent_ids {
            // Fetch recent sessions (limit 50 per agent to keep cycles bounded)
            let sessions = match STMRepository::get_recent_sessions(user_id, agent_id, Some(50)).await {
                Ok(s) => s,
                Err(_) => continue,
            };

            for session in &sessions {
                let msgs = match STMRepository::get_session_messages(&session.session_id, Some(1000)).await {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                // --- Promotion pass ---
                for msg in &msgs {
                    let score = msg.importance_score.unwrap_or(0.0) as f32;
                    if score < cfg.min_promotion_score {
                        continue;
                    }
                    // Build a descriptive source_id for LTM
                    let source_id = format!("stm:{}:{}", session.session_id, msg.message_id);
                    match MemoryStorageService::store_ltm(
                        &source_id,
                        "user_input",
                        &msg.content,
                        None, // title derived inside store_ltm
                    )
                    .await
                    {
                        Ok(entry_id) => {
                            info!(
                                msg_id = %msg.message_id,
                                entry_id = %entry_id,
                                score,
                                "Promoted STM message to LTM"
                            );
                            total_promoted += 1;
                        }
                        Err(e) => {
                            // Duplicate content returns an error about existing hash — ignore.
                            let msg_str = e.to_string();
                            if !msg_str.contains("duplicate") && !msg_str.contains("unique") {
                                warn!("LTM promotion error: {}", e);
                            }
                        }
                    }
                }

                // --- Sliding-window eviction pass ---
                let window = cfg.stm_sliding_window_size;
                if msgs.len() > window {
                    let excess = msgs.len() - window;
                    // msgs are returned oldest-first; evict the first `excess` entries
                    let to_evict = &msgs[..excess];
                    if let Err(e) = evict_stm_messages(
                        &session.session_id,
                        to_evict.iter().map(|m| m.message_id.as_str()),
                    )
                    .await
                    {
                        warn!("Sliding-window eviction error: {}", e);
                    } else {
                        total_evicted += excess;
                    }
                }
            }
        }
    }

    if total_promoted > 0 || total_evicted > 0 {
        info!(
            promoted = total_promoted,
            evicted = total_evicted,
            "Reflection cycle completed"
        );
    }

    Ok(())
}

/// Delete a set of STM messages by ID (sliding-window eviction).
/// Uses raw SQL so we don't need a separate repository method.
async fn evict_stm_messages<'a, I>(session_id: &str, message_ids: I) -> anyhow::Result<usize>
where
    I: Iterator<Item = &'a str>,
{
    let pool = crate::db::pool();
    let ids: Vec<&str> = message_ids.collect();
    if ids.is_empty() {
        return Ok(0);
    }
    let mut evicted = 0usize;
    for id in &ids {
        sqlx::query("DELETE FROM session_messages WHERE message_id = $1 AND session_id = $2")
            .bind(*id)
            .bind(session_id)
            .execute(pool)
            .await?;
        evicted += 1;
    }
    Ok(evicted)
}
