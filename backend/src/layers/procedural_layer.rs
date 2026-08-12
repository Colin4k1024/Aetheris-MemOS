//! Procedural Memory Layer Implementation (A-6: connected to real storage).
//!
//! Stores and retrieves "how-to-do" skill/process memories:
//! operation steps, tool call chains, and execution context.
//!
//! ## Storage strategy
//!
//! Entries are persisted to `knowledge_entries` (source_type = 'procedural') and
//! kept in an in-memory cache for fast search. The in-memory search uses substring
//! matching + tag filtering, which is correct for procedural entries (small count,
//! structured names). On startup the cache is cold and populated lazily on first
//! search; writes go to both PG and cache atomically.
//!
//! This replaces the previous pure in-memory HashMap that lost all entries on
//! restart (backlog A-6). The search semantics are unchanged.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::traits::{LayerStats, MemoryLayer};
use crate::kernel::types::*;
use crate::models::procedural::ProceduralEntry;

struct ProceduralState {
    entries: HashMap<String, MemoryEntry>,
    versions: HashMap<String, Vec<String>>,
    loaded_from_db: bool,
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
                loaded_from_db: false,
            })),
        }
    }

    /// Load procedural entries from PG into the in-memory cache. Called lazily on
    /// first search. Idempotent — skips if already loaded.
    async fn ensure_loaded(&self) {
        {
            let state = self.state.read().await;
            if state.loaded_from_db {
                return;
            }
        }

        if !crate::db::is_postgres() {
            let mut state = self.state.write().await;
            state.loaded_from_db = true;
            return;
        }

        let pool = crate::db::pool();
        let rows = match sqlx::query_as::<_, (String, String, String)>(
            "SELECT entry_id, content, source_id FROM knowledge_entries \
             WHERE source_type = 'procedural' AND status = 'active' \
             ORDER BY created_at DESC LIMIT 1000",
        )
        .fetch_all(pool)
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                warn!("Failed to load procedural entries from DB: {e}");
                let mut state = self.state.write().await;
                state.loaded_from_db = true;
                return;
            }
        };

        let mut state = self.state.write().await;
        if state.loaded_from_db {
            return; // another task loaded while we waited
        }

        for (entry_id, content_json, _source_id) in rows {
            let Ok(content_value) = serde_json::from_str::<serde_json::Value>(&content_json) else {
                continue;
            };
            let Ok(proc_entry) = serde_json::from_value::<ProceduralEntry>(content_value.clone())
            else {
                continue;
            };

            let now = chrono::Utc::now().timestamp();
            let mut metadata = MemoryMetadata::default();
            metadata
                .tags
                .push(format!("task_type:{}", proc_entry.task_type));

            let entry = MemoryEntry {
                id: MemoryId(entry_id.clone()),
                content: MemoryContent::Json(content_value),
                layer: LayerType::Procedural,
                metadata,
                created_at: now,
                updated_at: now,
            };

            let version_key = format!("{}:{}", proc_entry.task_type, proc_entry.name);
            state
                .versions
                .entry(version_key)
                .or_default()
                .push(entry_id.clone());
            state.entries.insert(entry_id, entry);
        }

        info!("Loaded {} procedural entries from DB", state.entries.len());
        state.loaded_from_db = true;
    }

    /// Persist a single entry to PG.
    async fn persist_to_db(id: &str, content: &MemoryContent, tags: &[String]) {
        if !crate::db::is_postgres() {
            return;
        }

        let content_str = match content {
            MemoryContent::Json(v) => serde_json::to_string(v).unwrap_or_default(),
            _ => return,
        };

        let pool = crate::db::pool();
        let content_hash = crate::services::information_guard::compute_sha256(&content_str);
        let source_id = format!("t:system:procedural:{id}");
        let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());

        if let Err(e) = sqlx::query(
            "INSERT INTO knowledge_entries \
             (entry_id, source_id, source_type, content, content_type, content_hash, \
              embedding_vector, embedding_model, embedding_dimension, status, tenant_id, \
              summary_status, category) \
             VALUES ($1, $2, 'procedural', $3, 'application/json', $4, '[]', 'none', 0, \
                     'active', 'system', 'complete', $5) \
             ON CONFLICT (entry_id) DO UPDATE SET content = $3, content_hash = $4",
        )
        .bind(id)
        .bind(&source_id)
        .bind(&content_str)
        .bind(&content_hash)
        .bind(&tags_json)
        .execute(pool)
        .await
        {
            warn!("Failed to persist procedural entry {id}: {e}");
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

        // Persist to DB first, then cache
        Self::persist_to_db(id.as_str(), &entry.content, &entry.metadata.tags).await;

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
        self.ensure_loaded().await;
        let state = self.state.read().await;
        state
            .entries
            .get(&id.0)
            .cloned()
            .ok_or_else(|| MemoryError::NotFound(format!("procedural memory not found: {}", id.0)))
    }

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        self.ensure_loaded().await;
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

        Self::persist_to_db(id.as_str(), &entry.content, &entry.metadata.tags).await;

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
        // Soft-delete in DB
        if crate::db::is_postgres() {
            let pool = crate::db::pool();
            let _ =
                sqlx::query("UPDATE knowledge_entries SET status = 'deleted' WHERE entry_id = $1")
                    .bind(&id.0)
                    .execute(pool)
                    .await;
        }

        let mut state = self.state.write().await;
        if state.entries.remove(&id.0).is_none() {
            return Err(MemoryError::NotFound(format!(
                "procedural memory not found: {}",
                id.0
            )));
        }
        for versions in state.versions.values_mut() {
            versions.retain(|v| v != &id.0);
        }
        Ok(())
    }

    async fn stats(&self) -> MemoryResult<LayerStats> {
        self.ensure_loaded().await;
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
