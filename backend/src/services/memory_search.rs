use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};

use crate::AppError;
use crate::config;
use crate::db::{
    ltm::LTMRepository,
    stm::{STMRepository, SessionMessage},
};
use crate::services::{
    embedding::get_embedding_service, qdrant::get_qdrant_client, rerank::get_rerank_service,
};

/// 记忆搜索服务
pub struct MemorySearchService;

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SearchResult {
    pub entry_id: String,
    pub score: f32,
    pub content: String,
    pub title: Option<String>,
    pub metadata: serde_json::Value,
}

impl MemorySearchService {
    /// 搜索短期记忆（基于会话、时间范围等）
    #[instrument]
    pub async fn search_stm(
        user_id: &str,
        agent_id: &str,
        session_type: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<SessionMessage>, AppError> {
        info!(
            "Searching STM: user_id={}, agent_id={}, session_type={:?}",
            user_id, agent_id, session_type
        );

        // 获取最近会话
        let sessions = STMRepository::get_recent_sessions(user_id, agent_id, limit).await?;

        // 获取所有会话的消息
        let mut all_messages = Vec::new();
        for session in sessions {
            // 如果指定了 session_type，进行过滤
            if let Some(st) = session_type {
                if session.session_type != st {
                    continue;
                }
            }

            let messages =
                STMRepository::get_session_messages(&session.session_id, Some(100)).await?;
            all_messages.extend(messages);
        }

        // 按时间排序
        all_messages.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        info!(
            "STM search completed: found {} messages",
            all_messages.len()
        );
        Ok(all_messages)
    }

    /// 搜索长期记忆（基于向量相似度，使用 Qdrant）
    #[instrument]
    pub async fn search_ltm(
        query: &str,
        top_k: usize,
        enable_rerank: Option<bool>,
        min_score: Option<f32>,
    ) -> Result<Vec<SearchResult>, AppError> {
        info!(
            "Searching LTM: query_length={}, top_k={}, enable_rerank={:?}, min_score={:?}",
            query.len(),
            top_k,
            enable_rerank,
            min_score
        );

        let rerank_config = config::get().rerank.clone();
        let should_rerank = enable_rerank.unwrap_or(rerank_config.enabled);
        let min_score_threshold = min_score.unwrap_or(rerank_config.min_score_threshold);

        // 计算初始检索数量
        let initial_top_k = if should_rerank {
            top_k * rerank_config.candidate_multiplier
        } else {
            top_k
        };

        // 1. 生成查询向量
        let embedding_service = get_embedding_service()
            .map_err(|e| AppError::Internal(format!("Failed to get embedding service: {}", e)))?;

        let query_vector = embedding_service
            .generate_embedding(query)
            .await
            .map_err(|e| {
                error!("Failed to generate query embedding: {}", e);
                AppError::Internal(format!("Failed to generate embedding: {}", e))
            })?;

        info!(
            "Query embedding generated: dimension={}",
            query_vector.len()
        );

        // 2. 在 Qdrant 中搜索
        let qdrant_client = get_qdrant_client()
            .map_err(|e| AppError::Internal(format!("Failed to get Qdrant client: {}", e)))?;

        let qdrant_results = qdrant_client
            .search(query_vector, initial_top_k)
            .await
            .map_err(|e| {
                error!("Qdrant search failed: {}", e);
                AppError::Internal(format!("Qdrant search failed: {}", e))
            })?;

        info!(
            "Qdrant search completed: found {} results",
            qdrant_results.len()
        );

        // 3. 从 SQLite 获取详细信息
        let mut search_results = Vec::new();
        for qdrant_result in qdrant_results {
            info!(
                "Processing Qdrant result: id={}, score={:.4}",
                qdrant_result.id, qdrant_result.score
            );
            match LTMRepository::get_entry_by_id(&qdrant_result.id).await {
                Ok(Some(entry)) => {
                    info!("Found entry in SQLite: entry_id={}", entry.entry_id);
                    search_results.push(SearchResult {
                        entry_id: entry.entry_id,
                        score: qdrant_result.score,
                        content: entry.content,
                        title: entry.title,
                        metadata: qdrant_result.metadata,
                    });
                }
                Ok(None) => {
                    warn!("Entry not found in SQLite: id={}", qdrant_result.id);
                }
                Err(e) => {
                    error!(
                        "Failed to get entry from SQLite: id={}, error={}",
                        qdrant_result.id, e
                    );
                }
            }
        }

        // 4. 应用 rerank（如果启用）
        if should_rerank && !search_results.is_empty() {
            info!("Applying rerank to {} candidates", search_results.len());
            search_results = Self::apply_rerank(query, search_results).await?;
        }

        // 5. 应用最低分数阈值过滤
        if min_score_threshold > 0.0 {
            search_results = Self::filter_by_threshold(search_results, min_score_threshold);
        }

        // 6. 返回前 top_k 个结果
        search_results.truncate(top_k);

        info!(
            "LTM search completed: returned {} results",
            search_results.len()
        );
        Ok(search_results)
    }

    /// 混合搜索（结合关键词和向量搜索）
    #[instrument]
    pub async fn hybrid_search(
        query: &str,
        top_k: usize,
        keyword_weight: f32,
        vector_weight: f32,
        enable_rerank: Option<bool>,
        min_score: Option<f32>,
    ) -> Result<Vec<SearchResult>, AppError> {
        info!(
            "Hybrid search: query_length={}, top_k={}, keyword_weight={}, vector_weight={}, enable_rerank={:?}, min_score={:?}",
            query.len(),
            top_k,
            keyword_weight,
            vector_weight,
            enable_rerank,
            min_score
        );

        let rerank_config = config::get().rerank.clone();
        let should_rerank = enable_rerank.unwrap_or(rerank_config.enabled);
        let min_score_threshold = min_score.unwrap_or(rerank_config.min_score_threshold);

        // 计算初始检索数量
        let initial_top_k = if should_rerank {
            top_k * rerank_config.candidate_multiplier
        } else {
            top_k * 2 // 保持原有逻辑
        };

        // 1. 向量搜索
        let vector_results = Self::search_ltm(query, initial_top_k, Some(false), None).await?;

        // 2. 关键词搜索（基于SQLite的全文搜索）
        let keyword_results = Self::keyword_search(query, initial_top_k).await?;

        // 3. 合并结果（加权分数）
        // 创建一个HashMap来存储每个entry_id的综合分数
        use std::collections::HashMap;
        let mut entry_scores: HashMap<String, (SearchResult, f32, f32)> = HashMap::new();

        // 添加向量搜索结果
        for result in vector_results {
            entry_scores.insert(
                result.entry_id.clone(),
                (result.clone(), result.score as f32, 0.0), // (result, vector_score, keyword_score)
            );
        }

        // 添加关键词搜索结果
        for (entry_id, keyword_score) in keyword_results {
            if let Some((result, _, _)) = entry_scores.get(&entry_id) {
                // 获取向量分数
                let vector_score = result.score;
                // 更新entry_scores
                entry_scores.insert(
                    entry_id.clone(),
                    (result.clone(), vector_score, keyword_score as f32),
                );
            } else {
                // 如果关键词搜索结果不在向量搜索结果中，尝试获取完整的知识条目
                if let Ok(Some(entry)) =
                    crate::db::ltm::LTMRepository::get_entry_by_id(&entry_id).await
                {
                    // 创建一个新的SearchResult
                    let search_result = SearchResult {
                        entry_id: entry.entry_id.clone(),
                        score: 0.0, // 初始向量分数为0，后续会被关键词分数加权
                        content: entry.content.clone(),
                        title: entry.title.clone(),
                        metadata: serde_json::Value::Null,
                    };

                    entry_scores
                        .insert(entry_id.clone(), (search_result, 0.0, keyword_score as f32));
                }
            }
        }

        // 计算综合分数
        let mut combined_results: Vec<(SearchResult, f32)> = entry_scores
            .into_iter()
            .map(|(_, (result, vector_score, keyword_score))| {
                // 归一化分数：将关键词匹配次数转换为0-1之间的分数
                let normalized_keyword_score = if keyword_score > 0.0 {
                    (keyword_score / query.split_whitespace().count() as f32).min(1.0)
                } else {
                    0.0
                };

                // 计算加权综合分数
                let combined_score =
                    (vector_score * vector_weight) + (normalized_keyword_score * keyword_weight);
                (result, combined_score)
            })
            .collect();

        // 4. 按综合分数排序
        combined_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 5. 转换为 SearchResult 列表
        let mut results: Vec<SearchResult> = combined_results
            .into_iter()
            .map(|(r, score)| SearchResult { score, ..r })
            .collect();

        // 6. 应用 rerank（如果启用）
        if should_rerank && !results.is_empty() {
            info!(
                "Applying rerank to {} candidates in hybrid search",
                results.len()
            );
            results = Self::apply_rerank(query, results).await?;
        }

        // 7. 应用最低分数阈值过滤
        if min_score_threshold > 0.0 {
            results = Self::filter_by_threshold(results, min_score_threshold);
        }

        // 8. 返回前 top_k 结果
        results.truncate(top_k);

        info!(
            "Hybrid search completed: returned {} results",
            results.len()
        );
        Ok(results)
    }

    /// 关键词搜索（基于SQLite的全文搜索）
    #[instrument]
    async fn keyword_search(query: &str, limit: usize) -> Result<Vec<(String, f64)>, AppError> {
        info!("Keyword search: query={}, limit={}", query, limit);

        let pool = crate::db::pool();
        let limit_i32 = limit as i32;

        // 使用SQLite的LIKE搜索来实现关键词搜索
        // 实际应用中应该使用SQLite的全文搜索扩展（FTS5）
        let query_with_wildcards = format!("%{query}%");

        let rows = sqlx::query_as::<_, (String, f64)>(
            r#"
            SELECT entry_id, 
                   (CASE 
                        WHEN content LIKE $1 THEN 1.0 
                        WHEN title LIKE $2 THEN 0.8 
                        ELSE 0.0 
                    END) as score
            FROM knowledge_entries
            WHERE (content LIKE $3 OR title LIKE $4)
            AND status = 'active'
            ORDER BY score DESC, access_count DESC, created_at DESC
            LIMIT $5
            "#,
        )
        .bind(&query_with_wildcards)
        .bind(&query_with_wildcards)
        .bind(&query_with_wildcards)
        .bind(&query_with_wildcards)
        .bind(limit_i32)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to perform keyword search: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        // 进一步优化：如果查询包含多个关键词，增加匹配多个关键词的条目的分数
        let keywords: Vec<&str> = query.split_whitespace().collect();
        if keywords.len() > 1 {
            let mut enhanced_rows = Vec::new();

            for (entry_id, mut score) in rows {
                // 获取完整的知识条目
                if let Ok(Some(entry)) =
                    crate::db::ltm::LTMRepository::get_entry_by_id(&entry_id).await
                {
                    let content_lower = entry.content.to_lowercase();
                    let title_lower = entry.title.unwrap_or_default().to_lowercase();

                    // 计算匹配的关键词数量
                    let mut match_count = 0;
                    for keyword in &keywords {
                        let keyword_lower = keyword.to_lowercase();
                        if content_lower.contains(&keyword_lower)
                            || title_lower.contains(&keyword_lower)
                        {
                            match_count += 1;
                        }
                    }

                    // 增强分数：匹配的关键词数量越多，分数越高
                    let enhancement = (match_count as f64 / keywords.len() as f64) * 0.5;
                    score += enhancement;

                    enhanced_rows.push((entry_id, score));
                }
            }

            // 按分数排序
            enhanced_rows
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            Ok(enhanced_rows)
        } else {
            Ok(rows)
        }
    }

    /// 基于实体搜索（知识图谱）
    #[instrument]
    pub async fn search_by_entity(
        entity: &str,
        limit: Option<i32>,
    ) -> Result<Vec<SearchResult>, AppError> {
        info!("Searching by entity: entity={}, limit={:?}", entity, limit);

        let top_k = limit.unwrap_or(10) as usize;
        let limit_i32 = limit.unwrap_or(10);

        // 1. 首先尝试在知识图谱中查找该实体
        let entity_result = crate::db::KGRepository::get_entity_by_name(entity, None).await?;

        let mut entry_ids_with_scores: Vec<(String, f64)> = Vec::new();

        if let Some(found_entity) = entity_result {
            info!(
                "Found entity in KG: entity_id={}, entity_name={}",
                found_entity.entity_id, found_entity.entity_name
            );

            // 2. 如果找到实体，获取相关的知识条目
            let pool = crate::db::pool();
            let kg_results = crate::db::KGRepository::search_knowledge_by_entity(
                pool,
                &found_entity.entity_name,
                Some(limit_i32),
            )
            .await?;
            for entity in kg_results {
                entry_ids_with_scores
                    .push((entity.entity_id.clone(), entity.popularity_score as f64));
            }

            // 3. 获取相关实体，并搜索相关实体的知识条目
            let related_entities = crate::db::KGRepository::get_related_entities(
                &found_entity.entity_id,
                None,
                Some(5),
            )
            .await?;
            for (related_entity, relation) in related_entities {
                let related_results = crate::db::KGRepository::search_knowledge_by_entity(
                    pool,
                    &related_entity.entity_name,
                    Some(limit_i32 / 2),
                )
                .await?;
                for entity in related_results {
                    // 相关实体的分数要乘以关系权重
                    entry_ids_with_scores.push((
                        entity.entity_id,
                        (entity.popularity_score as f64) * relation.weight,
                    ));
                }
            }
        } else {
            // 4. 如果实体不存在于知识图谱中，搜索包含该实体名称的知识条目
            info!("Entity not found in KG, searching entries containing entity name");
            let pool = crate::db::pool();
            let text_results =
                crate::db::KGRepository::search_entries_by_entity(pool, entity, limit_i32).await?;
            for entity in text_results {
                entry_ids_with_scores.push((entity.entity_id, entity.popularity_score as f64));
            }
        }

        // 5. 去重并排序
        use std::collections::HashMap;
        let mut entry_scores: HashMap<String, f64> = HashMap::new();
        for (entry_id, score) in entry_ids_with_scores {
            *entry_scores.entry(entry_id).or_insert(0.0) += score;
        }

        let mut sorted_entries: Vec<(String, f64)> = entry_scores.into_iter().collect();
        sorted_entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted_entries.truncate(top_k);

        // 6. 获取完整的知识条目信息
        let mut results = Vec::new();
        for (entry_id, score) in sorted_entries {
            if let Ok(Some(entry)) = crate::db::ltm::LTMRepository::get_entry_by_id(&entry_id).await
            {
                results.push(SearchResult {
                    entry_id: entry.entry_id,
                    score: score as f32,
                    content: entry.content,
                    title: entry.title,
                    metadata: serde_json::json!({ "entity": entity }),
                });
            }
        }

        info!("Entity search completed: found {} results", results.len());
        Ok(results)
    }

    /// 应用 rerank 重排序
    #[instrument(skip(candidates))]
    async fn apply_rerank(
        query: &str,
        candidates: Vec<SearchResult>,
    ) -> Result<Vec<SearchResult>, AppError> {
        if candidates.is_empty() {
            return Ok(candidates);
        }

        let rerank_service = match get_rerank_service() {
            Ok(service) => service,
            Err(e) => {
                warn!(
                    "Failed to get rerank service: {}, falling back to original order",
                    e
                );
                return Ok(candidates); // 如果 rerank 服务不可用，返回原始结果
            }
        };

        // 准备候选数据：提取内容和原始分数
        let candidate_data: Vec<(String, f32)> = candidates
            .iter()
            .map(|r| {
                // 使用摘要或内容的前500个字符作为 rerank 输入
                let content = r
                    .metadata
                    .get("summary")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        // 如果元数据中没有摘要，使用内容的前500个字符
                        r.content.chars().take(500).collect()
                    });
                (content, r.score)
            })
            .collect();

        // 调用 rerank 服务
        let rerank_results = match rerank_service.rerank(query, &candidate_data).await {
            Ok(results) => results,
            Err(e) => {
                error!("Rerank failed: {}, falling back to original order", e);
                return Ok(candidates); // 如果 rerank 失败，返回原始结果
            }
        };

        // 根据 rerank 结果重新排序
        let mut reranked_candidates: Vec<SearchResult> = rerank_results
            .into_iter()
            .map(|rr| {
                let mut result = candidates[rr.index].clone();
                result.score = rr.score; // 使用 rerank 分数
                result
            })
            .collect();

        // 按 rerank 分数降序排序（rerank 服务已经排序，但为了确保）
        reranked_candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        info!(
            "Rerank completed: reordered {} candidates",
            reranked_candidates.len()
        );
        Ok(reranked_candidates)
    }

    /// 根据最低分数阈值过滤结果
    fn filter_by_threshold(results: Vec<SearchResult>, threshold: f32) -> Vec<SearchResult> {
        let original_count = results.len();
        let filtered: Vec<SearchResult> = results
            .into_iter()
            .filter(|r| r.score >= threshold)
            .collect();

        info!(
            "Filtered by threshold {}: {} -> {} results",
            threshold,
            original_count,
            filtered.len()
        );
        filtered
    }
}
