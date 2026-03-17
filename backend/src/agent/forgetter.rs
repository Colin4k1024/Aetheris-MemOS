//! Memory Forgetter - Intelligent Memory Eviction
//!
//! This module handles intelligent memory forgetting based on importance and age.
//! Implements bio-inspired fractal decay mechanism.

use crate::kernel::types::*;
use crate::kernel::error::MemoryResult;
use crate::agent::memory_agent::ForgetResult;

/// Memory type classification for decay strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    /// Working memory - short-term, hourly decay
    Working,
    /// Episodic memory - daily events, daily decay
    Episodic,
    /// Factual memory - knowledge facts, monthly decay
    Factual,
    /// Procedural memory - skills/procedures, minimal decay
    Procedural,
}

impl Default for MemoryType {
    fn default() -> Self {
        // Default to episodic memory
        MemoryType::Episodic
    }
}

impl MemoryType {
    /// Get decay rate (hours) for this memory type
    pub fn decay_rate_hours(&self) -> f64 {
        match self {
            MemoryType::Working => 1.0,      // Hourly
            MemoryType::Episodic => 24.0,    // Daily
            MemoryType::Factual => 720.0,    // ~Monthly (30 days)
            MemoryType::Procedural => 8760.0, // Yearly (minimal decay)
        }
    }

    /// Determine memory type from tags and metadata
    pub fn from_entry(entry: &MemoryEntry) -> Self {
        // Check tags for hints
        for tag in &entry.metadata.tags {
            let tag_lower = tag.to_lowercase();
            if tag_lower.contains("skill") || tag_lower.contains("procedure") || tag_lower.contains("procedure") {
                return MemoryType::Procedural;
            }
            if tag_lower.contains("fact") || tag_lower.contains("knowledge") || tag_lower.contains("entity") {
                return MemoryType::Factual;
            }
            if tag_lower.contains("episodic") || tag_lower.contains("event") || tag_lower.contains("conversation") {
                return MemoryType::Episodic;
            }
            if tag_lower.contains("working") || tag_lower.contains("temp") || tag_lower.contains("temporary") {
                return MemoryType::Working;
            }
        }

        // Default based on age and access patterns
        let now = chrono::Utc::now().timestamp();
        let age_hours = (now - entry.created_at) as f64 / 3600.0;

        if age_hours < 24.0 {
            MemoryType::Working
        } else if age_hours < 168.0 { // < 1 week
            MemoryType::Episodic
        } else if age_hours < 720.0 { // < 1 month
            MemoryType::Factual
        } else {
            MemoryType::Procedural
        }
    }
}

/// Fractal decay configuration
#[derive(Debug, Clone)]
pub struct FractalDecayConfig {
    /// Fractal exponent - higher values mean slower decay for important memories
    pub fractal_exponent: f64,
    /// Base decay rate multiplier
    pub base_decay_multiplier: f64,
    /// Minimum importance threshold
    pub min_importance_threshold: f64,
}

impl Default for FractalDecayConfig {
    fn default() -> Self {
        Self {
            fractal_exponent: 1.5,
            base_decay_multiplier: 1.0,
            min_importance_threshold: 0.1,
        }
    }
}

/// Intelligent memory forgetter that evicts low-importance memories.
pub struct MemoryForGetter {
    min_importance: f64,
    max_age_seconds: i64,
}

impl MemoryForGetter {
    pub fn new(min_importance: f64, max_age_seconds: i64) -> Self {
        Self {
            min_importance,
            max_age_seconds,
        }
    }

    /// Determine which memories should be evicted.
    pub async fn select_for_eviction(&self, entries: &[MemoryEntry]) -> Vec<MemoryId> {
        let now = chrono::Utc::now().timestamp();
        let mut to_evict = Vec::new();

        for entry in entries {
            let should_evict = self.should_forget(entry, now);
            if should_evict {
                to_evict.push(entry.id.clone());
            }
        }

        to_evict
    }

    /// Check if a memory should be forgotten.
    fn should_forget(&self, entry: &MemoryEntry, now: i64) -> bool {
        // Always evict if importance is below threshold
        if entry.metadata.importance < self.min_importance {
            return true;
        }

        // Check age
        if let Some(expires_at) = entry.metadata.expires_at {
            if now > expires_at {
                return true;
            }
        }

        // Evict old memories that haven't been accessed recently
        let age_seconds = now - entry.created_at;
        
        if age_seconds > self.max_age_seconds {
            // Very old: evict if not important
            return entry.metadata.importance < 0.5;
        }

        // Medium age: evict if rarely accessed and low importance
        if age_seconds > self.max_age_seconds / 2 {
            return entry.metadata.access_count < 3 && entry.metadata.importance < 0.4;
        }

        false
    }

    /// Execute eviction.
    pub async fn evict(&self, entries: &[MemoryEntry]) -> MemoryResult<ForgetResult> {
        let to_evict = self.select_for_eviction(entries).await;
        let evicted_count = to_evict.len();

        let reasons = if evicted_count == 0 {
            vec!["No memories met eviction criteria".to_string()]
        } else {
            vec![format!("Evicted {} low-importance/old memories", evicted_count)]
        };

        Ok(ForgetResult {
            evicted_count,
            reasons,
        })
    }
}

/// Calculate memory importance based on access patterns.
pub fn calculate_importance(entry: &MemoryEntry) -> f64 {
    let now = chrono::Utc::now().timestamp();

    // Base importance
    let mut importance = 0.5;

    // Boost based on access count
    importance += (entry.metadata.access_count as f64 * 0.05).min(0.3);

    // Boost based on recency
    let age_hours = (now - entry.created_at) / 3600;
    if age_hours < 1 {
        importance += 0.2;
    } else if age_hours < 24 {
        importance += 0.1;
    }

    // Cap at 1.0
    importance.min(1.0)
}

// ============================================================================
// Fractal Decay Mechanism
// ============================================================================

/// Fractal decay calculator for adaptive forgetting
pub struct FractalDecay {
    config: FractalDecayConfig,
}

impl Default for FractalDecay {
    fn default() -> Self {
        Self::new()
    }
}

impl FractalDecay {
    pub fn new() -> Self {
        Self {
            config: FractalDecayConfig::default(),
        }
    }

    pub fn with_config(config: FractalDecayConfig) -> Self {
        Self { config }
    }

    /// Calculate fractal decay for a memory entry
    ///
    /// The fractal decay formula:
    /// decay = importance * base_decay^(elapsed_hours / decay_rate^fractal_exponent)
    ///
    /// Higher importance memories decay slower
    /// Memories with longer decay rates (factual/procedural) decay slower
    pub fn calculate_decay(&self, entry: &MemoryEntry, now: i64) -> f64 {
        let memory_type = MemoryType::from_entry(entry);
        let decay_rate_hours = memory_type.decay_rate_hours();

        // Calculate elapsed hours since last access
        let last_access = entry.metadata.last_accessed.unwrap_or(entry.created_at);
        let elapsed_hours = (now - last_access) as f64 / 3600.0;

        // Base decay factor
        let base_decay = 1.0 / (1.0 + elapsed_hours * 0.1 * self.config.base_decay_multiplier);

        // Apply fractal exponent to make important memories decay slower
        let fractal_factor = (decay_rate_hours / 24.0).powf(self.config.fractal_exponent);

        // Calculate final decay
        let decay = entry.metadata.importance * base_decay.powf(1.0 / fractal_factor.max(0.1));

        // Clamp to 0-1 range
        decay.clamp(0.0, 1.0)
    }

    /// Check if a memory should be forgotten based on fractal decay
    pub fn should_forget(&self, entry: &MemoryEntry, now: i64) -> bool {
        let current_importance = self.calculate_decay(entry, now);
        current_importance < self.config.min_importance_threshold
    }

    /// Select memories for eviction using fractal decay
    pub fn select_for_eviction(&self, entries: &[MemoryEntry]) -> Vec<MemoryId> {
        let now = chrono::Utc::now().timestamp();
        entries
            .iter()
            .filter(|entry| self.should_forget(entry, now))
            .map(|entry| entry.id.clone())
            .collect()
    }

    /// Get decay info for a memory entry
    pub fn get_decay_info(&self, entry: &MemoryEntry) -> DecayInfo {
        let now = chrono::Utc::now().timestamp();
        let memory_type = MemoryType::from_entry(entry);
        let current_decay = self.calculate_decay(entry, now);

        DecayInfo {
            memory_type,
            current_decay,
            should_forget: self.should_forget(entry, now),
        }
    }
}

/// Information about memory decay
#[derive(Debug, Clone)]
pub struct DecayInfo {
    pub memory_type: MemoryType,
    pub current_decay: f64,
    pub should_forget: bool,
}

/// Background task for periodic memory forgetting
pub struct ForgetterScheduler {
    decay: FractalDecay,
    scan_interval_seconds: i64,
}

impl Default for ForgetterScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgetterScheduler {
    pub fn new() -> Self {
        Self {
            decay: FractalDecay::new(),
            scan_interval_seconds: 3600, // Default: hourly scan
        }
    }

    pub fn with_interval(mut self, seconds: i64) -> Self {
        self.scan_interval_seconds = seconds;
        self
    }

    /// Scan and evict low-importance memories
    pub async fn run_eviction_cycle(&self, entries: &[MemoryEntry]) -> ForgetResult {
        let to_evict = self.decay.select_for_eviction(entries);
        let evicted_count = to_evict.len();

        let reasons = if evicted_count == 0 {
            vec!["No memories met fractal decay eviction criteria".to_string()]
        } else {
            vec![format!(
                "Evicted {} memories due to fractal decay (importance below threshold)",
                evicted_count
            )]
        };

        ForgetResult {
            evicted_count,
            reasons,
        }
    }

    /// Get scan interval in seconds
    pub fn scan_interval(&self) -> i64 {
        self.scan_interval_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_type_from_entry() {
        let mut entry = MemoryEntry::new(LayerType::Stm, MemoryContent::Text("test".to_string()));
        entry.metadata.tags = vec!["skill:programming".to_string()];

        let memory_type = MemoryType::from_entry(&entry);
        assert_eq!(memory_type, MemoryType::Procedural);
    }

    #[test]
    fn test_fractal_decay_calculation() {
        let decay = FractalDecay::new();
        let mut entry = MemoryEntry::new(LayerType::Stm, MemoryContent::Text("test".to_string()));
        entry.metadata.importance = 0.8;

        let now = chrono::Utc::now().timestamp();
        let decay_value = decay.calculate_decay(&entry, now);

        // High importance should result in high decay value (close to original)
        assert!(decay_value > 0.5);
    }

    #[test]
    fn test_should_forget_low_importance() {
        let decay = FractalDecay::new();
        let mut entry = MemoryEntry::new(LayerType::Stm, MemoryContent::Text("test".to_string()));
        entry.metadata.importance = 0.05; // Very low importance

        let now = chrono::Utc::now().timestamp();
        // Create old entry
        entry.metadata.last_accessed = Some(now - (31 * 24 * 3600)); // 31 days ago

        assert!(decay.should_forget(&entry, now));
    }

    #[test]
    fn test_forgetter_scheduler() {
        let scheduler = ForgetterScheduler::new();
        assert_eq!(scheduler.scan_interval(), 3600);
    }
}
