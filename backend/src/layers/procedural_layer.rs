//! Procedural Memory Layer Implementation
//!
//! Stores and retrieves "how-to-do" skill/process memories:
//! operation steps, tool call chains, and execution context.
//! Uses MemoryContent::Json for storage with schema validation.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::traits::{LayerStats, MemoryLayer};
use crate::kernel::types::*;
use crate::models::procedural::ProceduralEntry;

struct ProceduralState {
    entries: HashMap<String, MemoryEntry>,
    versions: HashMap<String, Vec<String>>,
}

pub struct ProceduralMemoryLayer {
    state: Arc<RwLock<ProceduralState>>,
}

impl ProceduralMemoryLayer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(ProceduralState {
                entries: HashMap::new(),
                versions: HashMap::new(),
            })),
        }
    }

    fn validate_procedural_content(content: &MemoryContent) -> MemoryResult<ProceduralEntry> {
        match content {
            MemoryContent::Json(value) => {
                let entry: ProceduralEntry =
                    serde_json::from_value(value.clone()).map_err(|e| {
                        MemoryError::Serialization(format!("invalid procedural entry: {e}"))
                    })?;
                entry.validate().map_err(|e| {
                    MemoryError::InvalidOperation(format!("validation failed: {e}"))
                })?;
                Ok(entry)
            }
            _ => Err(MemoryError::InvalidOperation(
                "procedural layer only accepts Json content".to_string(),
            )),
        }
    }

    fn matches_query(
        entry: &MemoryEntry,
        proc_entry: &ProceduralEntry,
        query: &MemoryQuery,
    ) -> bool {
        if let Some(ref text) = query.text {
            let lower = text.to_lowercase();
            let searchable = proc_entry.searchable_text().to_lowercase();
            if !searchable.contains(&lower) {
                return false;
            }
        }

        if let Some(ref tags) = query.filters.tags {
            let entry_tags = &entry.metadata.tags;
            if !tags.iter().any(|t| entry_tags.contains(t)) {
                return false;
            }
        }

        if let Some(ref user_id) = query.filters.user_id {
            if entry.metadata.user_id.as_ref() != Some(user_id) {
                return false;
            }
        }

        true
    }

    fn score_entry(proc_entry: &ProceduralEntry) -> f64 {
        let execution_boost = (proc_entry.execution_count as f64).ln_1p() / 10.0;
        proc_entry.success_rate + execution_boost
    }
}

impl Default for ProceduralMemoryLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl MemoryLayer for ProceduralMemoryLayer {
    fn layer_type(&self) -> LayerType {
        LayerType::Procedural
    }

    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId> {
        let proc_entry = Self::validate_procedural_content(&entry.content)?;

        let id = entry.id.clone();
        let version_key = format!("{}:{}", proc_entry.task_type, proc_entry.name);

        let mut state = self.state.write().await;
        state
            .versions
            .entry(version_key)
            .or_default()
            .push(id.0.clone());
        state.entries.insert(id.0.clone(), entry);

        Ok(id)
    }

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry> {
        let state = self.state.read().await;
        state
            .entries
            .get(&id.0)
            .cloned()
            .ok_or_else(|| MemoryError::NotFound(format!("procedural memory not found: {}", id.0)))
    }

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        let state = self.state.read().await;
        let mut results = Vec::new();

        for entry in state.entries.values() {
            let proc_entry = match &entry.content {
                MemoryContent::Json(v) => serde_json::from_value::<ProceduralEntry>(v.clone()).ok(),
                _ => None,
            };

            if let Some(ref proc) = proc_entry {
                if Self::matches_query(entry, proc, query) {
                    let score = Self::score_entry(proc);
                    results.push(MemoryMatch {
                        entry: entry.clone(),
                        score,
                        highlights: vec![proc.name.clone()],
                    });
                }
            }
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(query.limit);

        Ok(results)
    }

    async fn update(&self, id: &MemoryId, entry: MemoryEntry) -> MemoryResult<()> {
        Self::validate_procedural_content(&entry.content)?;

        let mut state = self.state.write().await;
        if !state.entries.contains_key(&id.0) {
            return Err(MemoryError::NotFound(format!(
                "procedural memory not found: {}",
                id.0
            )));
        }

        state.entries.insert(id.0.clone(), entry);
        Ok(())
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        let mut state = self.state.write().await;
        if state.entries.remove(&id.0).is_none() {
            return Err(MemoryError::NotFound(format!(
                "procedural memory not found: {}",
                id.0
            )));
        }
        // Clean up version tracking
        for versions in state.versions.values_mut() {
            versions.retain(|v| v != &id.0);
        }
        Ok(())
    }

    async fn stats(&self) -> MemoryResult<LayerStats> {
        let state = self.state.read().await;
        let total_size: u64 = state
            .entries
            .values()
            .map(|e| {
                serde_json::to_string(e)
                    .map(|s| s.len() as u64)
                    .unwrap_or(0)
            })
            .sum();

        Ok(LayerStats {
            entry_count: state.entries.len(),
            size_bytes: total_size,
            avg_access_count: 0.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_procedural_entry() -> MemoryEntry {
        let proc = ProceduralEntry {
            name: "deploy-service".to_string(),
            description: "Deploy a microservice to k8s".to_string(),
            task_type: "deployment".to_string(),
            steps: vec![
                crate::models::procedural::ProceduralStep {
                    order: 0,
                    action: "Build image".to_string(),
                    tool: Some("docker".to_string()),
                    parameters: HashMap::new(),
                    expected_output: None,
                    fallback: None,
                },
                crate::models::procedural::ProceduralStep {
                    order: 1,
                    action: "Apply manifests".to_string(),
                    tool: Some("kubectl".to_string()),
                    parameters: HashMap::new(),
                    expected_output: None,
                    fallback: Some("rollback".to_string()),
                },
            ],
            preconditions: vec!["cluster access".to_string()],
            tools_used: vec!["docker".to_string(), "kubectl".to_string()],
            success_rate: 0.9,
            execution_count: 5,
            version: 1,
            context: HashMap::new(),
        };

        let content = MemoryContent::Json(serde_json::to_value(&proc).unwrap());
        let mut entry = MemoryEntry::new(LayerType::Procedural, content);
        entry.metadata.tags = vec!["deployment".to_string(), "k8s".to_string()];
        entry
    }

    #[tokio::test]
    async fn store_and_retrieve_procedural() {
        let layer = ProceduralMemoryLayer::new();
        let entry = make_procedural_entry();
        let id = layer.store(entry.clone()).await.unwrap();
        let retrieved = layer.retrieve(&id).await.unwrap();
        assert_eq!(retrieved.id, id);
    }

    #[tokio::test]
    async fn rejects_non_json_content() {
        let layer = ProceduralMemoryLayer::new();
        let entry = MemoryEntry::new(
            LayerType::Procedural,
            MemoryContent::Text("bad".to_string()),
        );
        let result = layer.store(entry).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn search_by_text() {
        let layer = ProceduralMemoryLayer::new();
        layer.store(make_procedural_entry()).await.unwrap();

        let query = MemoryQuery {
            text: Some("deploy".to_string()),
            layer: Some(LayerType::Procedural),
            ..Default::default()
        };

        let results = layer.search(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].score > 0.0);
    }

    #[tokio::test]
    async fn search_no_match() {
        let layer = ProceduralMemoryLayer::new();
        layer.store(make_procedural_entry()).await.unwrap();

        let query = MemoryQuery {
            text: Some("nonexistent".to_string()),
            ..Default::default()
        };

        let results = layer.search(&query).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn version_tracking() {
        let layer = ProceduralMemoryLayer::new();
        layer.store(make_procedural_entry()).await.unwrap();
        layer.store(make_procedural_entry()).await.unwrap();

        let state = layer.state.read().await;
        assert_eq!(
            state
                .versions
                .get("deployment:deploy-service")
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn delete_procedural() {
        let layer = ProceduralMemoryLayer::new();
        let id = layer.store(make_procedural_entry()).await.unwrap();
        layer.delete(&id).await.unwrap();
        assert!(layer.retrieve(&id).await.is_err());
    }
}
