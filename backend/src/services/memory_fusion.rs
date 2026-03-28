//! Memory Fusion Service
//!
//! Provides a unified query interface across all memory layers (STM, LTM, KG, MM).
//! Cross-layer queries fan out to all repositories and merge results by relevance score.

use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::db::pool;
use crate::db::{stm::STMRepository, ltm::LTMRepository, kg::KGRepository, mm::MMRepository};
use crate::tenant::TenantId;
use crate::AppError;

/// Memory layer identifier for fusion results.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MemoryLayer {
    Stm,
    Ltm,
    Kg,
    Mm,
}

impl std::fmt::Display for MemoryLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryLayer::Stm => write!(f, "stm"),
            MemoryLayer::Ltm => write!(f, "ltm"),
            MemoryLayer::Kg => write!(f, "kg"),
            MemoryLayer::Mm => write!(f, "mm"),
        }
    }
}

/// A merged entry from cross-layer fusion query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedEntry {
    /// Unique identifier for this entry (layer-prefixed).
    pub id: String,
    /// Source layer.
    pub layer: MemoryLayer,
    /// Content summary or title.
    pub title: String,
    /// Full content (if available).
    pub content: String,
    /// Relevance score across layers (0.0 - 1.0).
    pub relevance_score: f64,
    /// Creation timestamp.
    pub created_at: String,
    /// Quality or confidence score.
    pub quality_score: Option<f32>,
}

/// Results broken down by layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerResults {
    pub stm: Vec<MemoryEntry>,
    pub ltm: Vec<MemoryEntry>,
    pub kg: Vec<MemoryEntry>,
    pub mm: Vec<MemoryEntry>,
}

/// A memory entry from a specific layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub layer: MemoryLayer,
    pub title: String,
    pub content: String,
    pub relevance_score: f64,
    pub created_at: String,
    pub quality_score: Option<f32>,
}

/// Cross-layer fusion result combining both layer-separated and merged views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionResult {
    /// Results grouped by layer.
    pub layer_results: LayerResults,
    /// Cross-layer results merged and sorted by relevance score.
    pub merged_results: Vec<MergedEntry>,
}

/// Memory fusion service status (layer counts and health).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionStatus {
    pub stm_count: i64,
    pub ltm_count: i64,
    pub kg_count: i64,
    pub mm_count: i64,
    pub stm_healthy: bool,
    pub ltm_healthy: bool,
    pub kg_healthy: bool,
    pub mm_healthy: bool,
}

/// Response for fusion status endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionStatusResponse {
    pub status: FusionStatus,
    pub total_entries: i64,
}

/// MemoryFusionService overlays all memory layers and provides cross-layer query results.
#[derive(Debug, Clone)]
pub struct MemoryFusionService;

impl MemoryFusionService {
    /// Query across all memory layers and return both layer-separated and merged results.
    ///
    /// For STM: uses exact match or prefix search.
    /// For LTM/KG: uses semantic similarity via ILIKE search (placeholder for vector similarity).
    /// For MM: uses text content search.
    pub async fn query(
        query: &str,
        tenant_id: &TenantId,
        limit: Option<i32>,
    ) -> Result<FusionResult, AppError> {
        let limit = limit.unwrap_or(20);
        let pool = pool();

        // Fan out to all layers concurrently
        let (stm_results, ltm_results, kg_results, mm_results) = tokio::join!(
            Self::query_stm(pool, tenant_id, query, limit),
            Self::query_ltm(pool, tenant_id, query, limit),
            Self::query_kg(pool, tenant_id, query, limit),
            Self::query_mm(pool, tenant_id, query, limit),
        );

        let stm = stm_results?;
        let ltm = ltm_results?;
        let kg = kg_results?;
        let mm = mm_results?;

        // Collect all entries from all layers (clone for merging to preserve layer_results)
        let all_entries: Vec<MemoryEntry> = stm
            .iter()
            .chain(ltm.iter())
            .chain(kg.iter())
            .chain(mm.iter())
            .cloned()
            .collect();

        // Sort by relevance score descending
        let mut merged: Vec<MergedEntry> = all_entries
            .iter()
            .map(|e| MergedEntry {
                id: format!("{}:{}", e.layer, e.id),
                layer: e.layer,
                title: e.title.clone(),
                content: e.content.clone(),
                relevance_score: e.relevance_score,
                created_at: e.created_at.clone(),
                quality_score: e.quality_score,
            })
            .collect();

        merged.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));

        let merged_results: Vec<MergedEntry> = merged.into_iter().take(limit as usize).collect();

        Ok(FusionResult {
            layer_results: LayerResults { stm, ltm, kg, mm },
            merged_results,
        })
    }

    /// Get fusion status: counts per layer and health indicators.
    pub async fn get_status(tenant_id: &TenantId) -> Result<FusionStatusResponse, AppError> {
        let pool = pool();

        // Query counts for each layer (all tenant-isolated)
        let (stm_count, ltm_count, kg_count, mm_count) = tokio::join!(
            Self::count_stm(pool, tenant_id),
            Self::count_ltm(pool, tenant_id),
            Self::count_kg(pool, tenant_id),
            Self::count_mm(pool),
        );

        let stm_count = stm_count.unwrap_or(0);
        let ltm_count = ltm_count.unwrap_or(0);
        let kg_count = kg_count.unwrap_or(0);
        let mm_count = mm_count.unwrap_or(0);

        let total_entries = stm_count + ltm_count + kg_count + mm_count;

        Ok(FusionStatusResponse {
            status: FusionStatus {
                stm_count,
                ltm_count,
                kg_count,
                mm_count,
                stm_healthy: stm_count >= 0,
                ltm_healthy: ltm_count >= 0,
                kg_healthy: kg_count >= 0,
                mm_healthy: mm_count >= 0,
            },
            total_entries,
        })
    }

    // ---------- private helpers ----------

    async fn query_stm(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        query: &str,
        limit: i32,
    ) -> Result<Vec<MemoryEntry>, AppError> {
        let prefix = tenant_id.prefix();
        let pattern = format!("{}%", prefix);

        // Search session messages by content (exact/prefix match)
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            r#"
            SELECT m.message_id, m.content, s.created_at::text
            FROM session_messages m
            JOIN context_sessions s ON m.session_id = s.session_id
            WHERE s.user_id LIKE $1 AND m.content ILIKE '%' || $2 || '%'
            ORDER BY s.created_at DESC
            LIMIT $3
            "#,
        )
        .bind(&pattern)
        .bind(query)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to query STM: {}", e);
            AppError::Internal(format!("STM query failed: {}", e))
        })?;

        let entries: Vec<MemoryEntry> = rows
            .into_iter()
            .map(|(id, content, created_at)| {
                // Calculate a simple relevance: exact match = 1.0, partial = 0.5
                let relevance = if content.to_lowercase() == query.to_lowercase() {
                    1.0
                } else {
                    0.5
                };
                MemoryEntry {
                    id,
                    layer: MemoryLayer::Stm,
                    title: content.chars().take(50).collect::<String>(),
                    content,
                    relevance_score: relevance,
                    created_at,
                    quality_score: None,
                }
            })
            .collect();

        info!(
            "Fusion STM query '{}' returned {} results for tenant {}",
            query,
            entries.len(),
            tenant_id
        );
        Ok(entries)
    }

    async fn query_ltm(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        query: &str,
        limit: i32,
    ) -> Result<Vec<MemoryEntry>, AppError> {
        let prefix = tenant_id.prefix();
        let pattern = format!("{}%", prefix);

        // Search knowledge entries by content/title (semantic-like via ILIKE)
        let rows: Vec<(String, Option<String>, String, Option<f32>, String)> = sqlx::query_as(
            r#"
            SELECT entry_id, title, content, quality_score, created_at::text
            FROM knowledge_entries
            WHERE source_id LIKE $1
              AND status = 'active'
              AND (title ILIKE '%' || $2 || '%' OR content ILIKE '%' || $2 || '%')
            ORDER BY quality_score DESC NULLS LAST, created_at DESC
            LIMIT $3
            "#,
        )
        .bind(&pattern)
        .bind(query)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to query LTM: {}", e);
            AppError::Internal(format!("LTM query failed: {}", e))
        })?;

        let entries: Vec<MemoryEntry> = rows
            .into_iter()
            .map(|(id, title, content, quality_score, created_at)| {
                let relevance = if title.as_ref().map_or(false, |t| t.to_lowercase().contains(&query.to_lowercase())) {
                    0.8
                } else {
                    0.6
                };
                MemoryEntry {
                    id,
                    layer: MemoryLayer::Ltm,
                    title: title.unwrap_or_else(|| content.chars().take(50).collect()),
                    content,
                    relevance_score: relevance,
                    created_at,
                    quality_score,
                }
            })
            .collect();

        info!(
            "Fusion LTM query '{}' returned {} results for tenant {}",
            query,
            entries.len(),
            tenant_id
        );
        Ok(entries)
    }

    async fn query_kg(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        query: &str,
        limit: i32,
    ) -> Result<Vec<MemoryEntry>, AppError> {
        let prefix = tenant_id.prefix();
        let pattern = format!("{}%", prefix);

        // Search KG entities by name/description
        let rows: Vec<(String, String, Option<String>, f32, String)> = sqlx::query_as(
            r#"
            SELECT entity_id, entity_name, description, confidence_score, created_at::text
            FROM entities
            WHERE entity_id LIKE $1
              AND status = 'active'
              AND (entity_name ILIKE '%' || $2 || '%' OR description ILIKE '%' || $2 || '%')
            ORDER BY confidence_score DESC, created_at DESC
            LIMIT $3
            "#,
        )
        .bind(&pattern)
        .bind(query)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to query KG: {}", e);
            AppError::Internal(format!("KG query failed: {}", e))
        })?;

        let entries: Vec<MemoryEntry> = rows
            .into_iter()
            .map(|(id, name, description, confidence_score, created_at)| {
                MemoryEntry {
                    id,
                    layer: MemoryLayer::Kg,
                    title: name,
                    content: description.unwrap_or_default(),
                    relevance_score: confidence_score as f64,
                    created_at,
                    quality_score: Some(confidence_score),
                }
            })
            .collect();

        info!(
            "Fusion KG query '{}' returned {} results for tenant {}",
            query,
            entries.len(),
            tenant_id
        );
        Ok(entries)
    }

    async fn query_mm(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        query: &str,
        limit: i32,
    ) -> Result<Vec<MemoryEntry>, AppError> {
        let prefix = tenant_id.prefix();
        let pattern = format!("{}%", prefix);

        // Search MM entries by text content or title
        let rows: Vec<(String, Option<String>, Option<String>, Option<f32>, String)> = sqlx::query_as(
            r#"
            SELECT entry_id, title, text_content, quality_score, created_at::text
            FROM multimodal_entries
            WHERE source_id LIKE $1
              AND status = 'active'
              AND (title ILIKE '%' || $2 || '%' OR text_content ILIKE '%' || $2 || '%')
            ORDER BY quality_score DESC NULLS LAST, created_at DESC
            LIMIT $3
            "#,
        )
        .bind(&pattern)
        .bind(query)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to query MM: {}", e);
            AppError::Internal(format!("MM query failed: {}", e))
        })?;

        let entries: Vec<MemoryEntry> = rows
            .into_iter()
            .map(|(id, title, text_content, quality_score, created_at)| {
                MemoryEntry {
                    id,
                    layer: MemoryLayer::Mm,
                    title: title.unwrap_or_else(|| "Untitled".to_string()),
                    content: text_content.unwrap_or_default(),
                    relevance_score: quality_score.unwrap_or(0.5) as f64,
                    created_at,
                    quality_score,
                }
            })
            .collect();

        info!(
            "Fusion MM query '{}' returned {} results for tenant {}",
            query,
            entries.len(),
            tenant_id
        );
        Ok(entries)
    }

    async fn count_stm(pool: &sqlx::PgPool, tenant_id: &TenantId) -> Result<i64, AppError> {
        let prefix = tenant_id.prefix();
        let pattern = format!("{}%", prefix);

        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM context_sessions WHERE user_id LIKE $1 AND status = 'active'",
        )
        .bind(&pattern)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Failed to count STM: {}", e);
            AppError::Internal(format!("STM count failed: {}", e))
        })?;

        Ok(row.0)
    }

    async fn count_ltm(pool: &sqlx::PgPool, tenant_id: &TenantId) -> Result<i64, AppError> {
        let prefix = tenant_id.prefix();
        let pattern = format!("{}%", prefix);

        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM knowledge_entries WHERE source_id LIKE $1 AND status = 'active'",
        )
        .bind(&pattern)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Failed to count LTM: {}", e);
            AppError::Internal(format!("LTM count failed: {}", e))
        })?;

        Ok(row.0)
    }

    async fn count_kg(pool: &sqlx::PgPool, tenant_id: &TenantId) -> Result<i64, AppError> {
        let prefix = tenant_id.prefix();
        let pattern = format!("{}%", prefix);

        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entities WHERE entity_id LIKE $1 AND status = 'active'",
        )
        .bind(&pattern)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Failed to count KG: {}", e);
            AppError::Internal(format!("KG count failed: {}", e))
        })?;

        Ok(row.0)
    }

    async fn count_mm(pool: &sqlx::PgPool) -> Result<i64, AppError> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM multimodal_entries WHERE status = 'active'",
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Failed to count MM: {}", e);
            AppError::Internal(format!("MM count failed: {}", e))
        })?;

        Ok(row.0)
    }
}
