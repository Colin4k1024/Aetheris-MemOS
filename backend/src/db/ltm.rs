#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::{error, info};
use ulid::Ulid;

use crate::db::pool;
use crate::AppError;

/// 长期记忆仓库
pub struct LTMRepository;

/// 知识条目列表响应
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct KnowledgeEntryListResponse {
    pub entries: Vec<KnowledgeEntry>,
    pub total: usize,
    pub limit: i32,
    pub offset: i32,
}

/// 知识条目
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct KnowledgeEntry {
    pub entry_id: String,
    pub source_id: String,
    pub source_type: String,
    pub title: Option<String>,
    pub content: String,
    pub content_type: String,
    pub content_hash: String,
    pub embedding_vector: String,
    pub embedding_model: String,
    pub embedding_dimension: i32,
    pub created_at: String,
    pub updated_at: String,
    pub last_accessed_at: Option<String>,
    pub category: Option<String>,
    pub domain: Option<String>,
    pub quality_score: Option<f32>,
    pub relevance_score: Option<f32>,
    pub status: String,
    pub access_count: Option<i32>,
}

impl LTMRepository {
    /// 创建知识条目
    pub async fn create_knowledge_entry(
        source_id: &str,
        source_type: &str,
        title: Option<&str>,
        content: &str,
        content_type: &str,
        embedding_vector: &[f32],
        embedding_model: &str,
        embedding_dimension: i32,
        quality_score: Option<f64>,
    ) -> Result<String, AppError> {
        Self::create_knowledge_entry_with_id(
            None,
            source_id,
            source_type,
            title,
            content,
            content_type,
            embedding_vector,
            embedding_model,
            embedding_dimension,
            quality_score,
        )
        .await
    }

    /// 创建知识条目（使用指定的 entry_id）
    pub async fn create_knowledge_entry_with_id(
        entry_id: Option<String>,
        source_id: &str,
        source_type: &str,
        title: Option<&str>,
        content: &str,
        content_type: &str,
        embedding_vector: &[f32],
        embedding_model: &str,
        embedding_dimension: i32,
        quality_score: Option<f64>,
    ) -> Result<String, AppError> {
        let entry_id = entry_id.unwrap_or_else(|| Ulid::new().to_string());
        let pool = pool();

        // 计算内容哈希
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let content_hash = format!("{:x}", hasher.finish());

        // 将向量转换为 JSON 字符串
        let embedding_json = serde_json::to_string(embedding_vector).map_err(|e| {
            error!("Failed to serialize embedding vector: {}", e);
            AppError::Internal(format!("Failed to serialize embedding: {}", e))
        })?;

        sqlx::query(
            r#"
            INSERT INTO knowledge_entries (
                entry_id, source_id, source_type, title, content, content_type, content_hash,
                embedding_vector, embedding_model, embedding_dimension,
                quality_score, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'active')
            "#,
        )
        .bind(&entry_id)
        .bind(source_id)
        .bind(source_type)
        .bind(title)
        .bind(content)
        .bind(content_type)
        .bind(&content_hash)
        .bind(&embedding_json)
        .bind(embedding_model)
        .bind(embedding_dimension)
        .bind(quality_score)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create knowledge entry: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Created new knowledge entry: {}", entry_id);
        Ok(entry_id)
    }

    /// 更新条目
    pub async fn update_entry(
        entry_id: &str,
        title: Option<&str>,
        content: Option<&str>,
        quality_score: Option<f64>,
    ) -> Result<(), AppError> {
        let pool = pool();

        sqlx::query(
            r#"
            UPDATE knowledge_entries
            SET title = COALESCE($1, title),
                content = COALESCE($2, content),
                quality_score = COALESCE($3, quality_score),
                updated_at = CURRENT_TIMESTAMP
            WHERE entry_id = $4
            "#,
        )
        .bind(title)
        .bind(content)
        .bind(quality_score)
        .bind(entry_id)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to update knowledge entry: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Updated knowledge entry: {}", entry_id);
        Ok(())
    }

    /// 根据 ID 获取条目
    pub async fn get_entry_by_id(entry_id: &str) -> Result<Option<KnowledgeEntry>, AppError> {
        let pool = pool();

        let entry = sqlx::query_as::<_, KnowledgeEntry>(
            r#"
            SELECT entry_id, source_id, source_type, title, content, content_type, content_hash,
                   embedding_vector, embedding_model, embedding_dimension,
                   created_at::text as created_at, updated_at::text as updated_at,
                   last_accessed_at::text as last_accessed_at,
                   category, domain,
                   quality_score, relevance_score, status,
                   COALESCE(access_count, 0) as access_count
            FROM knowledge_entries
            WHERE entry_id = $1 AND status = 'active'
            "#,
        )
        .bind(entry_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to get knowledge entry: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(entry)
    }

    /// 根据来源获取条目
    pub async fn get_entries_by_source(
        source_id: &str,
        source_type: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<KnowledgeEntry>, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(100);

        let query = if let Some(st) = source_type {
            sqlx::query_as::<_, KnowledgeEntry>(
                r#"
                SELECT entry_id, source_id, source_type, title, content, content_type, content_hash,
                       embedding_vector, embedding_model, embedding_dimension,
                       created_at::text as created_at, updated_at::text as updated_at,
                       last_accessed_at::text as last_accessed_at,
                       category, domain,
                       quality_score, relevance_score, status,
                       COALESCE(access_count, 0) as access_count
                FROM knowledge_entries
                WHERE source_id = $1 AND source_type = $2 AND status = 'active'
                ORDER BY created_at DESC
                LIMIT $3
                "#,
            )
            .bind(source_id)
            .bind(st)
            .bind(limit)
        } else {
            sqlx::query_as::<_, KnowledgeEntry>(
                r#"
                SELECT entry_id, source_id, source_type, title, content, content_type, content_hash,
                       embedding_vector, embedding_model, embedding_dimension,
                       created_at::text as created_at, updated_at::text as updated_at,
                       last_accessed_at::text as last_accessed_at,
                       category, domain,
                       quality_score, relevance_score, status,
                       COALESCE(access_count, 0) as access_count
                FROM knowledge_entries
                WHERE source_id = $1 AND status = 'active'
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(source_id)
            .bind(limit)
        };

        let entries = query.fetch_all(pool).await.map_err(|e| {
            error!("Failed to get entries by source: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(entries)
    }

    /// 获取所有知识条目列表
    pub async fn list_entries(
        _category: Option<&str>,
        _status: Option<&str>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<KnowledgeEntryListResponse, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(20);
        let offset = offset.unwrap_or(0);

        let entries: Vec<KnowledgeEntry> = sqlx::query_as(
            r#"
            SELECT entry_id, source_id, source_type, title, content, content_type, content_hash,
                   embedding_vector, embedding_model, embedding_dimension,
                   created_at::text as created_at, updated_at::text as updated_at,
                   last_accessed_at::text as last_accessed_at,
                   category, domain, quality_score, relevance_score, status,
                   COALESCE(access_count, 0) as access_count
            FROM knowledge_entries
            WHERE status = 'active'
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to list knowledge entries: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        let total: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM knowledge_entries WHERE status = 'active'")
                .fetch_one(pool)
                .await
                .map_err(|e| {
                    error!("Failed to count knowledge entries: {}", e);
                    AppError::Internal(format!("Database error: {}", e))
                })?;

        Ok(KnowledgeEntryListResponse {
            entries,
            total: total.0 as usize,
            limit,
            offset,
        })
    }

    /// 获取长期记忆条目总数
    pub async fn count(pool: &sqlx::PgPool) -> Result<i64, AppError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM knowledge_entries")
            .fetch_one(pool)
            .await
            .map_err(|e| {
                error!("Failed to count knowledge entries: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;

        Ok(row.0)
    }
}
