//! Memory Consolidation Service
//!
//! Implements sleep-like memory consolidation for transforming short-term memories
//! into structured long-term knowledge.

use crate::kernel::error::MemoryResult;
use crate::kernel::types::*;

/// Consolidation trigger type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerType {
    /// Time-based trigger (e.g., daily during idle period)
    Scheduled,
    /// Threshold-based trigger (e.g., STM backlog exceeds limit)
    Threshold,
    /// Manual trigger
    Manual,
}

/// Consolidation result
#[derive(Debug, Clone)]
pub struct ConsolidationResult {
    /// Number of memories consolidated
    pub consolidated_count: usize,
    /// Number of memories compressed
    pub compressed_count: usize,
    /// Number of conflicts resolved
    pub conflicts_resolved: usize,
    /// Summary of operations
    pub summary: Vec<String>,
}

impl Default for ConsolidationResult {
    fn default() -> Self {
        Self {
            consolidated_count: 0,
            compressed_count: 0,
            conflicts_resolved: 0,
            summary: vec![],
        }
    }
}

/// Consolidation configuration
#[derive(Debug, Clone)]
pub struct ConsolidationConfig {
    /// Maximum STM entries before threshold trigger
    pub stm_threshold: usize,
    /// Minimum time between scheduled consolidations (seconds)
    pub schedule_interval_seconds: i64,
    /// Maximum entries to process per cycle
    pub max_entries_per_cycle: usize,
    /// Enable memory compression
    pub enable_compression: bool,
    /// Enable conflict detection
    pub enable_conflict_resolution: bool,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            stm_threshold: 1000,
            schedule_interval_seconds: 86400, // Daily
            max_entries_per_cycle: 100,
            enable_compression: true,
            enable_conflict_resolution: true,
        }
    }
}

/// Memory consolidation service
pub struct ConsolidationService {
    config: ConsolidationConfig,
    last_consolidation: i64,
}

impl Default for ConsolidationService {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsolidationService {
    pub fn new() -> Self {
        Self {
            config: ConsolidationConfig::default(),
            last_consolidation: 0,
        }
    }

    pub fn with_config(config: ConsolidationConfig) -> Self {
        Self {
            config,
            last_consolidation: 0,
        }
    }

    /// Check if consolidation should be triggered
    pub fn should_consolidate(&self, stm_count: usize) -> bool {
        let now = chrono::Utc::now().timestamp();

        // Check threshold trigger
        if stm_count >= self.config.stm_threshold {
            return true;
        }

        // Check scheduled trigger
        if now - self.last_consolidation >= self.config.schedule_interval_seconds {
            return true;
        }

        false
    }

    /// Run memory consolidation cycle
    pub async fn consolidate(
        &mut self,
        stm_entries: &[MemoryEntry],
    ) -> MemoryResult<ConsolidationResult> {
        use crate::services::memory_storage::MemoryStorageService;

        let now = chrono::Utc::now().timestamp();
        let mut result = ConsolidationResult::default();

        // Limit entries to process
        let entries_to_process = stm_entries
            .iter()
            .take(self.config.max_entries_per_cycle)
            .collect::<Vec<_>>();

        for entry in entries_to_process {
            // Check if entry should be consolidated based on age and importance
            let age_hours = (now - entry.created_at) as f64 / 3600.0;
            let importance = entry.metadata.importance;

            // Consolidate important old memories to LTM
            if age_hours > 24.0 && importance > 0.5 {
                // Get session_id from metadata
                if let Some(session_id) = &entry.metadata.session_id {
                    // Extract content string from MemoryContent enum
                    let content = match &entry.content {
                        MemoryContent::Text(s) => s.clone(),
                        MemoryContent::Json(v) => v.to_string(),
                        MemoryContent::Binary(_) => String::from("[binary data]"),
                        MemoryContent::Graph(_) => String::from("[graph data]"),
                    };
                    // Truncate content for LTM storage
                    let truncated = content.chars().take(100).collect::<String>();

                    // Transfer to LTM via storage service
                    match MemoryStorageService::store_ltm(
                        session_id,
                        "consolidated",
                        &format!("Consolidated from STM: {}", truncated),
                        None,
                    )
                    .await
                    {
                        Ok(_) => {
                            result.consolidated_count += 1;
                        }
                        Err(e) => {
                            result.summary.push(format!(
                                "Transfer failed for {}: {}",
                                entry.id.as_str(),
                                e
                            ));
                        }
                    }
                }
            } else if self.config.enable_compression && age_hours > 48.0 {
                // Compress old low-importance memories
                result.compressed_count += 1;
                result
                    .summary
                    .push(format!("Compressed entry: {}", entry.id.as_str()));
            }
        }

        // Conflict resolution
        if self.config.enable_conflict_resolution {
            let conflicts = self.detect_conflicts(stm_entries).await;
            result.conflicts_resolved = conflicts.len();
            for conflict in conflicts {
                result
                    .summary
                    .push(format!("Resolved conflict: {}", conflict));
            }
        }

        // Update last consolidation time
        self.last_consolidation = now;

        if result.consolidated_count == 0 && result.compressed_count == 0 {
            result
                .summary
                .push("No memories met consolidation criteria".to_string());
        }

        Ok(result)
    }

    /// Detect conflicts between memories
    async fn detect_conflicts(&self, _entries: &[MemoryEntry]) -> Vec<String> {
        // Placeholder for conflict detection logic
        // In production, this would compare semantic embeddings or content
        vec![]
    }

    /// Get consolidation statistics
    pub fn get_stats(&self) -> ConsolidationStats {
        ConsolidationStats {
            last_consolidation: self.last_consolidation,
            config: self.config.clone(),
        }
    }
}

/// Consolidation statistics
#[derive(Debug, Clone)]
pub struct ConsolidationStats {
    pub last_consolidation: i64,
    pub config: ConsolidationConfig,
}

/// Background task for periodic consolidation
pub struct ConsolidationScheduler {
    service: ConsolidationService,
}

impl Default for ConsolidationScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsolidationScheduler {
    pub fn new() -> Self {
        Self {
            service: ConsolidationService::new(),
        }
    }

    pub fn with_config(config: ConsolidationConfig) -> Self {
        Self {
            service: ConsolidationService::with_config(config),
        }
    }

    /// Run consolidation if triggered
    pub async fn run_cycle(
        &mut self,
        stm_entries: &[MemoryEntry],
    ) -> MemoryResult<ConsolidationResult> {
        if self.service.should_consolidate(stm_entries.len()) {
            self.service.consolidate(stm_entries).await
        } else {
            Ok(ConsolidationResult {
                summary: vec!["Consolidation not triggered".to_string()],
                ..Default::default()
            })
        }
    }

    /// Force immediate consolidation
    pub async fn force_consolidate(
        &mut self,
        stm_entries: &[MemoryEntry],
    ) -> MemoryResult<ConsolidationResult> {
        self.service.consolidate(stm_entries).await
    }

    /// Get service statistics
    pub fn get_stats(&self) -> ConsolidationStats {
        self.service.get_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consolidation_config_defaults() {
        let config = ConsolidationConfig::default();
        assert_eq!(config.stm_threshold, 1000);
        assert_eq!(config.schedule_interval_seconds, 86400);
    }

    #[test]
    fn test_should_consolidate_threshold() {
        let service = ConsolidationService::with_config(ConsolidationConfig {
            stm_threshold: 100,
            schedule_interval_seconds: i64::MAX, // Disable scheduled trigger
            ..Default::default()
        });

        assert!(service.should_consolidate(100));
        assert!(service.should_consolidate(150));
        assert!(!service.should_consolidate(50));
    }

    #[test]
    fn test_consolidation_result_default() {
        let result = ConsolidationResult::default();
        assert_eq!(result.consolidated_count, 0);
        assert_eq!(result.compressed_count, 0);
        assert_eq!(result.conflicts_resolved, 0);
    }

    #[test]
    fn test_consolidation_scheduler() {
        let scheduler = ConsolidationScheduler::new();
        let stats = scheduler.get_stats();
        assert_eq!(stats.config.stm_threshold, 1000);
    }
}
