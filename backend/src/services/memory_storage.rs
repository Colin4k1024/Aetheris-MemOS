use anyhow::Result;
use tracing::{error, info, instrument, warn};

use crate::db::{ltm::LTMRepository, stm::STMRepository};
use crate::services::{embedding::get_embedding_service, llm::get_llm_service, qdrant::get_qdrant_client};
use crate::AppError;

/// 记忆存储服务
pub struct MemoryStorageService;

impl MemoryStorageService {
    /// 存储短期记忆
    #[instrument]
    pub async fn store_stm(
        user_id: &str,
        agent_id: &str,
        session_type: &str,
        role: &str,
        content: &str,
        max_context_length: i32,
        retention_hours: i32,
    ) -> Result<(String, String), AppError> {
        info!(
            "Storing STM: user_id={}, agent_id={}, session_type={}",
            user_id, agent_id, session_type
        );

        // 创建或获取会话
        let session_id = STMRepository::create_session(
            user_id,
            agent_id,
            session_type,
            max_context_length,
            retention_hours,
        )
        .await?;

        // 添加消息到会话
        let message_id = STMRepository::add_message(
            &session_id,
            role,
            content,
            None, // token_count
            None, // importance_score
        )
        .await?;

        info!("STM stored successfully: session_id={}, message_id={}", session_id, message_id);
        Ok((session_id, message_id))
    }

    /// 存储长期记忆（调用 LLM 总结 + 向量化 + 存储到 Qdrant）
    #[instrument]
    pub async fn store_ltm(
        source_id: &str,
        source_type: &str,
        content: &str,
        title: Option<&str>,
    ) -> Result<String, AppError> {
        // 验证和规范化 source_type
        // 数据库约束只允许：'document', 'api', 'database', 'web', 'user_input'
        let normalized_source_type = match source_type {
            "document" | "api" | "database" | "web" | "user_input" => source_type,
            "test" | "testing" => {
                warn!("source_type '{}' is not allowed, mapping to 'user_input'", source_type);
                "user_input"
            }
            _ => {
                warn!("Unknown source_type '{}', mapping to 'user_input'", source_type);
                "user_input"
            }
        };
        
        info!(
            "Storing LTM: source_id={}, source_type={} (normalized from {}), content_length={}",
            source_id,
            normalized_source_type,
            source_type,
            content.len()
        );

        // 1. 调用 LLM 进行总结和结构化提取
        let llm_service = get_llm_service()
            .map_err(|e| AppError::Internal(format!("Failed to get LLM service: {}", e)))?;
        
        let extraction = llm_service
            .summarize_and_extract(content)
            .await
            .map_err(|e| {
                error!("LLM summarization failed: {}", e);
                AppError::Internal(format!("LLM summarization failed: {}", e))
            })?;

        info!("LLM extraction completed: entities={}, relations={}", 
              extraction.entities.len(), extraction.relations.len());

        // 2. 生成向量嵌入
        let embedding_service = get_embedding_service()
            .map_err(|e| AppError::Internal(format!("Failed to get embedding service: {}", e)))?;
        
        let embedding = embedding_service
            .generate_embedding(&extraction.summary)
            .await
            .map_err(|e| {
                error!("Embedding generation failed: {}", e);
                AppError::Internal(format!("Embedding generation failed: {}", e))
            })?;

        info!("Embedding generated: dimension={}", embedding.len());

        // 3. 存储到 Qdrant
        let qdrant_client = get_qdrant_client()
            .map_err(|e| AppError::Internal(format!("Failed to get Qdrant client: {}", e)))?;
        
        let entry_id = ulid::Ulid::new().to_string();
        let metadata = serde_json::json!({
            "title": title,
            "summary": extraction.summary.clone(),
            "entities": extraction.entities.clone(),
            "relations": extraction.relations.clone(),
            "key_facts": extraction.key_facts.clone(),
        });

        // 克隆向量用于 Qdrant
        let embedding_for_qdrant = embedding.clone();
        qdrant_client
            .insert_vectors(
                vec![embedding_for_qdrant],
                vec![entry_id.clone()],
                vec![metadata],
            )
            .await
            .map_err(|e| {
                error!("Failed to insert vector to Qdrant: {}", e);
                AppError::Internal(format!("Failed to insert vector: {}", e))
            })?;

        // 4. 存储到 SQLite（使用相同的 entry_id）
        let quality_score = Some(0.8); // 可以根据实际情况计算质量分数
        LTMRepository::create_knowledge_entry_with_id(
            Some(entry_id.clone()),
            source_id,
            normalized_source_type,
            title,
            &extraction.summary,
            "text",
            &embedding,
            embedding_service.model(),
            embedding_service.dimension() as i32,
            quality_score,
        )
        .await?;

        info!("LTM stored successfully: entry_id={}", entry_id);
        Ok(entry_id)
    }

    /// 自动将 STM 转移到 LTM（当达到阈值时）
    #[instrument]
    pub async fn auto_transfer_stm_to_ltm(
        session_id: &str,
        message_count_threshold: i32,
    ) -> Result<Vec<String>, AppError> {
        info!("Auto transferring STM to LTM: session_id={}", session_id);

        // 获取会话消息
        let messages = STMRepository::get_session_messages(session_id, Some(1000)).await?;
        
        if messages.len() < message_count_threshold as usize {
            info!("Message count ({}) below threshold ({}), skipping transfer", 
                  messages.len(), message_count_threshold);
            return Ok(Vec::new());
        }

        // 合并所有消息内容
        let combined_content: String = messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        // 存储为长期记忆
        let entry_id = Self::store_ltm(
            session_id,
            "session",
            &combined_content,
            Some(&format!("Session {}", session_id)),
        )
        .await?;

        info!("STM to LTM transfer completed: entry_id={}", entry_id);
        Ok(vec![entry_id])
    }

    /// 批量存储长期记忆
    #[instrument]
    pub async fn batch_store_ltm(
        entries: Vec<(String, String, String, Option<String>)>, // (source_id, source_type, content, title)
    ) -> Result<Vec<String>, AppError> {
        let total_count = entries.len();
        info!("Batch storing LTM: count={}", total_count);

        let mut entry_ids = Vec::new();
        for (source_id, source_type, content, title) in entries {
            match Self::store_ltm(&source_id, &source_type, &content, title.as_deref()).await {
                Ok(entry_id) => entry_ids.push(entry_id),
                Err(e) => {
                    error!("Failed to store LTM entry: source_id={}, error={}", source_id, e);
                    // 继续处理其他条目
                }
            }
        }

        info!("Batch storage completed: success={}/{}", entry_ids.len(), total_count);
        Ok(entry_ids)
    }
}

