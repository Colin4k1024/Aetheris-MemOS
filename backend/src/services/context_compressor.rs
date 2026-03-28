/// 上下文压缩服务 — Issue #54
///
/// 对话上下文 / 记忆内容超长时，通过 LLM 摘要或滑动窗口策略进行压缩，
/// 使得送入模型的 Token 数量保持在可控范围内。
///
/// 策略：
/// 1. **滑动窗口**（无 LLM 调用）— 保留最新 N 条消息，丢弃旧消息
/// 2. **LLM 摘要**（有 LLM 调用）— 将多条历史消息合并为一条摘要消息
/// 3. **重要性裁剪**（无 LLM 调用）— 按 importance_score 从低到高裁剪，直至满足 token 预算
/// 4. **分层压缩**（混合）— 先用窗口保留最新 K 条，再对旧消息做 LLM 摘要，合并后返回

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::db::pool;
use crate::tenant::get_default_tenant;
use crate::AppError;

/// 压缩策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompressionStrategy {
    /// 保留最新 N 条消息
    SlidingWindow,
    /// LLM 摘要压缩
    LlmSummary,
    /// 按重要性裁剪
    ImportancePrune,
    /// 分层压缩：窗口 + LLM 摘要
    Hierarchical,
}

/// 压缩配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub strategy: CompressionStrategy,
    /// 目标 Token 预算（粗估：1 token ≈ 4 bytes）
    pub token_budget: usize,
    /// 滑动窗口保留条数
    pub window_size: usize,
    /// 分层压缩中，窗口保留最近 K 条
    pub hierarchical_recent_k: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            strategy: CompressionStrategy::Hierarchical,
            token_budget: 4096,
            window_size: 20,
            hierarchical_recent_k: 10,
        }
    }
}

/// 待压缩的消息条目
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MessageEntry {
    pub id: String,
    pub role: String,
    pub content: String,
    pub importance_score: Option<f32>,
    pub created_at: Option<String>,
}

/// 压缩结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CompressionResult {
    /// 压缩后的消息列表
    pub messages: Vec<MessageEntry>,
    /// 原始消息数量
    pub original_count: usize,
    /// 压缩后消息数量
    pub compressed_count: usize,
    /// 估算压缩前 token 数
    pub original_tokens: usize,
    /// 估算压缩后 token 数
    pub compressed_tokens: usize,
    /// 所用策略
    pub strategy_used: CompressionStrategy,
    /// 是否包含 LLM 摘要消息
    pub has_summary: bool,
}

/// 粗略估算内容的 token 数（1 token ≈ 4 bytes）
fn estimate_tokens(content: &str) -> usize {
    (content.len() + 3) / 4
}

/// 计算消息列表总 token 数
fn total_tokens(msgs: &[MessageEntry]) -> usize {
    msgs.iter().map(|m| estimate_tokens(&m.content)).sum()
}

/// 上下文压缩器
pub struct ContextCompressor;

impl ContextCompressor {
    /// 压缩消息列表，使其不超过 token_budget
    pub async fn compress(
        messages: Vec<MessageEntry>,
        cfg: &CompressionConfig,
    ) -> Result<CompressionResult, AppError> {
        let original_count = messages.len();
        let original_tokens = total_tokens(&messages);

        // 如果已在预算内，无需压缩
        if original_tokens <= cfg.token_budget || messages.is_empty() {
            return Ok(CompressionResult {
                compressed_count: original_count,
                compressed_tokens: original_tokens,
                original_count,
                original_tokens,
                messages,
                strategy_used: cfg.strategy,
                has_summary: false,
            });
        }

        info!(
            "Compressing {} messages ({} tokens) → budget {} tokens, strategy={:?}",
            original_count, original_tokens, cfg.token_budget, cfg.strategy
        );

        match cfg.strategy {
            CompressionStrategy::SlidingWindow => {
                Self::sliding_window(messages, cfg, original_count, original_tokens)
            }
            CompressionStrategy::ImportancePrune => {
                Self::importance_prune(messages, cfg, original_count, original_tokens)
            }
            CompressionStrategy::LlmSummary => {
                Self::llm_summary(messages, cfg, original_count, original_tokens).await
            }
            CompressionStrategy::Hierarchical => {
                Self::hierarchical(messages, cfg, original_count, original_tokens).await
            }
        }
    }

    /// 策略1：滑动窗口 — 保留最新 window_size 条
    fn sliding_window(
        mut messages: Vec<MessageEntry>,
        cfg: &CompressionConfig,
        original_count: usize,
        original_tokens: usize,
    ) -> Result<CompressionResult, AppError> {
        // 按 created_at 升序，保留尾部
        if messages.len() > cfg.window_size {
            let drop = messages.len() - cfg.window_size;
            messages = messages.into_iter().skip(drop).collect();
        }
        let compressed_tokens = total_tokens(&messages);
        Ok(CompressionResult {
            compressed_count: messages.len(),
            compressed_tokens,
            original_count,
            original_tokens,
            messages,
            strategy_used: CompressionStrategy::SlidingWindow,
            has_summary: false,
        })
    }

    /// 策略2：按重要性裁剪 — 从低重要性消息开始删除
    fn importance_prune(
        mut messages: Vec<MessageEntry>,
        cfg: &CompressionConfig,
        original_count: usize,
        original_tokens: usize,
    ) -> Result<CompressionResult, AppError> {
        // 按重要性升序排序（无 importance_score 的视为 0.5）
        let mut indexed: Vec<(usize, f32)> = messages
            .iter()
            .enumerate()
            .map(|(i, m)| (i, m.importance_score.unwrap_or(0.5)))
            .collect();
        indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // 标记需要删除的索引，直至满足 token 预算
        let mut to_remove = std::collections::HashSet::new();
        let mut current_tokens = original_tokens;
        for (idx, _score) in &indexed {
            if current_tokens <= cfg.token_budget {
                break;
            }
            current_tokens -= estimate_tokens(&messages[*idx].content);
            to_remove.insert(*idx);
        }

        let retained: Vec<MessageEntry> = messages
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !to_remove.contains(i))
            .map(|(_, m)| m)
            .collect();

        let compressed_tokens = total_tokens(&retained);
        Ok(CompressionResult {
            compressed_count: retained.len(),
            compressed_tokens,
            original_count,
            original_tokens,
            messages: retained,
            strategy_used: CompressionStrategy::ImportancePrune,
            has_summary: false,
        })
    }

    /// 策略3：LLM 摘要 — 将所有消息合并为一条摘要
    async fn llm_summary(
        messages: Vec<MessageEntry>,
        cfg: &CompressionConfig,
        original_count: usize,
        original_tokens: usize,
    ) -> Result<CompressionResult, AppError> {
        let summary_text = Self::call_llm_summary(&messages).await;
        let summary_msg = MessageEntry {
            id: format!("summary-{}", ulid::Ulid::new()),
            role: "system".to_string(),
            content: format!("[对话历史摘要]\n{}", summary_text),
            importance_score: Some(0.9),
            created_at: Some(chrono::Utc::now().to_rfc3339()),
        };
        let compressed_tokens = estimate_tokens(&summary_msg.content);
        Ok(CompressionResult {
            compressed_count: 1,
            compressed_tokens,
            original_count,
            original_tokens,
            messages: vec![summary_msg],
            strategy_used: CompressionStrategy::LlmSummary,
            has_summary: true,
        })
    }

    /// 策略4：分层压缩 — 保留最新 K 条，对旧消息做 LLM 摘要
    async fn hierarchical(
        messages: Vec<MessageEntry>,
        cfg: &CompressionConfig,
        original_count: usize,
        original_tokens: usize,
    ) -> Result<CompressionResult, AppError> {
        let recent_k = cfg.hierarchical_recent_k.min(messages.len());
        let split = messages.len() - recent_k;
        let (old_msgs, recent_msgs): (Vec<_>, Vec<_>) =
            messages.into_iter().enumerate().partition(|(i, _)| *i < split);
        let old_msgs: Vec<MessageEntry> = old_msgs.into_iter().map(|(_, m)| m).collect();
        let recent_msgs: Vec<MessageEntry> = recent_msgs.into_iter().map(|(_, m)| m).collect();

        let mut result_msgs: Vec<MessageEntry> = Vec::new();
        let mut has_summary = false;

        // 旧消息做摘要（若有）
        if !old_msgs.is_empty() {
            let summary_text = Self::call_llm_summary(&old_msgs).await;
            result_msgs.push(MessageEntry {
                id: format!("summary-{}", ulid::Ulid::new()),
                role: "system".to_string(),
                content: format!("[早期对话摘要]\n{}", summary_text),
                importance_score: Some(0.85),
                created_at: Some(chrono::Utc::now().to_rfc3339()),
            });
            has_summary = true;
        }

        result_msgs.extend(recent_msgs);

        // 若仍超出预算，再做一次滑动窗口裁剪
        let current_tokens = total_tokens(&result_msgs);
        if current_tokens > cfg.token_budget {
            let window_cfg = CompressionConfig {
                strategy: CompressionStrategy::SlidingWindow,
                token_budget: cfg.token_budget,
                window_size: cfg.window_size,
                hierarchical_recent_k: cfg.hierarchical_recent_k,
            };
            let r = Self::sliding_window(
                result_msgs,
                &window_cfg,
                original_count,
                original_tokens,
            )?;
            return Ok(CompressionResult {
                has_summary,
                strategy_used: CompressionStrategy::Hierarchical,
                ..r
            });
        }

        let compressed_tokens = total_tokens(&result_msgs);
        Ok(CompressionResult {
            compressed_count: result_msgs.len(),
            compressed_tokens,
            original_count,
            original_tokens,
            messages: result_msgs,
            strategy_used: CompressionStrategy::Hierarchical,
            has_summary,
        })
    }

    /// 调用 LLM 服务做摘要，若失败则降级为截断式摘要
    async fn call_llm_summary(messages: &[MessageEntry]) -> String {
        let combined: String = messages
            .iter()
            .map(|m| format!("[{}]: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        match crate::services::llm::get_llm_service() {
            Ok(svc) => match svc.summarize(&combined).await {
                Ok(summary) => summary,
                Err(e) => {
                    warn!("LLM summary failed ({}), falling back to truncation", e);
                    Self::truncation_summary(&combined)
                }
            },
            Err(e) => {
                warn!("LLM service unavailable ({}), using truncation summary", e);
                Self::truncation_summary(&combined)
            }
        }
    }

    /// 降级摘要：截取前 500 字符
    fn truncation_summary(text: &str) -> String {
        let limit = 500;
        if text.len() <= limit {
            text.to_string()
        } else {
            format!("{}…（已截断）", &text[..limit])
        }
    }

    /// 从 STM 会话中压缩指定会话的消息
    pub async fn compress_session(
        session_id: &str,
        cfg: &CompressionConfig,
    ) -> Result<CompressionResult, AppError> {
        let messages = crate::db::stm::STMRepository::get_session_messages(pool(), &get_default_tenant(), session_id, None)
            .await?
            .into_iter()
            .map(|m| MessageEntry {
                id: m.message_id,
                role: m.role,
                content: m.content,
                importance_score: m.importance_score,
                created_at: Some(m.created_at),
            })
            .collect();

        Self::compress(messages, cfg).await
    }
}
