/// 置信度评分服务 — Issue #53
///
/// 为记忆条目和搜索结果计算多维度置信度分数。
///
/// 维度：
/// 1. **内容质量** (`quality_score` 字段) — 存储时评估的质量
/// 2. **检索相关性** — 搜索时的向量/关键词/图谱综合分数
/// 3. **时效性** — 条目距当前时间越长，衰减越多
/// 4. **访问频率** — 被访问次数越多，置信度越高
/// 5. **完整性** — 内容长度 + 标题是否存在
///
/// 最终公式（可配置权重）：
/// `confidence = w_quality * quality + w_relevance * relevance + w_recency * recency + w_access * access + w_completeness * completeness`

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::debug;

/// 置信度评分配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceScorerConfig {
    /// 内容质量权重
    pub quality_weight: f32,
    /// 检索相关性权重
    pub relevance_weight: f32,
    /// 时效性权重
    pub recency_weight: f32,
    /// 访问频率权重
    pub access_weight: f32,
    /// 完整性权重
    pub completeness_weight: f32,
    /// 时效性半衰期（天数）：超过该天数分数衰减至 0.5
    pub recency_half_life_days: f32,
    /// 访问频率饱和点：达到该次数时 access 分数接近 1.0
    pub access_saturation: f32,
}

impl Default for ConfidenceScorerConfig {
    fn default() -> Self {
        Self {
            quality_weight: 0.30,
            relevance_weight: 0.35,
            recency_weight: 0.15,
            access_weight: 0.10,
            completeness_weight: 0.10,
            recency_half_life_days: 30.0,
            access_saturation: 20.0,
        }
    }
}

/// 单条记忆的置信度分解结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ConfidenceBreakdown {
    /// 综合置信度 [0, 1]
    pub confidence: f32,
    /// 内容质量分 [0, 1]
    pub quality_score: f32,
    /// 检索相关性分 [0, 1]
    pub relevance_score: f32,
    /// 时效性分 [0, 1]
    pub recency_score: f32,
    /// 访问频率分 [0, 1]
    pub access_score: f32,
    /// 完整性分 [0, 1]
    pub completeness_score: f32,
}

/// 置信度评分器（无状态，方法均为纯函数）
pub struct ConfidenceScorer;

impl ConfidenceScorer {
    /// 计算综合置信度。
    ///
    /// # 参数
    /// - `retrieval_score`: 搜索/向量/混合搜索返回的分数
    /// - `quality_score`: LTM 条目的 `quality_score` 字段（可为 None）
    /// - `created_at`: 条目创建时间（RFC3339 字符串）
    /// - `access_count`: 条目访问次数（可为 None）
    /// - `content_len`: 内容字节长度
    /// - `has_title`: 条目是否有标题
    /// - `cfg`: 评分配置
    pub fn score(
        retrieval_score: f32,
        quality_score: Option<f32>,
        created_at: &str,
        access_count: Option<i32>,
        content_len: usize,
        has_title: bool,
        cfg: &ConfidenceScorerConfig,
    ) -> ConfidenceBreakdown {
        // 1. 内容质量
        let qs = quality_score.unwrap_or(0.5_f32).clamp(0.0, 1.0);

        // 2. 检索相关性
        let rs = retrieval_score.clamp(0.0, 1.0);

        // 3. 时效性：指数衰减 e^(-ln2 * age_days / half_life)
        let recency = Self::recency_score(created_at, cfg.recency_half_life_days);

        // 4. 访问频率：对数归一化 log(1 + access_count) / log(1 + saturation)
        let access = {
            let count = access_count.unwrap_or(0).max(0) as f32;
            let sat = cfg.access_saturation.max(1.0);
            (1.0_f32 + count).ln() / (1.0_f32 + sat).ln()
        }
        .clamp(0.0, 1.0);

        // 5. 完整性：内容长度（> 100 字节为满分）+ 标题存在性
        let completeness = {
            let len_score = (content_len as f32 / 100.0).min(1.0);
            let title_bonus = if has_title { 0.2 } else { 0.0 };
            (len_score * 0.8 + title_bonus).clamp(0.0, 1.0)
        };

        // 归一化权重之和
        let wsum = cfg.quality_weight
            + cfg.relevance_weight
            + cfg.recency_weight
            + cfg.access_weight
            + cfg.completeness_weight;
        let wsum = if wsum > 0.0 { wsum } else { 1.0 };

        let confidence = (cfg.quality_weight * qs
            + cfg.relevance_weight * rs
            + cfg.recency_weight * recency
            + cfg.access_weight * access
            + cfg.completeness_weight * completeness)
            / wsum;

        debug!(
            "Confidence breakdown: q={:.3}, r={:.3}, rec={:.3}, acc={:.3}, comp={:.3} → {:.3}",
            qs, rs, recency, access, completeness, confidence
        );

        ConfidenceBreakdown {
            confidence: confidence.clamp(0.0, 1.0),
            quality_score: qs,
            relevance_score: rs,
            recency_score: recency,
            access_score: access,
            completeness_score: completeness,
        }
    }

    /// 时效性分数：指数衰减
    fn recency_score(created_at: &str, half_life_days: f32) -> f32 {
        let age_days = created_at
            .parse::<DateTime<Utc>>()
            .ok()
            .map(|t| {
                let secs = (Utc::now() - t).num_seconds().max(0) as f32;
                secs / 86400.0
            })
            .unwrap_or(0.0); // 无法解析则视为刚创建

        let lambda = std::f32::consts::LN_2 / half_life_days.max(1.0);
        (-lambda * age_days).exp().clamp(0.0, 1.0)
    }

    /// 批量为 SearchResult 列表附加置信度（使用 retrieval score + LTM 元数据）
    pub async fn score_search_results(
        results: &[crate::services::memory_search::SearchResult],
        cfg: &ConfidenceScorerConfig,
    ) -> Vec<ScoredSearchResult> {
        let mut scored = Vec::with_capacity(results.len());
        for r in results {
            // 尝试从 LTM 获取完整条目元数据
            let breakdown = if let Ok(Some(entry)) =
                crate::db::ltm::LTMRepository::get_entry_by_id(&r.entry_id).await
            {
                Self::score(
                    r.score,
                    entry.quality_score,
                    &entry.created_at,
                    entry.access_count,
                    entry.content.len(),
                    entry.title.is_some(),
                    cfg,
                )
            } else {
                // 回退：只用检索分数
                Self::score(r.score, None, "", None, r.content.len(), r.title.is_some(), cfg)
            };

            scored.push(ScoredSearchResult {
                entry_id: r.entry_id.clone(),
                score: r.score,
                content: r.content.clone(),
                title: r.title.clone(),
                metadata: r.metadata.clone(),
                confidence: breakdown,
            });
        }
        // 按综合置信度降序排列
        scored.sort_by(|a, b| {
            b.confidence
                .confidence
                .partial_cmp(&a.confidence.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored
    }
}

/// 携带置信度分解的搜索结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ScoredSearchResult {
    pub entry_id: String,
    pub score: f32,
    pub content: String,
    pub title: Option<String>,
    pub metadata: serde_json::Value,
    pub confidence: ConfidenceBreakdown,
}
