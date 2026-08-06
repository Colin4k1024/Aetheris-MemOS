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
    api_type: String,
    api_key: Option<String>,
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
            "Rerank service initialized: base_url={}, model={}, api_type={}",
            config.rerank.base_url, config.rerank.model, config.rerank.api_type
        );

        Ok(Self {
            client,
            base_url: config.rerank.base_url.clone(),
            model: config.rerank.model.clone(),
            timeout,
            api_type: config.rerank.api_type.clone(),
            api_key: config.rerank.api_key.clone(),
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

        // 使用 DashScope 原生 Rerank API（qwen3-rerank 等专用 rerank 模型）
        if self.model.contains("rerank") {
            return self.rerank_dashscope_native(query, candidates).await;
        }

        // 使用 LLM 评分模式（兼容 Ollama 和 OpenAI 兼容格式）
        const RERANK_CONCURRENCY: usize = 8;
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let model = self.model.clone();
        let query = query.to_string();

        let indexed: Vec<(usize, String)> = candidates
            .iter()
            .enumerate()
            .map(|(index, (content, _original_score))| (index, content.clone()))
            .collect();

        let mut rerank_results: Vec<RerankResult> = Vec::with_capacity(indexed.len());
        for chunk in indexed.chunks(RERANK_CONCURRENCY) {
            let batch = chunk.iter().map(|(index, content)| {
                let client = client.clone();
                let base_url = base_url.clone();
                let model = model.clone();
                let api_type = self.api_type.clone();
                let api_key = self.api_key.clone();
                let query = query.clone();
                let content = content.clone();
                let index = *index;
                async move {
                    let score = Self::score_relevance(
                        &client, &base_url, &model, &api_type, &api_key, &query, &content,
                    )
                    .await
                    .unwrap_or_else(|e| {
                        warn!("Failed to score relevance for candidate {}: {}", index, e);
                        0.5
                    });
                    RerankResult { index, score }
                }
            });
            rerank_results.extend(futures::future::join_all(batch).await);
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
    async fn score_relevance(
        client: &Client,
        base_url: &str,
        model: &str,
        api_type: &str,
        api_key: &Option<String>,
        query: &str,
        document: &str,
    ) -> Result<f32> {
        let prompt = format!(
            r#"请评估以下文档与查询的相关性，返回一个 0 到 1 之间的分数（0 表示完全不相关，1 表示完全相关）。

查询：{}

文档：{}

请只返回一个浮点数（0.0 到 1.0 之间），不要包含任何其他文字。"#,
            query, document
        );

        let response_text = if api_type == "openai" {
            // OpenAI 兼容格式
            let url = format!("{}/chat/completions", base_url);

            let request_body = json!({
                "model": model,
                "messages": [{"role": "user", "content": prompt}],
                "stream": false,
                "temperature": 0.1,
            });

            let mut request = client.post(&url).json(&request_body);

            if let Some(ref key) = api_key {
                request = request.header("Authorization", format!("Bearer {}", key));
            }

            let response = request.send().await.map_err(|e| {
                error!(
                    "Failed to send rerank request to OpenAI-compatible API: {}",
                    e
                );
                anyhow::anyhow!("Failed to call rerank: {}", e)
            })?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                error!(
                    "OpenAI-compatible API returned error: status={}, body={}",
                    status, error_text
                );
                return Err(anyhow::anyhow!(
                    "OpenAI-compatible API error: status={}",
                    status
                ));
            }

            let openai_response: OpenAIChatResponse = response.json().await.map_err(|e| {
                error!("Failed to parse OpenAI-compatible response: {}", e);
                anyhow::anyhow!("Failed to parse response: {}", e)
            })?;

            openai_response
                .choices
                .first()
                .map(|c| c.message.content.clone())
                .unwrap_or_default()
        } else {
            // Ollama 格式
            let url = format!("{}/api/generate", base_url);

            let request_body = json!({
                "model": model,
                "prompt": prompt,
                "stream": false,
                "options": {
                    "temperature": 0.1,
                }
            });

            let response = client
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

            ollama_response.response
        };

        // 从响应中提取分数
        let score_text = response_text.trim();
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

    /// 使用 DashScope 原生 Rerank API
    async fn rerank_dashscope_native(
        &self,
        query: &str,
        candidates: &[(String, f32)],
    ) -> Result<Vec<RerankResult>> {
        let url =
            "https://dashscope.aliyuncs.com/api/v1/services/rerank/text-reranking/text-reranking";

        let documents: Vec<String> = candidates
            .iter()
            .map(|(content, _)| content.clone())
            .collect();

        let request_body = json!({
            "model": self.model,
            "input": {
                "query": query,
                "documents": documents
            },
            "parameters": {
                "top_n": candidates.len(),
                "return_documents": false
            }
        });

        let mut request = self.client.post(url).json(&request_body);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request.send().await.map_err(|e| {
            error!("Failed to send rerank request to DashScope: {}", e);
            anyhow::anyhow!("Failed to call rerank: {}", e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "DashScope rerank API returned error: status={}, body={}",
                status, error_text
            );
            return Err(anyhow::anyhow!(
                "DashScope rerank API error: status={}",
                status
            ));
        }

        let dashscope_response: DashScopeRerankResponse = response.json().await.map_err(|e| {
            error!("Failed to parse DashScope rerank response: {}", e);
            anyhow::anyhow!("Failed to parse rerank response: {}", e)
        })?;

        let mut rerank_results: Vec<RerankResult> = dashscope_response
            .output
            .results
            .iter()
            .map(|r| RerankResult {
                index: r.index,
                score: r.relevance_score,
            })
            .collect();

        // 按分数降序排序
        rerank_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        info!(
            "DashScope reranking completed: processed {} candidates",
            rerank_results.len()
        );
        Ok(rerank_results)
    }
}

/// Ollama API 响应
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

/// OpenAI 兼容 API 响应（含 DashScope）
#[derive(Debug, Deserialize)]
struct OpenAIChatResponse {
    choices: Vec<OpenAIChatChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatChoice {
    message: OpenAIChatMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatMessage {
    content: String,
}

/// DashScope Rerank API 响应
#[derive(Debug, Deserialize)]
struct DashScopeRerankResponse {
    output: DashScopeRerankOutput,
}

#[derive(Debug, Deserialize)]
struct DashScopeRerankOutput {
    results: Vec<DashScopeRerankResult>,
}

#[derive(Debug, Deserialize)]
struct DashScopeRerankResult {
    index: usize,
    relevance_score: f32,
}

/// 全局 Rerank 服务实例
static RERANK_SERVICE: once_cell::sync::OnceCell<RerankService> = once_cell::sync::OnceCell::new();

/// 获取全局 Rerank 服务实例
pub fn get_rerank_service() -> Result<&'static RerankService> {
    RERANK_SERVICE
        .get_or_try_init(|| RerankService::new())
        .map_err(|e| anyhow::anyhow!("Failed to initialize rerank service: {}", e))
}
