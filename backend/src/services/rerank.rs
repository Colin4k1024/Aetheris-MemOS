use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tracing::{error, info, instrument, warn};

use crate::config;

/// Rerank 服务，用于对搜索结果进行重新排序
pub struct RerankService {
    client: Client,
    base_url: String,
    model: String,
    timeout: Duration,
}

/// Rerank 结果项
#[derive(Debug, Clone)]
pub struct RerankResult {
    pub index: usize,
    pub score: f32,
}

impl RerankService {
    /// 创建新的 Rerank 服务实例
    pub fn new() -> Result<Self> {
        let config = config::get();
        let timeout = Duration::from_secs(config.rerank.timeout_seconds);

        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;

        info!(
            "Rerank service initialized: base_url={}, model={}",
            config.rerank.base_url, config.rerank.model
        );

        Ok(Self {
            client,
            base_url: config.rerank.base_url.clone(),
            model: config.rerank.model.clone(),
            timeout,
        })
    }

    /// 对搜索结果进行重新排序
    ///
    /// # 参数
    /// - `query`: 查询文本
    /// - `candidates`: 候选结果列表，每个元素包含内容和原始分数
    ///
    /// # 返回
    /// 返回重新排序后的结果，包含索引和新的相关性分数
    #[instrument(skip(self))]
    pub async fn rerank(
        &self,
        query: &str,
        candidates: &[(String, f32)], // (content, original_score)
    ) -> Result<Vec<RerankResult>> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        info!(
            "Reranking {} candidates for query: {}",
            candidates.len(),
            query
        );

        // 由于 Ollama 可能不直接支持 rerank API，我们使用 LLM 进行相关性评分
        // 方案：为每个候选结果生成相关性评分
        let mut rerank_results = Vec::with_capacity(candidates.len());

        // 批量处理以提高效率（可以并行处理）
        for (index, (content, _original_score)) in candidates.iter().enumerate() {
            let score = self
                .score_relevance(query, content)
                .await
                .unwrap_or_else(|e| {
                    warn!("Failed to score relevance for candidate {}: {}", index, e);
                    // 如果评分失败，使用原始分数（归一化到 0-1）
                    0.5
                });

            rerank_results.push(RerankResult { index, score });
        }

        // 按分数降序排序
        rerank_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        info!(
            "Reranking completed: processed {} candidates",
            rerank_results.len()
        );
        Ok(rerank_results)
    }

    /// 使用 LLM 对单个文档进行相关性评分
    async fn score_relevance(&self, query: &str, document: &str) -> Result<f32> {
        let prompt = format!(
            r#"请评估以下文档与查询的相关性，返回一个 0 到 1 之间的分数（0 表示完全不相关，1 表示完全相关）。

查询：{}

文档：{}

请只返回一个浮点数（0.0 到 1.0 之间），不要包含任何其他文字。"#,
            query, document
        );

        let url = format!("{}/api/generate", self.base_url);

        let request_body = json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.1, // 低温度以获得更稳定的评分
            }
        });

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send rerank request to Ollama: {}", e);
                anyhow::anyhow!("Failed to call rerank: {}", e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "Ollama API returned error: status={}, body={}",
                status, error_text
            );
            return Err(anyhow::anyhow!("Ollama API error: status={}", status));
        }

        let ollama_response: OllamaResponse = response.json().await.map_err(|e| {
            error!("Failed to parse Ollama response: {}", e);
            anyhow::anyhow!("Failed to parse response: {}", e)
        })?;

        // 从响应中提取分数
        let score_text = ollama_response.response.trim();
        let score: f32 = score_text
            .parse::<f32>()
            .unwrap_or_else(|_| {
                // 如果解析失败，尝试提取数字
                let numbers: Vec<&str> = score_text
                    .split_whitespace()
                    .filter(|s| s.chars().any(|c| c.is_ascii_digit() || c == '.'))
                    .collect();

                if let Some(first_num) = numbers.first() {
                    first_num.parse::<f32>().unwrap_or(0.5)
                } else {
                    0.5
                }
            })
            .clamp(0.0_f32, 1.0_f32); // 确保分数在 0-1 范围内

        Ok(score)
    }
}

/// Ollama API 响应
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

/// 全局 Rerank 服务实例
static RERANK_SERVICE: once_cell::sync::OnceCell<RerankService> = once_cell::sync::OnceCell::new();

/// 获取全局 Rerank 服务实例
pub fn get_rerank_service() -> Result<&'static RerankService> {
    RERANK_SERVICE
        .get_or_try_init(|| RerankService::new())
        .map_err(|e| anyhow::anyhow!("Failed to initialize rerank service: {}", e))
}
