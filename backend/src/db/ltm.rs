use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::{error, info};
use ulid::Ulid;

use crate::db::pool;
use crate::tenant::TenantId;
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
    pub version: Option<i32>,
    // Bi-temporal fields
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
    pub superseded_by: Option<String>,
}

impl LTMRepository {
    /// 创建知识条目（租户隔离）
    pub async fn create_knowledge_entry(
        tenant_id: &TenantId,
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
            tenant_id,
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

    /// 创建知识条目（使用指定的 entry_id，租户隔离）
    pub async fn create_knowledge_entry_with_id(
        entry_id: Option<String>,
        tenant_id: &TenantId,
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

        // 计算内容哈希（使用 SHA-256 确保可移植性和完整性验证，Issue #58）
        let content_hash = crate::services::information_guard::compute_sha256(content);

        // 将向量转换为 JSON 字符串
        let embedding_json = serde_json::to_string(embedding_vector).map_err(|e| {
            error!("Failed to serialize embedding vector: {}", e);
            AppError::Internal(format!("Failed to serialize embedding: {}", e))
        })?;

        // 构建租户限定的source_id用于跨租户隔离
        let tenant_source_id = format!("{}:{}", tenant_id.prefix(), source_id);

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
        .bind(tenant_source_id)
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

        info!(
            "Created new knowledge entry: {} for tenant: {}",
            entry_id, tenant_id
        );
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

    /// Soft-delete a knowledge entry by marking it deprecated.
    pub async fn soft_delete_entry(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        entry_id: &str,
    ) -> Result<bool, AppError> {
        let entry = Self::get_entry_by_id(pool, tenant_id, entry_id).await?;
        if entry.is_none() {
            return Ok(false);
        }

        let result = sqlx::query(
            r#"
            UPDATE knowledge_entries
            SET status = 'deprecated',
                updated_at = CURRENT_TIMESTAMP
            WHERE entry_id = $1 AND status = 'active'
            "#,
        )
        .bind(entry_id)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to soft-delete knowledge entry: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!(
            "Soft-deleted knowledge entry: {} for tenant: {}",
            entry_id, tenant_id
        );
        Ok(result.rows_affected() > 0)
    }

    /// 根据 ID 获取条目（租户隔离）
    pub async fn get_entry_by_id(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        entry_id: &str,
    ) -> Result<Option<KnowledgeEntry>, AppError> {
        let entry = sqlx::query_as::<_, KnowledgeEntry>(
            r#"
            SELECT entry_id, source_id, source_type, title, content, content_type, content_hash,
                   embedding_vector, embedding_model, embedding_dimension,
                   created_at::text as created_at, updated_at::text as updated_at,
                   last_accessed_at::text as last_accessed_at,
                   category, domain,
                   quality_score, relevance_score, status,
                   COALESCE(access_count, 0) as access_count,
                   version,
                   valid_from::text as valid_from,
                   valid_until::text as valid_until,
                   superseded_by
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

        // 验证条目属于该租户
        if let Some(ref e) = entry {
            let prefix = tenant_id.prefix();
            if !e.source_id.starts_with(&prefix) {
                // 跨租户访问尝试 - 记录违规但返回None（不泄露数据存在性）
                crate::services::multi_tenant::record_isolation_violation(
                    tenant_id.as_str(),
                    entry_id,
                    "ltm_entry_cross_tenant_read",
                );
                return Ok(None);
            }
        }

        Ok(entry)
    }

    /// 根据来源获取条目（租户隔离）
    pub async fn get_entries_by_source(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        source_id: &str,
        source_type: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<KnowledgeEntry>, AppError> {
        let limit = limit.unwrap_or(100);
        let prefix = tenant_id.prefix();
        let tenant_source_pattern = format!("{}:{}%", prefix, source_id);

        let query = if let Some(st) = source_type {
            sqlx::query_as::<_, KnowledgeEntry>(
                r#"
                SELECT entry_id, source_id, source_type, title, content, content_type, content_hash,
                       embedding_vector, embedding_model, embedding_dimension,
                       created_at::text as created_at, updated_at::text as updated_at,
                       last_accessed_at::text as last_accessed_at,
                       category, domain,
                       quality_score, relevance_score, status,
                       COALESCE(access_count, 0) as access_count,
                       version,
                       valid_from::text as valid_from,
                       valid_until::text as valid_until,
                       superseded_by
                FROM knowledge_entries
                WHERE source_id LIKE $1 AND source_type = $2 AND status = 'active'
                ORDER BY created_at DESC
                LIMIT $3
                "#,
            )
            .bind(tenant_source_pattern)
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
                       COALESCE(access_count, 0) as access_count,
                       version,
                       valid_from::text as valid_from,
                       valid_until::text as valid_until,
                       superseded_by
                FROM knowledge_entries
                WHERE source_id LIKE $1 AND status = 'active'
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(tenant_source_pattern)
            .bind(limit)
        };

        let entries = query.fetch_all(pool).await.map_err(|e| {
            error!("Failed to get entries by source: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(entries)
    }

    /// 获取所有知识条目列表（租户隔离）
    pub async fn list_entries(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        _category: Option<&str>,
        _status: Option<&str>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<KnowledgeEntryListResponse, AppError> {
        let limit = limit.unwrap_or(20);
        let offset = offset.unwrap_or(0);

        let prefix = tenant_id.prefix();
        let tenant_source_pattern = format!("{}%", prefix);
        // Clone for second query since String doesn't implement Copy
        let tenant_source_pattern_clone = tenant_source_pattern.clone();

        let entries: Vec<KnowledgeEntry> = sqlx::query_as(
            r#"
            SELECT entry_id, source_id, source_type, title, content, content_type, content_hash,
                   embedding_vector, embedding_model, embedding_dimension,
                   created_at::text as created_at, updated_at::text as updated_at,
                   last_accessed_at::text as last_accessed_at,
                   category, domain, quality_score, relevance_score, status,
                   COALESCE(access_count, 0) as access_count,
                   version,
                   valid_from::text as valid_from,
                   valid_until::text as valid_until,
                   superseded_by
            FROM knowledge_entries
            WHERE source_id LIKE $1 AND status = 'active'
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(tenant_source_pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to list knowledge entries: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM knowledge_entries WHERE source_id LIKE $1 AND status = 'active'",
        )
        .bind(tenant_source_pattern_clone)
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

    /// 获取长期记忆条目总数（租户隔离）
    pub async fn count(pool: &sqlx::PgPool, tenant_id: &TenantId) -> Result<i64, AppError> {
        let prefix = tenant_id.prefix();
        let tenant_pattern = format!("{}%", prefix);

        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM knowledge_entries WHERE source_id LIKE $1")
                .bind(tenant_pattern)
                .fetch_one(pool)
                .await
                .map_err(|e| {
                    error!("Failed to count knowledge entries: {}", e);
                    AppError::Internal(format!("Database error: {}", e))
                })?;

        Ok(row.0)
    }

    // ============ Bi-temporal Tracking Methods (租户隔离) ============

    /// 获取特定时间点的知识条目（时间旅行查询，租户隔离）
    pub async fn get_entry_at_time(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        entry_id: &str,
        at_timestamp: &str,
    ) -> Result<Option<KnowledgeEntry>, AppError> {
        let entry = sqlx::query_as::<_, KnowledgeEntry>(
            r#"
            SELECT entry_id, source_id, source_type, title, content, content_type, content_hash,
                   embedding_vector, embedding_model, embedding_dimension,
                   created_at::text as created_at, updated_at::text as updated_at,
                   last_accessed_at::text as last_accessed_at,
                   category, domain,
                   quality_score, relevance_score, status,
                   COALESCE(access_count, 0) as access_count,
                   valid_from::text as valid_from,
                   valid_until::text as valid_until,
                   superseded_by
            FROM knowledge_entries
            WHERE entry_id = $1
              AND valid_from <= $2
              AND (valid_until IS NULL OR valid_until > $2)
            "#,
        )
        .bind(entry_id)
        .bind(at_timestamp)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to get knowledge entry at time: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        // 验证条目属于该租户
        if let Some(ref e) = entry {
            let prefix = tenant_id.prefix();
            if !e.source_id.starts_with(&prefix) {
                crate::services::multi_tenant::record_isolation_violation(
                    tenant_id.as_str(),
                    entry_id,
                    "ltm_entry_at_time_cross_tenant_access",
                );
                return Ok(None);
            }
        }

        Ok(entry)
    }

    /// 搜索特定时间点的知识条目（租户隔离）
    pub async fn search_entries_at_time(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        query: &str,
        at_timestamp: &str,
        limit: Option<i32>,
    ) -> Result<Vec<KnowledgeEntry>, AppError> {
        let limit = limit.unwrap_or(20);
        let prefix = tenant_id.prefix();
        let tenant_pattern = format!("{}%", prefix);

        let entries = sqlx::query_as::<_, KnowledgeEntry>(
            r#"
            SELECT entry_id, source_id, source_type, title, content, content_type, content_hash,
                   embedding_vector, embedding_model, embedding_dimension,
                   created_at::text as created_at, updated_at::text as updated_at,
                   last_accessed_at::text as last_accessed_at,
                   category, domain,
                   quality_score, relevance_score, status,
                   COALESCE(access_count, 0) as access_count,
                   valid_from::text as valid_from,
                   valid_until::text as valid_until,
                   superseded_by
            FROM knowledge_entries
            WHERE source_id LIKE $1
              AND status = 'active'
              AND valid_from <= $2
              AND (valid_until IS NULL OR valid_until > $2)
              AND (
                  title ILIKE '%' || $3 || '%'
                  OR content ILIKE '%' || $3 || '%'
                  OR category ILIKE '%' || $3 || '%'
                  OR domain ILIKE '%' || $3 || '%'
              )
            ORDER BY quality_score DESC, relevance_score DESC
            LIMIT $4
            "#,
        )
        .bind(tenant_pattern)
        .bind(at_timestamp)
        .bind(query)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to search knowledge entries at time: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(entries)
    }

    /// 获取条目的版本历史
    pub async fn get_entry_history(entry_id: &str) -> Result<Vec<KnowledgeEntry>, AppError> {
        let pool = pool();

        let entries = sqlx::query_as::<_, KnowledgeEntry>(
            r#"
            SELECT entry_id, source_id, source_type, title, content, content_type, content_hash,
                   embedding_vector, embedding_model, embedding_dimension,
                   created_at::text as created_at, updated_at::text as updated_at,
                   last_accessed_at::text as last_accessed_at,
                   category, domain,
                   quality_score, relevance_score, status,
                   COALESCE(access_count, 0) as access_count,
                   valid_from::text as valid_from,
                   valid_until::text as valid_until,
                   superseded_by
            FROM knowledge_entries
            WHERE entry_id = $1
            ORDER BY version DESC, valid_from DESC
            "#,
        )
        .bind(entry_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get entry history: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(entries)
    }

    /// 更新条目时创建新版本（保留历史，租户隔离）
    pub async fn supersede_entry(
        tenant_id: &TenantId,
        entry_id: &str,
        new_content: &str,
        new_title: Option<&str>,
        change_reason: Option<&str>,
    ) -> Result<String, AppError> {
        let pool = pool();
        let new_entry_id = Ulid::new().to_string();

        // 获取当前条目（租户隔离）
        let current = Self::get_entry_by_id(&pool, tenant_id, entry_id).await?;
        if current.is_none() {
            return Err(AppError::NotFound(format!("Entry {} not found", entry_id)));
        }
        let current = current.unwrap();

        // 计算内容哈希（SHA-256，Issue #58）
        let content_hash = crate::services::information_guard::compute_sha256(new_content);

        // 将当前条目标记为被替换
        sqlx::query(
            r#"
            UPDATE knowledge_entries
            SET valid_until = CURRENT_TIMESTAMP,
                superseded_by = $1,
                status = 'deprecated'
            WHERE entry_id = $2
            "#,
        )
        .bind(&new_entry_id)
        .bind(entry_id)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to supersede entry: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        // 创建新版本条目
        sqlx::query(
            r#"
            INSERT INTO knowledge_entries (
                entry_id, source_id, source_type, title, content, content_type, content_hash,
                embedding_vector, embedding_model, embedding_dimension,
                quality_score, status, version,
                valid_from, superseded_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'active', $12, CURRENT_TIMESTAMP, NULL)
            "#,
        )
        .bind(&new_entry_id)
        .bind(&current.source_id)
        .bind(&current.source_type)
        .bind(new_title)
        .bind(new_content)
        .bind(&current.content_type)
        .bind(&content_hash)
        .bind(&current.embedding_vector)
        .bind(&current.embedding_model)
        .bind(current.embedding_dimension)
        .bind(current.quality_score)
        .bind(current.version.unwrap_or(1) + 1)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create new version: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!(
            "Superseded entry {} with new version {}, reason: {:?}",
            entry_id, new_entry_id, change_reason
        );
        Ok(new_entry_id)
    }
}
