use anyhow::Result;
use tracing::{error, info, instrument, warn};

use crate::db::{ltm::LTMRepository, stm::STMRepository};
use crate::services::{
    embedding::get_embedding_service, llm::get_llm_service, qdrant::get_qdrant_client,
};
use crate::AppError;

#[derive(Debug, Clone)]
pub struct LtmWriteRequest {
    pub tenant_id: Option<String>,
    pub source_id: String,
    pub source_type: String,
    pub content: String,
    pub title: Option<String>,
}

fn normalized_tenant_id(tenant_id: Option<&str>) -> Option<&str> {
    tenant_id.filter(|tenant_id| !tenant_id.is_empty())
}

fn tenant_prefix(tenant_id: &str) -> String {
    crate::services::multi_tenant::TenantId::new(tenant_id).prefix()
}

fn scope_actor_id(tenant_id: Option<&str>, actor_kind: &str, actor_id: &str) -> String {
    let Some(tenant_id) = normalized_tenant_id(tenant_id) else {
        return actor_id.to_string();
    };

    let prefix = tenant_prefix(tenant_id);
    if actor_id.starts_with(&prefix) {
        return actor_id.to_string();
    }

    format!("{}:{}:{}", prefix, actor_kind, actor_id)
}

fn scope_source_id(tenant_id: Option<&str>, source_id: &str) -> String {
    let Some(tenant_id) = normalized_tenant_id(tenant_id) else {
        return source_id.to_string();
    };

    let prefix = tenant_prefix(tenant_id);
    if source_id.starts_with(&prefix) {
        return source_id.to_string();
    }

    format!("{}:{}", prefix, source_id)
}

fn extract_tenant_id_from_actor_id(actor_id: &str, actor_kind: &str) -> Option<String> {
    let rest = actor_id.strip_prefix("t:")?;
    let marker = format!(":{}:", actor_kind);
    let tenant_end = rest.find(&marker)?;
    Some(rest[..tenant_end].to_string())
}

fn infer_session_tenant_id(session: &crate::db::stm::Session) -> Option<String> {
    extract_tenant_id_from_actor_id(&session.user_id, "user")
        .or_else(|| extract_tenant_id_from_actor_id(&session.agent_id, "agent"))
}

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
        Self::store_stm_with_tenant(
            None,
            user_id,
            agent_id,
            session_type,
            role,
            content,
            max_context_length,
            retention_hours,
        )
        .await
    }

    /// 存储短期记忆，并在需要时写入租户前缀。
    #[instrument]
    pub async fn store_stm_with_tenant(
        tenant_id: Option<&str>,
        user_id: &str,
        agent_id: &str,
        session_type: &str,
        role: &str,
        content: &str,
        max_context_length: i32,
        retention_hours: i32,
    ) -> Result<(String, String), AppError> {
        let scoped_user_id = scope_actor_id(tenant_id, "user", user_id);
        let scoped_agent_id = scope_actor_id(tenant_id, "agent", agent_id);

        info!(
            "Storing STM: tenant_id={:?}, user_id={}, agent_id={}, session_type={}",
            tenant_id, scoped_user_id, scoped_agent_id, session_type
        );

        // 创建或获取会话
        let session_id = STMRepository::create_session(
            &scoped_user_id,
            &scoped_agent_id,
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

        info!(
            "STM stored successfully: session_id={}, message_id={}",
            session_id, message_id
        );
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
        Self::store_ltm_with_tenant(None, source_id, source_type, content, title).await
    }

    /// 存储长期记忆，并在需要时写入租户信息。
    #[instrument]
    pub async fn store_ltm_with_tenant(
        tenant_id: Option<&str>,
        source_id: &str,
        source_type: &str,
        content: &str,
        title: Option<&str>,
    ) -> Result<String, AppError> {
        let scoped_source_id = scope_source_id(tenant_id, source_id);

        // 验证和规范化 source_type
        // 数据库约束只允许：'document', 'api', 'database', 'web', 'user_input'
        let normalized_source_type = match source_type {
            "document" | "api" | "database" | "web" | "user_input" => source_type,
            "test" | "testing" => {
                warn!(
                    "source_type '{}' is not allowed, mapping to 'user_input'",
                    source_type
                );
                "user_input"
            }
            _ => {
                warn!(
                    "Unknown source_type '{}', mapping to 'user_input'",
                    source_type
                );
                "user_input"
            }
        };

        info!(
            "Storing LTM: tenant_id={:?}, source_id={}, source_type={} (normalized from {}), content_length={}",
            tenant_id,
            scoped_source_id,
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

        info!(
            "LLM extraction completed: entities={}, relations={}",
            extraction.entities.len(),
            extraction.relations.len()
        );

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
            "tenant_id": tenant_id,
            "source_id": scoped_source_id,
            "source_type": normalized_source_type,
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

        // 4. 存储到关系数据库（使用相同的 entry_id）
        let quality_score = Some(0.8); // 可以根据实际情况计算质量分数
        if let Err(db_err) = LTMRepository::create_knowledge_entry_with_id(
            Some(entry_id.clone()),
            &scoped_source_id,
            normalized_source_type,
            title,
            &extraction.summary,
            "text",
            &embedding,
            embedding_service.model(),
            embedding_service.dimension() as i32,
            quality_score,
        )
        .await
        {
            error!(
                "Failed to persist LTM metadata after vector insert, rolling back Qdrant point: entry_id={}, error={}",
                entry_id, db_err
            );

            if let Err(rollback_err) = qdrant_client.delete_vectors(vec![entry_id.clone()]).await {
                error!(
                    "Rollback failed for Qdrant point: entry_id={}, rollback_error={}",
                    entry_id, rollback_err
                );
                return Err(AppError::Internal(format!(
                    "Failed to persist LTM metadata and rollback vector insert: db_error={}, rollback_error={}",
                    db_err, rollback_err
                )));
            }

            warn!(
                "Rolled back Qdrant point after metadata persist failure: entry_id={}",
                entry_id
            );
            return Err(db_err);
        }

        info!("LTM stored successfully: entry_id={}", entry_id);

        // Issue #58: record the successful write in the write journal.
        crate::services::information_guard::record_write(
            &crate::services::information_guard::WriteRecord {
                timestamp: chrono::Utc::now().to_rfc3339(),
                operation: "create".to_string(),
                entry_id: entry_id.clone(),
                source_id: scoped_source_id,
                content_hash: crate::services::information_guard::compute_sha256(
                    &extraction.summary,
                ),
                status: "ok".to_string(),
            },
        );

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
            info!(
                "Message count ({}) below threshold ({}), skipping transfer",
                messages.len(),
                message_count_threshold
            );
            return Ok(Vec::new());
        }

        // 合并所有消息内容
        let combined_content: String = messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let tenant_id = STMRepository::get_session(session_id)
            .await?
            .as_ref()
            .and_then(infer_session_tenant_id);

        // 存储为长期记忆
        let entry_id = Self::store_ltm_with_tenant(
            tenant_id.as_deref(),
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
        let writes = entries
            .into_iter()
            .map(|(source_id, source_type, content, title)| LtmWriteRequest {
                tenant_id: None,
                source_id,
                source_type,
                content,
                title,
            })
            .collect();

        Self::batch_store_ltm_with_tenant(writes).await
    }

    /// 批量存储长期记忆，并在每个条目上携带可选租户信息。
    #[instrument]
    pub async fn batch_store_ltm_with_tenant(
        entries: Vec<LtmWriteRequest>,
    ) -> Result<Vec<String>, AppError> {
        let total_count = entries.len();
        info!("Batch storing LTM: count={}", total_count);

        let mut entry_ids = Vec::new();
        for entry in entries {
            match Self::store_ltm_with_tenant(
                entry.tenant_id.as_deref(),
                &entry.source_id,
                &entry.source_type,
                &entry.content,
                entry.title.as_deref(),
            )
            .await
            {
                Ok(entry_id) => entry_ids.push(entry_id),
                Err(e) => {
                    error!(
                        "Failed to store LTM entry: source_id={}, error={}",
                        entry.source_id, e
                    );
                    // 继续处理其他条目
                }
            }
        }

        info!(
            "Batch storage completed: success={}/{}",
            entry_ids.len(),
            total_count
        );
        Ok(entry_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_actor_ids_for_tenant() {
        assert_eq!(
            scope_actor_id(Some("tenant_a"), "user", "alice"),
            "t:tenant_a:user:alice"
        );
        assert_eq!(
            scope_actor_id(Some("tenant_a"), "agent", "writer"),
            "t:tenant_a:agent:writer"
        );
    }

    #[test]
    fn keeps_existing_scoped_actor_ids() {
        assert_eq!(
            scope_actor_id(Some("tenant_a"), "user", "t:tenant_a:user:alice"),
            "t:tenant_a:user:alice"
        );
    }

    #[test]
    fn scopes_source_ids_for_tenant() {
        assert_eq!(
            scope_source_id(Some("tenant_a"), "session_123"),
            "t:tenant_a:session_123"
        );
    }

    #[test]
    fn extracts_tenant_id_from_scoped_actor_ids() {
        assert_eq!(
            extract_tenant_id_from_actor_id("t:tenant_a:user:alice", "user").as_deref(),
            Some("tenant_a")
        );
        assert_eq!(
            extract_tenant_id_from_actor_id("t:tenant_b:agent:writer", "agent").as_deref(),
            Some("tenant_b")
        );
    }
}
