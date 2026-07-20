use anyhow::Result;
use tracing::{error, info, instrument, warn};

use crate::db::tenant_scope::begin_tenant_tx;
use crate::db::vector_outbox::{self, OutboxOperation};
use crate::db::{ltm::LTMRepository, pool, stm::STMRepository};
use crate::services::{
    embedding::get_embedding_service, llm::get_llm_service, qdrant::get_qdrant_client,
};
use crate::tenant::{get_default_tenant, TenantId};
use crate::AppError;

/// 记忆存储服务
pub struct MemoryStorageService;

/// Result of an LTM store (fact row committed; vector may still be pending).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct StoreLtmResult {
    pub entry_id: String,
    /// `"ready"` when Qdrant was written synchronously (SQLite/legacy);
    /// `"pending"` when delivery is via the durable outbox (PostgreSQL).
    #[serde(rename = "indexStatus")]
    pub index_status: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct QdrantTenantBackfillReport {
    #[serde(rename = "dryRun")]
    pub dry_run: bool,
    pub scanned: usize,
    pub planned: usize,
    pub updated: usize,
    #[serde(rename = "skippedWithoutTenant")]
    pub skipped_without_tenant: usize,
}

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
        Self::store_stm_for_tenant(
            &get_default_tenant(),
            user_id,
            agent_id,
            session_type,
            role,
            content,
            max_context_length,
            retention_hours,
            None,
        )
        .await
    }

    /// 存储短期记忆（租户隔离）
    #[instrument]
    pub async fn store_stm_for_tenant(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        session_type: &str,
        role: &str,
        content: &str,
        max_context_length: i32,
        retention_hours: i32,
        existing_session_id: Option<&str>,
    ) -> Result<(String, String), AppError> {
        info!(
            "Storing STM: tenant_id={}, user_id={}, agent_id={}, session_type={}, existing_session={:?}",
            tenant_id, user_id, agent_id, session_type, existing_session_id
        );

        // If an existing session_id is provided, append to it directly
        let session_id = if let Some(sid) = existing_session_id {
            sid.to_string()
        } else {
            // 创建新会话
            STMRepository::create_session(
                tenant_id,
                user_id,
                agent_id,
                session_type,
                max_context_length,
                retention_hours,
            )
            .await?
        };

        // 添加消息到会话
        let message_id = STMRepository::add_message(
            pool(),
            tenant_id,
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

    /// 存储长期记忆（调用 LLM 总结 + 向量化 + 存储）
    #[instrument]
    pub async fn store_ltm(
        source_id: &str,
        source_type: &str,
        content: &str,
        title: Option<&str>,
    ) -> Result<String, AppError> {
        Ok(Self::store_ltm_for_tenant(
            &get_default_tenant(),
            source_id,
            source_type,
            content,
            title,
        )
        .await?
        .entry_id)
    }

    /// 存储长期记忆（租户隔离）。
    ///
    /// On PostgreSQL: DB fact + outbox event in one transaction; Qdrant is filled
    /// asynchronously by the outbox worker (`indexStatus: pending`).
    /// On SQLite / non-PG: legacy synchronous dual-write (`indexStatus: ready`).
    #[instrument]
    pub async fn store_ltm_for_tenant(
        tenant_id: &TenantId,
        source_id: &str,
        source_type: &str,
        content: &str,
        title: Option<&str>,
    ) -> Result<StoreLtmResult, AppError> {
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
            "Storing LTM: source_id={}, source_type={} (normalized from {}), content_length={}",
            source_id,
            normalized_source_type,
            source_type,
            content.len()
        );

        // 1. 调用 LLM 进行总结和结构化提取（事务外）
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

        // 2. 生成向量嵌入（事务外）
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

        let entry_id = ulid::Ulid::new().to_string();
        let content_hash = crate::services::information_guard::compute_sha256(&extraction.summary);
        let metadata = serde_json::json!({
            "tenantId": tenant_id.as_str(),
            "title": title,
            "summary": extraction.summary.clone(),
            "entities": extraction.entities.clone(),
            "relations": extraction.relations.clone(),
            "key_facts": extraction.key_facts.clone(),
            "contentHash": content_hash,
            "entryId": entry_id,
        });
        let quality_score = Some(0.8);

        let index_status = if crate::db::is_postgres() {
            // 3. PostgreSQL: single TX — knowledge_entries + vector outbox (no Qdrant on hot path)
            let outbox_payload = serde_json::json!({
                "vector": embedding,
                "metadata": metadata,
                "content_hash": content_hash,
            });
            let payload_json = serde_json::to_string(&outbox_payload).map_err(|e| {
                AppError::Internal(format!("Failed to serialize outbox payload: {e}"))
            })?;
            let payload_hash = crate::services::information_guard::compute_sha256(&payload_json);
            let idempotency_key = vector_outbox::upsert_idempotency_key(&entry_id, &payload_hash);

            let mut tx = begin_tenant_tx(pool(), tenant_id).await?;
            LTMRepository::insert_knowledge_entry_tx(
                &mut tx,
                Some(entry_id.clone()),
                tenant_id,
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

            vector_outbox::insert_event_tx(
                &mut tx,
                tenant_id.as_str(),
                &entry_id,
                OutboxOperation::Upsert,
                &payload_json,
                &payload_hash,
                &idempotency_key,
            )
            .await?;

            tx.commit().await.map_err(|e| {
                error!("Failed to commit LTM+outbox transaction: {}", e);
                AppError::Internal(format!("Database error: {e}"))
            })?;

            info!(
                entry_id = %entry_id,
                "LTM fact + outbox committed (index pending)"
            );
            "pending".to_string()
        } else {
            // 3. Legacy path: Qdrant first then DB (SQLite / non-PG dev)
            let qdrant_client = get_qdrant_client()
                .map_err(|e| AppError::Internal(format!("Failed to get Qdrant client: {}", e)))?;

            qdrant_client
                .insert_vectors(
                    vec![embedding.clone()],
                    vec![entry_id.clone()],
                    vec![metadata],
                )
                .await
                .map_err(|e| {
                    error!("Failed to insert vector to Qdrant: {}", e);
                    AppError::Internal(format!("Failed to insert vector: {}", e))
                })?;

            if let Err(db_err) = LTMRepository::create_knowledge_entry_with_id(
                Some(entry_id.clone()),
                tenant_id,
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
            .await
            {
                error!(
                    "Failed to persist LTM metadata after vector insert, rolling back Qdrant point: entry_id={}, error={}",
                    entry_id, db_err
                );

                if let Err(rollback_err) =
                    qdrant_client.delete_vectors(vec![entry_id.clone()]).await
                {
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

            info!(
                "LTM stored successfully (sync dual-write): entry_id={}",
                entry_id
            );
            "ready".to_string()
        };

        // Issue #58: write journal off the hot path
        let write_record = crate::services::information_guard::WriteRecord {
            timestamp: chrono::Utc::now().to_rfc3339(),
            operation: "create".to_string(),
            entry_id: entry_id.clone(),
            source_id: source_id.to_string(),
            content_hash,
            status: "ok".to_string(),
        };
        tokio::task::spawn_blocking(move || {
            crate::services::information_guard::record_write(&write_record);
        });

        Ok(StoreLtmResult {
            entry_id,
            index_status,
        })
    }

    /// 自动将 STM 转移到 LTM（当达到阈值时）
    #[instrument]
    pub async fn auto_transfer_stm_to_ltm(
        session_id: &str,
        message_count_threshold: i32,
    ) -> Result<Vec<String>, AppError> {
        Self::auto_transfer_stm_to_ltm_for_tenant(
            &get_default_tenant(),
            session_id,
            message_count_threshold,
        )
        .await
    }

    /// 自动将 STM 转移到 LTM（租户隔离）
    #[instrument]
    pub async fn auto_transfer_stm_to_ltm_for_tenant(
        tenant_id: &TenantId,
        session_id: &str,
        message_count_threshold: i32,
    ) -> Result<Vec<String>, AppError> {
        info!("Auto transferring STM to LTM: session_id={}", session_id);

        // 获取会话消息
        let messages =
            STMRepository::get_session_messages(pool(), tenant_id, session_id, Some(1000)).await?;

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

        // 存储为长期记忆
        let result = Self::store_ltm_for_tenant(
            tenant_id,
            session_id,
            "session",
            &combined_content,
            Some(&format!("Session {}", session_id)),
        )
        .await?;

        info!(
            "STM to LTM transfer completed: entry_id={}, index_status={}",
            result.entry_id, result.index_status
        );
        Ok(vec![result.entry_id])
    }

    /// 批量存储长期记忆
    #[instrument]
    pub async fn batch_store_ltm(
        entries: Vec<(String, String, String, Option<String>)>, // (source_id, source_type, content, title)
    ) -> Result<Vec<String>, AppError> {
        Self::batch_store_ltm_for_tenant(&get_default_tenant(), entries).await
    }

    /// 批量存储长期记忆（租户隔离）
    #[instrument]
    pub async fn batch_store_ltm_for_tenant(
        tenant_id: &TenantId,
        entries: Vec<(String, String, String, Option<String>)>, // (source_id, source_type, content, title)
    ) -> Result<Vec<String>, AppError> {
        let total_count = entries.len();
        info!("Batch storing LTM: count={}", total_count);

        let mut entry_ids = Vec::new();
        for (source_id, source_type, content, title) in entries {
            match Self::store_ltm_for_tenant(
                tenant_id,
                &source_id,
                &source_type,
                &content,
                title.as_deref(),
            )
            .await
            {
                Ok(result) => entry_ids.push(result.entry_id),
                Err(e) => {
                    error!(
                        "Failed to store LTM entry: source_id={}, error={}",
                        source_id, e
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

    /// Backfill Qdrant `tenantId` payload from LTM DB source_id prefixes.
    pub async fn backfill_qdrant_tenant_metadata(
        limit: i32,
        offset: i32,
        dry_run: bool,
    ) -> Result<QdrantTenantBackfillReport, AppError> {
        let limit = limit.clamp(1, 1000);
        let offset = offset.max(0);
        let rows =
            LTMRepository::list_qdrant_tenant_backfill_entries(pool(), limit, offset).await?;

        let mut report = QdrantTenantBackfillReport {
            dry_run,
            scanned: rows.len(),
            planned: 0,
            updated: 0,
            skipped_without_tenant: 0,
        };

        let mut by_tenant: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for row in rows {
            if let Some(tenant_id) = tenant_id_from_source_id(&row.source_id) {
                report.planned += 1;
                by_tenant.entry(tenant_id).or_default().push(row.entry_id);
            } else {
                report.skipped_without_tenant += 1;
            }
        }

        if dry_run || by_tenant.is_empty() {
            return Ok(report);
        }

        let qdrant_client = get_qdrant_client()
            .map_err(|e| AppError::Internal(format!("Failed to get Qdrant client: {}", e)))?;
        for (tenant_id, entry_ids) in by_tenant {
            let updated = qdrant_client
                .set_tenant_payload_for_entries(entry_ids, &tenant_id)
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to backfill Qdrant tenantId: {}", e))
                })?;
            report.updated += updated;
        }

        Ok(report)
    }
}

fn tenant_id_from_source_id(source_id: &str) -> Option<String> {
    let rest = source_id.strip_prefix("t:")?;
    let tenant_id = rest.split(':').next().unwrap_or_default();
    if tenant_id.is_empty() {
        None
    } else {
        Some(tenant_id.to_string())
    }
}
