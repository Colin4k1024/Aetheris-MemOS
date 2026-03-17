//! Importance Evaluator Service
//!
//! Provides intelligent importance scoring using LLM-as-a-Judge approach.

use crate::kernel::types::*;

/// Importance evaluation factors
#[derive(Debug, Clone)]
pub struct ImportanceFactors {
    /// Uniqueness score (0-1)
    pub uniqueness: f64,
    /// Emotional intensity (0-1)
    pub emotional_intensity: f64,
    /// Goal relevance (0-1)
    pub goal_relevance: f64,
    /// Timeliness (0-1)
    pub timeliness: f64,
}

impl Default for ImportanceFactors {
    fn default() -> Self {
        Self {
            uniqueness: 0.5,
            emotional_intensity: 0.0,
            goal_relevance: 0.5,
            timeliness: 0.5,
        }
    }
}

/// Importance evaluator service
pub struct ImportanceEvaluator {
    /// Weight for uniqueness factor
    uniqueness_weight: f64,
    /// Weight for emotional intensity
    emotional_weight: f64,
    /// Weight for goal relevance
    relevance_weight: f64,
    /// Weight for timeliness
    timeliness_weight: f64,
}

impl Default for ImportanceEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportanceEvaluator {
    pub fn new() -> Self {
        Self {
            uniqueness_weight: 0.3,
            emotional_weight: 0.2,
            relevance_weight: 0.3,
            timeliness_weight: 0.2,
        }
    }

    /// Create with custom weights
    pub fn with_weights(uniqueness: f64, emotional: f64, relevance: f64, timeliness: f64) -> Self {
        Self {
            uniqueness_weight: uniqueness,
            emotional_weight: emotional,
            relevance_weight: relevance,
            timeliness_weight: timeliness,
        }
    }

    /// Calculate importance score (0-1) for a memory entry
    pub fn evaluate(&self, entry: &MemoryEntry) -> f64 {
        let factors = self.extract_factors(entry);
        self.calculate_score(&factors)
    }

    /// Extract factors from a memory entry
    fn extract_factors(&self, entry: &MemoryEntry) -> ImportanceFactors {
        let now = chrono::Utc::now().timestamp();

        // Extract uniqueness based on tags and metadata
        let uniqueness = self.calculate_uniqueness(entry);

        // Extract emotional intensity (if available from metadata)
        let emotional_intensity = entry
            .metadata
            .extra
            .get("emotional_intensity")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        // Goal relevance based on tags
        let goal_relevance = self.calculate_goal_relevance(entry);

        // Timeliness based on age
        let timeliness = self.calculate_timeliness(entry, now);

        ImportanceFactors {
            uniqueness,
            emotional_intensity,
            goal_relevance,
            timeliness,
        }
    }

    /// Calculate uniqueness score
    fn calculate_uniqueness(&self, entry: &MemoryEntry) -> f64 {
        // Base score
        let mut score: f64 = 0.5;

        // Boost for having tags (indicates structured memory)
        if !entry.metadata.tags.is_empty() {
            score += 0.1;
        }

        // Boost for having an agent_id (indicates agent-generated content)
        if entry.metadata.agent_id.is_some() {
            score += 0.1;
        }

        // Boost for having a specific user
        if entry.metadata.user_id.is_some() {
            score += 0.1;
        }

        // Check source type
        if let Some(source) = &entry.metadata.source {
            match source.as_str() {
                "api" | "database" => score += 0.15, // Structured data is more valuable
                "user_input" => score += 0.1,
                _ => {}
            }
        }

        score.min(1.0)
    }

    /// Calculate goal relevance score
    fn calculate_goal_relevance(&self, entry: &MemoryEntry) -> f64 {
        // Base score
        let mut score: f64 = 0.5;

        // Boost for access count (frequently accessed = relevant)
        let access_count = entry.metadata.access_count as f64;
        score += (access_count * 0.05).min(0.3);

        // Boost for recent access
        if let Some(last_accessed) = entry.metadata.last_accessed {
            let now = chrono::Utc::now().timestamp();
            let hours_since_access = (now - last_accessed) / 3600;

            if hours_since_access < 1 {
                score += 0.15;
            } else if hours_since_access < 24 {
                score += 0.1;
            } else if hours_since_access < 168 {
                // Less than a week
                score += 0.05;
            }
        }

        score.min(1.0)
    }

    /// Calculate timeliness score
    fn calculate_timeliness(&self, entry: &MemoryEntry, now: i64) -> f64 {
        let age_seconds = now - entry.created_at;
        let age_hours = age_seconds as f64 / 3600.0;

        // Very recent (less than 1 hour)
        if age_hours < 1.0 {
            return 1.0;
        }
        // Recent (less than 24 hours)
        if age_hours < 24.0 {
            return 0.9;
        }
        // Within a week
        if age_hours < 168.0 {
            return 0.7;
        }
        // Within a month
        if age_hours < 720.0 {
            // ~30 days
            return 0.5;
        }
        // Older than a month
        0.3
    }

    /// Calculate final score from factors
    fn calculate_score(&self, factors: &ImportanceFactors) -> f64 {
        let score = factors.uniqueness * self.uniqueness_weight
            + factors.emotional_intensity * self.emotional_weight
            + factors.goal_relevance * self.relevance_weight
            + factors.timeliness * self.timeliness_weight;

        // Clamp to 0-1 range
        score.clamp(0.0, 1.0)
    }

    /// Batch evaluate multiple entries
    pub fn batch_evaluate(&self, entries: &[MemoryEntry]) -> Vec<f64> {
        entries.iter().map(|e| self.evaluate(e)).collect()
    }
}

/// Legacy function for backward compatibility
pub fn calculate_importance(entry: &MemoryEntry) -> f64 {
    let evaluator = ImportanceEvaluator::default();
    evaluator.evaluate(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_empty_entry() {
        let evaluator = ImportanceEvaluator::default();
        let entry = MemoryEntry::new(LayerType::Stm, MemoryContent::Text("test".to_string()));

        let score = evaluator.evaluate(&entry);
        assert!(score >= 0.0 && score <= 1.0);
    }

    #[test]
    fn test_evaluate_with_tags() {
        let evaluator = ImportanceEvaluator::default();
        let mut entry = MemoryEntry::new(LayerType::Stm, MemoryContent::Text("test".to_string()));
        entry.metadata.tags = vec!["important".to_string()];

        let score_with_tags = evaluator.evaluate(&entry);

        let mut entry_no_tags =
            MemoryEntry::new(LayerType::Stm, MemoryContent::Text("test".to_string()));
        let score_without_tags = evaluator.evaluate(&entry_no_tags);

        assert!(score_with_tags >= score_without_tags);
    }

    #[test]
    fn test_batch_evaluate() {
        let evaluator = ImportanceEvaluator::default();
        let entries = vec![
            MemoryEntry::new(LayerType::Stm, MemoryContent::Text("test1".to_string())),
            MemoryEntry::new(LayerType::Ltm, MemoryContent::Text("test2".to_string())),
        ];

        let scores = evaluator.batch_evaluate(&entries);
        assert_eq!(scores.len(), 2);
    }
}
