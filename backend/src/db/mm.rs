use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::{error, info};
use ulid::Ulid;

use crate::AppError;
use crate::db::pool;

/// 多模态记忆仓库
pub struct MMRepository;

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
    pub quality_score: f64,
    pub modality_consistency: f64,
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
    ) -> Result<String, AppError> {
        let entry_id = Ulid::new().to_string();
        let pool = pool();

        sqlx::query(
            r#"
            INSERT INTO multimodal_entries (
                entry_id, session_id, source_id, modality_type, content_metadata,
                text_content, image_url, audio_url, video_url
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&entry_id)
        .bind(session_id)
        .bind(source_id)
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
    pub async fn get_entry_by_id(entry_id: &str) -> Result<Option<MultimodalEntry>, AppError> {
        let pool = pool();

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
            WHERE entry_id = $1 AND status = 'active'
            "#,
        )
        .bind(entry_id)
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
    pub async fn get_entries_by_session(session_id: &str, limit: Option<i32>) -> Result<Vec<MultimodalEntry>, AppError> {
        let pool = pool();

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
        .bind(session_id)
        .bind(limit.unwrap_or(10))
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get multimodal entries by session: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Retrieved {} multimodal entries for session: session_id={}", entries.len(), session_id);
        Ok(entries)
    }

    /// 根据模态类型获取多模态记忆条目
    pub async fn get_entries_by_modality(modality_type: &str, limit: Option<i32>) -> Result<Vec<MultimodalEntry>, AppError> {
        let pool = pool();

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
            WHERE modality_type = $1 AND status = 'active'
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(modality_type)
        .bind(limit.unwrap_or(10))
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get multimodal entries by modality: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Retrieved {} multimodal entries for modality: modality_type={}", entries.len(), modality_type);
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
    ) -> Result<String, AppError> {
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
    pub async fn get_related_entries(entry_id: &str, limit: Option<i32>) -> Result<Vec<(MultimodalEntry, ModalityRelation)>, AppError> {
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

            if let Some(entry) = Self::get_entry_by_id(target_id).await? {
                result.push((entry, relation));
            }
        }

        info!("Retrieved {} related multimodal entries: entry_id={}", result.len(), entry_id);
        Ok(result)
    }
}
