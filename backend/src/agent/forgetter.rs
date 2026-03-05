//! Memory Forgetter - Intelligent Memory Eviction
//!
//! This module handles intelligent memory forgetting based on importance and age.

use crate::kernel::types::*;
use crate::kernel::error::MemoryResult;
use crate::agent::memory_agent::ForgetResult;

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
