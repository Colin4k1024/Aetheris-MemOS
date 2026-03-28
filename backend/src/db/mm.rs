use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use tracing::{error, info};
use ulid::Ulid;

use crate::db::pool;
use crate::AppError;

/// 多模态记忆仓库
pub struct MMRepository;

fn normalize_tenant_id(tenant_id: Option<&str>) -> Option<&str> {
    tenant_id.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn scope_prefixed_id(tenant_id: Option<&str>, value: &str, scope: Option<&str>) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return value.to_string();
    }

    match normalize_tenant_id(tenant_id) {
        Some(tenant_id) if trimmed.starts_with(&format!("t:{}:", tenant_id)) => trimmed.to_string(),
        Some(tenant_id) => match scope {
            Some(scope) => format!("t:{}:{}:{}", tenant_id, scope, trimmed),
            None => format!("t:{}:{}", tenant_id, trimmed),
        },
        None => trimmed.to_string(),
    }
}

fn merge_content_metadata(
    content_metadata: &str,
    tenant_id: Option<&str>,
) -> Result<String, AppError> {
    let mut metadata = serde_json::from_str::<Value>(content_metadata)
        .unwrap_or_else(|_| Value::Object(Default::default()));

    if !metadata.is_object() {
        metadata = Value::Object(Default::default());
    }

    if let Some(tenant_id) = normalize_tenant_id(tenant_id) {
        if let Some(map) = metadata.as_object_mut() {
            map.insert(
                "tenant_id".to_string(),
                Value::String(tenant_id.to_string()),
            );
        }
    }

    serde_json::to_string(&metadata).map_err(|e| {
        error!("Failed to serialize content metadata: {}", e);
        AppError::Internal(format!("Failed to serialize content metadata: {}", e))
    })
}

/// 多模态记忆条目列表响应
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MultimodalEntryListResponse {
    pub entries: Vec<MultimodalEntry>,
    pub total: usize,
    pub limit: i32,
    pub offset: i32,
}

/// 多模态记忆条目
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MultimodalEntry {
    pub entry_id: String,
    pub session_id: Option<String>,
    pub source_id: String,
    pub modality_type: String,
    pub modality_count: i32,
    pub title: Option<String>,
    pub description: Option<String>,
    pub content_metadata: String,
    pub text_content: Option<String>,
    pub text_embedding: Option<String>,
    pub image_url: Option<String>,
    pub image_embedding: Option<String>,
    pub image_features: Option<String>,
    pub audio_url: Option<String>,
    pub audio_embedding: Option<String>,
    pub audio_transcript: Option<String>,
    pub audio_features: Option<String>,
    pub video_url: Option<String>,
    pub video_embedding: Option<String>,
    pub video_transcript: Option<String>,
    pub video_features: Option<String>,
    pub cross_modal_alignment: Option<String>,
    pub unified_embedding: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub quality_score: f32,
    pub modality_consistency: f32,
    pub access_count: i32,
    pub success_count: i32,
    pub status: String,
}

/// 模态关联
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ModalityRelation {
    pub relation_id: String,
    pub source_entry_id: String,
    pub target_entry_id: String,
    pub relation_type: String,
    pub relation_strength: f64,
    pub relation_confidence: f64,
    pub created_at: String,
    pub metadata: Option<String>,
    pub description: Option<String>,
}

impl MMRepository {
    /// 创建多模态记忆条目
    pub async fn create_entry(
        session_id: Option<&str>,
        source_id: &str,
        modality_type: &str,
        content_metadata: &str,
        text_content: Option<&str>,
        image_url: Option<&str>,
        audio_url: Option<&str>,
        video_url: Option<&str>,
        tenant_id: Option<&str>,
    ) -> Result<String, AppError> {
        let entry_id = Ulid::new().to_string();
        let pool = pool();
        let scoped_session_id =
            session_id.map(|value| scope_prefixed_id(tenant_id, value, Some("session")));
        let scoped_source_id = scope_prefixed_id(tenant_id, source_id, None);
        let content_metadata = merge_content_metadata(content_metadata, tenant_id)?;

        sqlx::query(
            r#"
            INSERT INTO multimodal_entries (
                entry_id, session_id, source_id, modality_type, content_metadata,
                text_content, image_url, audio_url, video_url
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&entry_id)
        .bind(scoped_session_id)
        .bind(scoped_source_id)
        .bind(modality_type)
        .bind(content_metadata)
        .bind(text_content)
        .bind(image_url)
        .bind(audio_url)
        .bind(video_url)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create multimodal entry: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Created multimodal entry: entry_id={}", entry_id);
        Ok(entry_id)
    }

    /// 获取多模态记忆条目
    pub async fn get_entry_by_id(
        entry_id: &str,
        tenant_id: Option<&str>,
    ) -> Result<Option<MultimodalEntry>, AppError> {
        let pool = pool();
        let tenant_id = normalize_tenant_id(tenant_id);

        let entry = sqlx::query_as::<_, MultimodalEntry>(
            r#"
            SELECT entry_id, session_id, source_id, modality_type, modality_count,
                   title, description, content_metadata, text_content, text_embedding,
                   image_url, image_embedding, image_features, audio_url, audio_embedding,
                   audio_transcript, audio_features, video_url, video_embedding,
                   video_transcript, video_features, cross_modal_alignment, unified_embedding,
                   created_at, updated_at, quality_score, modality_consistency,
                   access_count, success_count, status
            FROM multimodal_entries
                        WHERE entry_id = $1
                            AND status = 'active'
                            AND ($2::text IS NULL OR COALESCE(NULLIF(content_metadata, ''), '{}')::jsonb ->> 'tenant_id' = $2)
            "#,
        )
        .bind(entry_id)
                .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to get multimodal entry: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(entry)
    }

    /// 更新多模态记忆条目
    pub async fn update_entry(
        entry_id: &str,
        title: Option<&str>,
        description: Option<&str>,
        text_embedding: Option<&str>,
        image_embedding: Option<&str>,
        audio_embedding: Option<&str>,
        video_embedding: Option<&str>,
        unified_embedding: Option<&str>,
    ) -> Result<(), AppError> {
        let pool = pool();

        sqlx::query(
            r#"
            UPDATE multimodal_entries
            SET title = COALESCE($1, title),
                description = COALESCE($2, description),
                text_embedding = COALESCE($3, text_embedding),
                image_embedding = COALESCE($4, image_embedding),
                audio_embedding = COALESCE($5, audio_embedding),
                video_embedding = COALESCE($6, video_embedding),
                unified_embedding = COALESCE($7, unified_embedding)
            WHERE entry_id = $8
            "#,
        )
        .bind(title)
        .bind(description)
        .bind(text_embedding)
        .bind(image_embedding)
        .bind(audio_embedding)
        .bind(video_embedding)
        .bind(unified_embedding)
        .bind(entry_id)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to update multimodal entry: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Updated multimodal entry: entry_id={}", entry_id);
        Ok(())
    }

    /// 根据会话ID获取多模态记忆条目
    pub async fn get_entries_by_session(
        session_id: &str,
        limit: Option<i32>,
        tenant_id: Option<&str>,
    ) -> Result<Vec<MultimodalEntry>, AppError> {
        let pool = pool();
        let scoped_session_id = scope_prefixed_id(tenant_id, session_id, Some("session"));

        let entries = sqlx::query_as::<_, MultimodalEntry>(
            r#"
            SELECT entry_id, session_id, source_id, modality_type, modality_count,
                   title, description, content_metadata, text_content, text_embedding,
                   image_url, image_embedding, image_features, audio_url, audio_embedding,
                   audio_transcript, audio_features, video_url, video_embedding,
                   video_transcript, video_features, cross_modal_alignment, unified_embedding,
                   created_at, updated_at, quality_score, modality_consistency,
                   access_count, success_count, status
            FROM multimodal_entries
            WHERE session_id = $1 AND status = 'active'
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(scoped_session_id)
        .bind(limit.unwrap_or(10))
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get multimodal entries by session: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!(
            "Retrieved {} multimodal entries for session: session_id={}",
            entries.len(),
            session_id
        );
        Ok(entries)
    }

    /// 根据模态类型获取多模态记忆条目
    pub async fn get_entries_by_modality(
        modality_type: &str,
        limit: Option<i32>,
        tenant_id: Option<&str>,
    ) -> Result<Vec<MultimodalEntry>, AppError> {
        let pool = pool();
        let tenant_id = normalize_tenant_id(tenant_id);

        let entries = sqlx::query_as::<_, MultimodalEntry>(
            r#"
            SELECT entry_id, session_id, source_id, modality_type, modality_count,
                   title, description, content_metadata, text_content, text_embedding,
                   image_url, image_embedding, image_features, audio_url, audio_embedding,
                   audio_transcript, audio_features, video_url, video_embedding,
                   video_transcript, video_features, cross_modal_alignment, unified_embedding,
                   created_at, updated_at, quality_score, modality_consistency,
                   access_count, success_count, status
            FROM multimodal_entries
                        WHERE modality_type = $1
                            AND status = 'active'
                            AND ($2::text IS NULL OR COALESCE(NULLIF(content_metadata, ''), '{}')::jsonb ->> 'tenant_id' = $2)
            ORDER BY created_at DESC
                        LIMIT $3
            "#,
        )
        .bind(modality_type)
                .bind(tenant_id)
        .bind(limit.unwrap_or(10))
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get multimodal entries by modality: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!(
            "Retrieved {} multimodal entries for modality: modality_type={}",
            entries.len(),
            modality_type
        );
        Ok(entries)
    }

    /// 创建模态关联
    pub async fn create_relation(
        source_entry_id: &str,
        target_entry_id: &str,
        relation_type: &str,
        relation_strength: f64,
        relation_confidence: f64,
        description: Option<&str>,
        tenant_id: Option<&str>,
    ) -> Result<String, AppError> {
        if Self::get_entry_by_id(source_entry_id, tenant_id)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound(format!(
                "Source entry {} not found",
                source_entry_id
            )));
        }

        if Self::get_entry_by_id(target_entry_id, tenant_id)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound(format!(
                "Target entry {} not found",
                target_entry_id
            )));
        }

        let relation_id = Ulid::new().to_string();
        let pool = pool();

        sqlx::query(
            r#"
            INSERT INTO modality_relations (
                relation_id, source_entry_id, target_entry_id, relation_type,
                relation_strength, relation_confidence, description
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&relation_id)
        .bind(source_entry_id)
        .bind(target_entry_id)
        .bind(relation_type)
        .bind(relation_strength)
        .bind(relation_confidence)
        .bind(description)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create modality relation: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Created modality relation: relation_id={}", relation_id);
        Ok(relation_id)
    }

    /// 获取相关联的多模态记忆条目
    pub async fn get_related_entries(
        entry_id: &str,
        limit: Option<i32>,
        tenant_id: Option<&str>,
    ) -> Result<Vec<(MultimodalEntry, ModalityRelation)>, AppError> {
        if Self::get_entry_by_id(entry_id, tenant_id).await?.is_none() {
            return Ok(Vec::new());
        }

        let pool = pool();

        let relations = sqlx::query_as::<_, ModalityRelation>(
            r#"
            SELECT relation_id, source_entry_id, target_entry_id, relation_type,
                   relation_strength, relation_confidence, created_at, metadata, description
            FROM modality_relations
            WHERE source_entry_id = $1 OR target_entry_id = $2
            ORDER BY relation_strength DESC, relation_confidence DESC
            LIMIT $3
            "#,
        )
        .bind(entry_id)
        .bind(entry_id)
        .bind(limit.unwrap_or(5))
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get modality relations: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        let mut result = Vec::new();
        for relation in relations {
            // 获取目标条目
            let target_id = if relation.source_entry_id == entry_id {
                &relation.target_entry_id
            } else {
                &relation.source_entry_id
            };

            if let Some(entry) = Self::get_entry_by_id(target_id, tenant_id).await? {
                result.push((entry, relation));
            }
        }

        info!(
            "Retrieved {} related multimodal entries: entry_id={}",
            result.len(),
            entry_id
        );
        Ok(result)
    }

    /// 获取多模态记忆条目总数
    pub async fn count(pool: &sqlx::PgPool) -> Result<i64, AppError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM multimodal_entries")
            .fetch_one(pool)
            .await
            .map_err(|e| {
                error!("Failed to count multimodal entries: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;

        Ok(row.0)
    }

    /// 获取多模态记忆条目列表
    pub async fn list_entries(
        modality_type: Option<&str>,
        limit: Option<i32>,
        offset: Option<i32>,
        tenant_id: Option<&str>,
    ) -> Result<MultimodalEntryListResponse, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(20);
        let offset = offset.unwrap_or(0);
        let tenant_id = normalize_tenant_id(tenant_id);

        let (entries, total): (Vec<MultimodalEntry>, (i64,)) = if let Some(mt) = modality_type {
            let entries = sqlx::query_as::<_, MultimodalEntry>(
                r#"
                SELECT entry_id, session_id, source_id, modality_type, modality_count,
                       title, description, content_metadata, text_content, text_embedding,
                       image_url, image_embedding, image_features,
                       audio_url, audio_embedding, audio_transcript, audio_features,
                       video_url, video_embedding, video_transcript, video_features,
                       cross_modal_alignment, unified_embedding,
                       created_at::text as created_at, updated_at::text as updated_at,
                       quality_score, modality_consistency, access_count, success_count, status
                FROM multimodal_entries
                                WHERE status = 'active'
                                    AND modality_type = $1
                                    AND ($2::text IS NULL OR COALESCE(NULLIF(content_metadata, ''), '{}')::jsonb ->> 'tenant_id' = $2)
                ORDER BY created_at DESC
                                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(mt)
                        .bind(tenant_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                error!("Failed to list multimodal entries: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;

            let total = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM multimodal_entries WHERE status = 'active' AND modality_type = $1 AND ($2::text IS NULL OR COALESCE(NULLIF(content_metadata, ''), '{}')::jsonb ->> 'tenant_id' = $2)",
            )
            .bind(mt)
            .bind(tenant_id)
            .fetch_one(pool)
            .await
            .map_err(|e| {
                error!("Failed to count multimodal entries: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;
            (entries, total)
        } else {
            let entries = sqlx::query_as::<_, MultimodalEntry>(
                r#"
                SELECT entry_id, session_id, source_id, modality_type, modality_count,
                       title, description, content_metadata, text_content, text_embedding,
                       image_url, image_embedding, image_features,
                       audio_url, audio_embedding, audio_transcript, audio_features,
                       video_url, video_embedding, video_transcript, video_features,
                       cross_modal_alignment, unified_embedding,
                       created_at::text as created_at, updated_at::text as updated_at,
                       quality_score, modality_consistency, access_count, success_count, status
                FROM multimodal_entries
                                WHERE status = 'active'
                                    AND ($1::text IS NULL OR COALESCE(NULLIF(content_metadata, ''), '{}')::jsonb ->> 'tenant_id' = $1)
                ORDER BY created_at DESC
                                LIMIT $2 OFFSET $3
                "#,
            )
                        .bind(tenant_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                error!("Failed to list multimodal entries: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;

            let total = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM multimodal_entries WHERE status = 'active' AND ($1::text IS NULL OR COALESCE(NULLIF(content_metadata, ''), '{}')::jsonb ->> 'tenant_id' = $1)",
            )
            .bind(tenant_id)
            .fetch_one(pool)
            .await
            .map_err(|e| {
                error!("Failed to count multimodal entries: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;
            (entries, total)
        };

        Ok(MultimodalEntryListResponse {
            entries,
            total: total.0 as usize,
            limit,
            offset,
        })
    }
}
