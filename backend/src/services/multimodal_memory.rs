use anyhow::Result;
use tracing::{info, instrument};

use crate::db::mm::MMRepository;
use crate::services::embedding::get_embedding_service;

/// 多模态记忆服务
pub struct MultimodalMemoryService;

impl MultimodalMemoryService {
    /// 存储多模态记忆
    #[instrument]
    pub async fn store_multimodal_memory(
        session_id: Option<&str>,
        source_id: &str,
        modality_type: &str,
        content_metadata: &str,
        text_content: Option<&str>,
        image_url: Option<&str>,
        audio_url: Option<&str>,
        video_url: Option<&str>,
    ) -> Result<String, crate::AppError> {
        info!(
            "Storing multimodal memory: modality_type={}, source_id={}",
            modality_type, source_id
        );

        // 创建多模态记忆条目
        let entry_id = MMRepository::create_entry(
            session_id,
            source_id,
            modality_type,
            content_metadata,
            text_content,
            image_url,
            audio_url,
            video_url,
        )
        .await?;

        // 如果有文本内容，生成嵌入
        if let Some(text) = text_content {
            info!("Generating text embedding for multimodal entry: entry_id={}", entry_id);
            
            let embedding_service = get_embedding_service()
                .map_err(|e| crate::AppError::Internal(format!("Failed to get embedding service: {}", e)))?;
            
            let text_embedding = embedding_service.generate_embedding(text).await?;
            let text_embedding_json = serde_json::to_string(&text_embedding)
                .map_err(|e| crate::AppError::Internal(format!("Failed to serialize embedding: {}", e)))?;
            
            // 更新多模态记忆条目，添加文本嵌入
            MMRepository::update_entry(
                &entry_id,
                None,
                None,
                Some(&text_embedding_json),
                None,
                None,
                None,
                None,
            )
            .await?;
        }

        info!("Multimodal memory stored successfully: entry_id={}", entry_id);
        Ok(entry_id)
    }

    /// 根据ID获取多模态记忆
    #[instrument]
    pub async fn get_multimodal_memory(
        entry_id: &str,
    ) -> Result<Option<crate::db::mm::MultimodalEntry>, crate::AppError> {
        info!("Getting multimodal memory: entry_id={}", entry_id);
        
        let entry = MMRepository::get_entry_by_id(entry_id).await?;
        
        if let Some(e) = &entry {
            info!("Retrieved multimodal memory: entry_id={}, modality_type={}", entry_id, e.modality_type);
        } else {
            info!("Multimodal memory not found: entry_id={}", entry_id);
        }
        
        Ok(entry)
    }

    /// 根据会话ID获取多模态记忆
    #[instrument]
    pub async fn get_multimodal_memories_by_session(
        session_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<crate::db::mm::MultimodalEntry>, crate::AppError> {
        info!("Getting multimodal memories by session: session_id={}", session_id);
        
        let entries = MMRepository::get_entries_by_session(session_id, limit).await?;
        
        info!("Retrieved {} multimodal memories for session: session_id={}", entries.len(), session_id);
        Ok(entries)
    }

    /// 根据模态类型获取多模态记忆
    #[instrument]
    pub async fn get_multimodal_memories_by_modality(
        modality_type: &str,
        limit: Option<i32>,
    ) -> Result<Vec<crate::db::mm::MultimodalEntry>, crate::AppError> {
        info!("Getting multimodal memories by modality: modality_type={}", modality_type);
        
        let entries = MMRepository::get_entries_by_modality(modality_type, limit).await?;
        
        info!("Retrieved {} multimodal memories for modality: modality_type={}", entries.len(), modality_type);
        Ok(entries)
    }

    /// 获取相关联的多模态记忆
    #[instrument]
    pub async fn get_related_multimodal_memories(
        entry_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<(crate::db::mm::MultimodalEntry, crate::db::mm::ModalityRelation)>, crate::AppError> {
        info!("Getting related multimodal memories: entry_id={}", entry_id);
        
        let related_entries = MMRepository::get_related_entries(entry_id, limit).await?;
        
        info!("Retrieved {} related multimodal memories: entry_id={}", related_entries.len(), entry_id);
        Ok(related_entries)
    }

    /// 创建多模态记忆关联
    #[instrument]
    pub async fn create_multimodal_relation(
        source_entry_id: &str,
        target_entry_id: &str,
        relation_type: &str,
        relation_strength: f64,
        relation_confidence: f64,
        description: Option<&str>,
    ) -> Result<String, crate::AppError> {
        info!(
            "Creating multimodal relation: source={}, target={}, type={}",
            source_entry_id, target_entry_id, relation_type
        );
        
        let relation_id = MMRepository::create_relation(
            source_entry_id,
            target_entry_id,
            relation_type,
            relation_strength,
            relation_confidence,
            description,
        )
        .await?;
        
        info!("Created multimodal relation: relation_id={}", relation_id);
        Ok(relation_id)
    }
}
