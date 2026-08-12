use anyhow::Result;
use tracing::{error, info, instrument, warn};

use crate::db::tenant_scope::begin_tenant_tx;
use crate::db::vector_outbox::{self, OutboxOperation};
use crate::db::{admin_pool, ltm::LTMRepository, pool, stm::STMRepository};
use crate::services::prometheus_exporter::get_exporter;
use crate::services::{
    embedding::{get_embedding_service, EmbeddingError},
    llm::{get_llm_service, LlmError, StructuredExtraction},
    qdrant::get_qdrant_client,
};
use crate::tenant::{get_default_tenant, TenantId};
use crate::AppError;

/// Max characters of raw content embedded (and stored in `content`) when the
/// LLM summary is deferred. The normal path embeds a short LLM summary; without
/// one we embed the raw content so retrieval still works, but bound it so a very
/// large document cannot overflow the embedding model's context and turn the
/// degraded write into a *new* failure. Conservative on purpose.
const DEGRADED_EMBED_MAX_CHARS: usize = 4000;

/// Whether an LTM entry carries a real LLM summary/extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SummaryStatus {
    /// Summary + structured extraction produced normally.
    Complete,
    /// LLM backend was unavailable/erroring; summary is empty and awaits backfill.
    Pending,
}

impl SummaryStatus {
    /// Stable string persisted to the DB `summary_status` column and surfaced in
    /// the store response / Qdrant metadata.
    fn as_str(self) -> &'static str {
        match self {
            SummaryStatus::Complete => "complete",
            SummaryStatus::Pending => "pending",
        }
    }
}

/// Classify an LLM summarization error into a bounded metric `reason` when the
/// failure is safe to degrade, or `None` when it must be surfaced to the caller.
///
/// - `Unavailable` (backend unreachable) / `Upstream` (reachable but errored) →
///   degrade: the summary is a best-effort enrichment, keep the write.
/// - `Malformed` (reachable but returned unusable output) or any non-`LlmError`
///   → surface it. Silently degrading garbage output would mask a real
///   prompt/model bug behind "successful" writes.
fn degradable_summary_reason(err: &anyhow::Error) -> Option<&'static str> {
    match err.downcast_ref::<LlmError>() {
        Some(LlmError::Unavailable(_)) => Some("unavailable"),
        Some(LlmError::Upstream { .. }) => Some("upstream_error"),
        _ => None,
    }
}

/// Text used for BOTH the embedding vector and the stored `content` column.
/// Degraded (no summary): a bounded prefix of the raw content, so retrieval on
/// the entry still works instead of silently embedding an empty summary.
///
/// ⚠️ BACKFILL CONSEQUENCE (read before writing a summary-backfill job):
/// the embedding vector is derived from THIS text. So a `Complete` entry's
/// vector = embed(summary), but a `Pending` entry's vector = embed(content) —
/// two *different* inputs. When a backfill later fills the deferred summary, it
/// MUST recompute the vector as embed(new summary) AND emit a
/// `vector_outbox` upsert (see `store_ltm_for_tenant`) so Qdrant is updated.
/// Filling only the summary text would leave the vector permanently derived
/// from the content while the rest of the corpus is summary-derived — a silent
/// inconsistency. This is NOT handled here; backfill is a separate work item.
fn embed_and_store_text(content: &str, summary: &str, status: SummaryStatus) -> String {
    match status {
        SummaryStatus::Complete => summary.to_string(),
        SummaryStatus::Pending => content.chars().take(DEGRADED_EMBED_MAX_CHARS).collect(),
    }
}

/// Map an embedding-generation failure to an HTTP-appropriate `AppError` (D-e2).
///
/// Embedding is a hard dependency, so we distinguish "backend can't serve right
/// now" (retryable → 503) from "our config is wrong / a real bug" (retry won't
/// help → 500), instead of collapsing everything to 500:
/// - transport-unreachable, or a `5xx`/`429` upstream status → 503
///   (`DependencyUnavailable`).
/// - a `4xx` upstream status (bad model, bad api key) → 500: that is a config
///   error, not the caller's fault and not fixed by retrying, so reporting it as
///   a retryable 503 would just spin the caller.
/// - malformed / wrong-dimension output, or any non-`EmbeddingError` → 500.
///
/// The detail (which may include an internal endpoint) goes into the `AppError`
/// String for LOGS ONLY; `AppError::api_message` returns a generic message, so
/// the endpoint never reaches the client.
fn embedding_error_to_app_error(e: anyhow::Error) -> AppError {
    match e.downcast_ref::<EmbeddingError>() {
        Some(EmbeddingError::Unavailable(_)) => {
            AppError::DependencyUnavailable(format!("embedding backend unavailable: {e}"))
        }
        Some(EmbeddingError::Upstream { status }) if *status >= 500 || *status == 429 => {
            AppError::DependencyUnavailable(format!("embedding backend error: {e}"))
        }
        _ => AppError::Internal(format!("embedding generation failed: {e}")),
    }
}

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
    /// `"complete"` when the LLM summary/extraction was produced; `"pending"`
    /// when the LLM backend was unavailable and the summary was deferred for
    /// later backfill (the write still succeeded).
    #[serde(rename = "summaryStatus")]
    pub summary_status: String,
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
    #[instrument(fields(tenant_id = %tenant_id, user_id = %user_id, agent_id = %agent_id))]
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
            // 创建新会话：仅在此分支 session_type 才会写入 DB 并撞上
            // `CHECK (session_type IN (...))`。校验放在写入前的边界，返回 400 并列出
            // 合法值（backlog D-a），避免 DB check 约束以 500「内部错误」暴露给调用方。
            // 追加到既有会话时 session_type 不参与写入，故不校验（见任务的既有数据说明）。
            let session_type = crate::models::memory_enums::SessionType::parse(session_type)
                .map_err(AppError::BadRequest)?;
            STMRepository::create_session(
                tenant_id,
                user_id,
                agent_id,
                session_type.as_str(),
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
    #[instrument(fields(tenant_id = %tenant_id, source_id = %source_id, source_type = %source_type))]
    pub async fn store_ltm_for_tenant(
        tenant_id: &TenantId,
        source_id: &str,
        source_type: &str,
        content: &str,
        title: Option<&str>,
    ) -> Result<StoreLtmResult, AppError> {
        // 校验 source_type（backlog D-a）。
        // 合法值的单一真相源是 `models::memory_enums::SourceType`（与 migration 的
        // `CHECK (source_type IN (...))` 由防漂移测试锁定一致）。此前这里把未知值
        // 静默重映射为 'user_input'——那会把调用方的笔误当成合法来源存进库，无人可知；
        // 现在改为在边界拒绝并返回 400，错误消息列出全部合法值，集成方可据此自查。
        let source_type = crate::models::memory_enums::SourceType::parse(source_type)
            .map_err(AppError::BadRequest)?;
        let normalized_source_type = source_type.as_str();

        info!(
            "Storing LTM: source_id={}, source_type={}, content_length={}",
            source_id,
            normalized_source_type,
            content.len()
        );

        // 1. 调用 LLM 进行总结和结构化提取（事务外）。
        //
        // 摘要为可降级能力（backlog D-e）：LLM 后端「不可达」或「返回错误状态」时，
        // 允许写入继续、摘要留空并标记 summary_status=pending，事后由独立回填流程补齐。
        // 但「可达却返回无法解析的内容」(Malformed) 属于真实故障，不静默降级、必须暴露——
        // 否则会把系统性 prompt/模型 bug 伪装成一次「成功」写入。
        let llm_service = get_llm_service()
            .map_err(|e| AppError::Internal(format!("Failed to get LLM service: {}", e)))?;

        let (extraction, summary_status) = match llm_service.summarize_and_extract(content).await {
            Ok(extraction) => {
                info!(
                    "LLM extraction completed: entities={}, relations={}",
                    extraction.entities.len(),
                    extraction.relations.len()
                );
                (extraction, SummaryStatus::Complete)
            }
            Err(e) => match degradable_summary_reason(&e) {
                Some(reason) => {
                    warn!(
                        tenant_id = %tenant_id,
                        source_id = %source_id,
                        reason = reason,
                        "LLM summary unavailable; storing LTM with deferred summary (summary_status=pending): {}",
                        e
                    );
                    get_exporter().inc_ltm_summary_degraded(reason);
                    (StructuredExtraction::default(), SummaryStatus::Pending)
                }
                None => {
                    error!("LLM summarization failed (non-degradable): {}", e);
                    return Err(AppError::Internal(format!(
                        "LLM summarization failed: {}",
                        e
                    )));
                }
            },
        };

        // Text for BOTH the embedding vector and the stored `content` column:
        // normal → the (short) summary; degraded → bounded raw content, so we
        // never silently embed an empty summary and wreck retrieval quality.
        let embed_text = embed_and_store_text(content, &extraction.summary, summary_status);

        // 2. 生成向量嵌入（事务外）。
        //
        // 注意：embedding 仍是硬依赖——它也走嵌入后端（典型部署与 LLM 同一个 Ollama）。
        // 若嵌入后端同样不可达，此处仍会失败并向上返回错误；本任务不改变这一点。让
        // 「嵌入后端宕机也能写入」需要延迟向量化，属于另立的工作项，不在本次范围内。
        let embedding_service = get_embedding_service()
            .map_err(|e| AppError::Internal(format!("Failed to get embedding service: {}", e)))?;

        let embedding = embedding_service
            .generate_embedding(&embed_text)
            .await
            .map_err(|e| {
                error!("Embedding generation failed: {}", e);
                embedding_error_to_app_error(e)
            })?;

        info!("Embedding generated: dimension={}", embedding.len());

        let entry_id = ulid::Ulid::new().to_string();
        let content_hash = crate::services::information_guard::compute_sha256(&embed_text);
        let metadata = serde_json::json!({
            "tenantId": tenant_id.as_str(),
            "title": title,
            "summary": extraction.summary.clone(),
            "summaryStatus": summary_status.as_str(),
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
                &embed_text,
                "text",
                &embedding,
                embedding_service.model(),
                embedding_service.dimension() as i32,
                quality_score,
                Some(summary_status.as_str()),
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
                &embed_text,
                "text",
                &embedding,
                embedding_service.model(),
                embedding_service.dimension() as i32,
                quality_score,
                Some(summary_status.as_str()),
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
            summary_status: summary_status.as_str().to_string(),
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
        let admin = admin_pool().ok_or_else(|| {
            AppError::BadRequest(
                "Qdrant tenant backfill requires a configured admin database connection \
                 (db.admin_url). This feature is disabled by default — set db.admin_url to \
                 an owner/BYPASSRLS connection string to enable it."
                    .into(),
            )
        })?;
        let rows = LTMRepository::list_qdrant_tenant_backfill_entries(admin, limit, offset).await?;

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

    /// Backfill summaries for entries stored with `summary_status = 'pending'`.
    ///
    /// For each entry: call LLM for summary + structured extraction, re-generate
    /// the embedding vector from the new summary text, update the row's `content` +
    /// `content_hash` + `summary_status`, and enqueue a fresh outbox event so
    /// Qdrant is re-indexed with the correct vector.
    ///
    /// Must update hash AND vector together: the `content` column switches from raw
    /// text to the summary, so the content_hash changes, and the embedding vector
    /// — which was generated from the raw text at degrade time — must be
    /// regenerated from the summary to match what all other entries have. Updating
    /// only one of the three (content, hash, vector) would make the reconciliation
    /// scanner report a drift it cannot repair.
    pub async fn backfill_pending_summaries(
        tenant_id: &TenantId,
        batch_size: i32,
    ) -> Result<SummaryBackfillReport, AppError> {
        let pool = crate::db::pool();
        let entries =
            LTMRepository::list_entries_pending_summary(pool, tenant_id, batch_size, 0).await?;

        let llm_service = get_llm_service()
            .map_err(|e| AppError::Internal(format!("LLM service unavailable: {e}")))?;
        let embedding_service = get_embedding_service()
            .map_err(|e| AppError::Internal(format!("Embedding service unavailable: {e}")))?;

        let mut report = SummaryBackfillReport {
            scanned: entries.len(),
            completed: 0,
            failed: 0,
            errors: vec![],
        };

        for entry in entries {
            match Self::backfill_one_entry(tenant_id, &entry, llm_service, embedding_service).await
            {
                Ok(()) => report.completed += 1,
                Err(e) => {
                    report.failed += 1;
                    report.errors.push(format!("{}: {e}", entry.entry_id));
                    tracing::warn!(
                        entry_id = %entry.entry_id,
                        error = %e,
                        "summary backfill failed for entry"
                    );
                }
            }
        }

        info!(
            tenant = %tenant_id,
            scanned = report.scanned,
            completed = report.completed,
            failed = report.failed,
            "summary backfill batch complete"
        );

        Ok(report)
    }

    async fn backfill_one_entry(
        tenant_id: &TenantId,
        entry: &crate::db::ltm::PendingSummaryEntry,
        llm_service: &crate::services::llm::LLMService,
        embedding_service: &crate::services::embedding::EmbeddingService,
    ) -> Result<(), AppError> {
        let extraction = llm_service
            .summarize_and_extract(&entry.content)
            .await
            .map_err(|e| AppError::Internal(format!("LLM failed: {e}")))?;

        let embed_text =
            embed_and_store_text(&entry.content, &extraction.summary, SummaryStatus::Complete);
        let embedding = embedding_service
            .generate_embedding(&embed_text)
            .await
            .map_err(embedding_error_to_app_error)?;

        let content_hash = crate::services::information_guard::compute_sha256(&embed_text);
        let metadata = serde_json::json!({
            "tenantId": tenant_id.as_str(),
            "summary": extraction.summary,
            "summaryStatus": "complete",
            "entities": extraction.entities,
            "relations": extraction.relations,
            "key_facts": extraction.key_facts,
            "contentHash": content_hash,
            "entryId": entry.entry_id,
        });

        let outbox_payload = serde_json::json!({
            "vector": embedding,
            "metadata": metadata,
            "content_hash": content_hash,
        });
        let payload_json = serde_json::to_string(&outbox_payload)
            .map_err(|e| AppError::Internal(format!("Failed to serialize payload: {e}")))?;
        let payload_hash = crate::services::information_guard::compute_sha256(&payload_json);
        let idempotency_key =
            crate::db::vector_outbox::upsert_idempotency_key(&entry.entry_id, &payload_hash);

        let pool = crate::db::pool();
        let mut tx = begin_tenant_tx(pool, tenant_id).await?;

        sqlx::query(
            "UPDATE knowledge_entries \
             SET content = $1, content_hash = $2, summary_status = 'complete', updated_at = now() \
             WHERE entry_id = $3",
        )
        .bind(&embed_text)
        .bind(&content_hash)
        .bind(&entry.entry_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to update entry: {e}")))?;

        sqlx::query(
            "INSERT INTO memory_vector_outbox \
             (event_id, tenant_id, entry_id, operation, payload_json, payload_hash, idempotency_key) \
             VALUES ($1, $2, $3, 'upsert', $4, $5, $6) \
             ON CONFLICT (tenant_id, idempotency_key) DO NOTHING",
        )
        .bind(ulid::Ulid::new().to_string())
        .bind(tenant_id.as_str())
        .bind(&entry.entry_id)
        .bind(&payload_json)
        .bind(&payload_hash)
        .bind(&idempotency_key)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to enqueue outbox: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to commit backfill: {e}")))?;

        Ok(())
    }
}

/// Summary backfill report.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SummaryBackfillReport {
    pub scanned: usize,
    pub completed: usize,
    pub failed: usize,
    pub errors: Vec<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_status_as_str_is_stable() {
        assert_eq!(SummaryStatus::Complete.as_str(), "complete");
        assert_eq!(SummaryStatus::Pending.as_str(), "pending");
    }

    #[test]
    fn unreachable_and_upstream_errors_are_degradable() {
        // "Ollama 不可达" and reachable-but-error-status keep the write alive.
        let unavailable = anyhow::Error::new(LlmError::Unavailable("connection refused".into()));
        assert_eq!(degradable_summary_reason(&unavailable), Some("unavailable"));

        let upstream = anyhow::Error::new(LlmError::Upstream { status: 503 });
        assert_eq!(degradable_summary_reason(&upstream), Some("upstream_error"));
    }

    #[test]
    fn malformed_and_unknown_errors_are_not_degradable() {
        // Reachable-but-unusable output must surface, not be silently masked as a
        // "successful" degraded write — this is the self-audit guard for D-e.
        let malformed = anyhow::Error::new(LlmError::Malformed("not json".into()));
        assert_eq!(degradable_summary_reason(&malformed), None);

        // A non-LlmError (e.g. an unexpected internal failure) must also surface.
        let other = anyhow::anyhow!("some unrelated failure");
        assert_eq!(degradable_summary_reason(&other), None);
    }

    #[test]
    fn complete_status_embeds_and_stores_the_summary() {
        let text = embed_and_store_text("raw content", "the summary", SummaryStatus::Complete);
        assert_eq!(text, "the summary");
    }

    #[test]
    fn pending_status_embeds_raw_content_not_empty_summary() {
        // Degraded: embed the real content, never an empty summary, otherwise
        // retrieval on the entry would be silently destroyed.
        let text = embed_and_store_text("raw content", "", SummaryStatus::Pending);
        assert_eq!(text, "raw content");
    }

    #[test]
    fn pending_status_bounds_large_content_by_chars() {
        let big = "x".repeat(DEGRADED_EMBED_MAX_CHARS + 500);
        let text = embed_and_store_text(&big, "", SummaryStatus::Pending);
        assert_eq!(text.chars().count(), DEGRADED_EMBED_MAX_CHARS);
    }

    #[test]
    fn pending_status_truncation_is_utf8_safe() {
        // Multi-byte chars must not be split mid-codepoint (char-based, not byte).
        let multibyte = "语".repeat(DEGRADED_EMBED_MAX_CHARS + 100);
        let text = embed_and_store_text(&multibyte, "", SummaryStatus::Pending);
        assert_eq!(text.chars().count(), DEGRADED_EMBED_MAX_CHARS);
        assert!(text.chars().all(|c| c == '语'));
    }

    // --- embedding failure → HTTP status mapping (D-e2) ------------------ //

    #[test]
    fn embedding_unavailable_maps_to_503_variant() {
        let e = anyhow::Error::new(EmbeddingError::Unavailable("connection refused".into()));
        assert!(matches!(
            embedding_error_to_app_error(e),
            AppError::DependencyUnavailable(_)
        ));
    }

    #[test]
    fn embedding_5xx_and_429_map_to_503() {
        for status in [500u16, 502, 503, 429] {
            let e = anyhow::Error::new(EmbeddingError::Upstream { status });
            assert!(
                matches!(
                    embedding_error_to_app_error(e),
                    AppError::DependencyUnavailable(_)
                ),
                "status {status} should map to 503"
            );
        }
    }

    #[test]
    fn embedding_4xx_maps_to_500_not_503() {
        // Bad model (404) / bad api key (401) are config errors — retrying never
        // helps, so they must NOT be reported as a retryable 503.
        for status in [400u16, 401, 404] {
            let e = anyhow::Error::new(EmbeddingError::Upstream { status });
            assert!(
                matches!(embedding_error_to_app_error(e), AppError::Internal(_)),
                "status {status} should map to 500"
            );
        }
    }

    #[test]
    fn embedding_malformed_and_unknown_map_to_500() {
        let malformed = anyhow::Error::new(EmbeddingError::Malformed("bad json".into()));
        assert!(matches!(
            embedding_error_to_app_error(malformed),
            AppError::Internal(_)
        ));
        // A non-EmbeddingError (unexpected failure) must also surface as 500.
        let other = anyhow::anyhow!("some unrelated failure");
        assert!(matches!(
            embedding_error_to_app_error(other),
            AppError::Internal(_)
        ));
    }
}
