//! Memory Merger - Merge Similar Memories
//!
//! This module handles merging similar memory entries to reduce redundancy.

use crate::kernel::types::*;
use crate::kernel::error::MemoryResult;

/// Memory merger that combines similar memory entries.
pub struct MemoryMerger {
    similarity_threshold: f64,
}

impl MemoryMerger {
    pub fn new(similarity_threshold: f64) -> Self {
        Self { similarity_threshold }
    }

    /// Find and merge similar memories.
    /// 
    /// Returns a list of merge operations to perform.
    pub async fn find_merges(&self, entries: &[MemoryEntry]) -> Vec<MergeOperation> {
        let mut merges = Vec::new();
        let mut processed = std::collections::HashSet::new();

        for i in 0..entries.len() {
            if processed.contains(&i) {
                continue;
            }

            let mut similar: Vec<(usize, f64)> = Vec::new();

            for j in (i + 1)..entries.len() {
                if processed.contains(&j) {
                    continue;
                }

                let similarity = self.calculate_similarity(&entries[i], &entries[j]);
                
                if similarity >= self.similarity_threshold {
                    similar.push((j, similarity));
                    processed.insert(j);
                }
            }

            if !similar.is_empty() {
                merges.push(MergeOperation {
                    primary: entries[i].id.clone(),
                    secondary: similar.iter().map(|(idx, _)| entries[*idx].id.clone()).collect(),
                    similarity: similar.iter().map(|(_, s)| *s).sum::<f64>() / similar.len() as f64,
                });
            }

            processed.insert(i);
        }

        merges
    }

    /// Calculate similarity between two memory entries.
    fn calculate_similarity(&self, a: &MemoryEntry, b: &MemoryEntry) -> f64 {
        // Text similarity (simple implementation)
        let text_a = match &a.content {
            MemoryContent::Text(s) => s.clone(),
            _ => return 0.0,
        };

        let text_b = match &b.content {
            MemoryContent::Text(s) => s.clone(),
            _ => return 0.0,
        };

        // Simple word overlap similarity
        let lower_a = text_a.to_lowercase();
        let lower_b = text_b.to_lowercase();
        let words_a: std::collections::HashSet<_> = lower_a.split_whitespace().collect();
        let words_b: std::collections::HashSet<_> = lower_b.split_whitespace().collect();

        if words_a.is_empty() || words_b.is_empty() {
            return 0.0;
        }

        let intersection = words_a.intersection(&words_b).count();
        let union = words_a.union(&words_b).count();

        intersection as f64 / union as f64
    }

    /// Merge multiple memories into one.
    pub async fn merge(&self, primary: &MemoryEntry, secondary: &[&MemoryEntry]) -> MemoryEntry {
        // Combine content from all entries
        let mut combined_content = String::new();
        
        if let MemoryContent::Text(text) = &primary.content {
            combined_content = text.clone();
        }

        for entry in secondary {
            if let MemoryContent::Text(text) = &entry.content {
                if !combined_content.contains(text) {
                    combined_content.push_str("\n");
                    combined_content.push_str(text);
                }
            }
        }

        // Combine metadata
        let max_importance = secondary.iter()
            .map(|e| e.metadata.importance)
            .fold(primary.metadata.importance, f64::max);

        let total_access = secondary.iter()
            .map(|e| e.metadata.access_count)
            .sum::<u32>() + primary.metadata.access_count;

        // Create merged entry
        MemoryEntry {
            id: primary.id.clone(),
            layer: primary.layer,
            content: MemoryContent::Text(combined_content),
            metadata: MemoryMetadata {
                user_id: primary.metadata.user_id.clone(),
                session_id: primary.metadata.session_id.clone(),
                agent_id: primary.metadata.agent_id.clone(),
                tags: primary.metadata.tags.clone(),
                importance: max_importance,
                access_count: total_access,
                last_accessed: Some(chrono::Utc::now().timestamp()),
                expires_at: None,
                source: Some("merged".to_string()),
                extra: Default::default(),
            },
            created_at: primary.created_at,
            updated_at: chrono::Utc::now().timestamp(),
        }
    }
}

/// Represents a merge operation to perform.
#[derive(Debug, Clone)]
pub struct MergeOperation {
    pub primary: MemoryId,
    pub secondary: Vec<MemoryId>,
    pub similarity: f64,
}
