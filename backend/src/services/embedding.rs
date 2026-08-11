use anyhow::Result;
use moka::future::Cache;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tracing::{error, info, instrument};

use crate::config;

/// Error classification for embedding generation, so the LTM write path can map
/// a failure to the right HTTP status (D-e2): a genuinely unreachable/erroring
/// backend is `503` (dependency unavailable, retryable), while a reachable
/// backend that returns unusable output is `500` (a real error that must
/// surface, not be silently masked). Mirrors `llm::LlmError`.
///
/// This is carried inside `anyhow::Error` (via `anyhow::Error::new`), so it is
/// downcastable at the call site WITHOUT changing `generate_embedding`'s
/// `anyhow::Result<Vec<f32>>` signature — `probe()` and other callers are
/// unaffected.
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    /// Backend could not be reached (connection refused, DNS, timeout).
    #[error("embedding backend unavailable: {0}")]
    Unavailable(String),
    /// Backend was reached but returned a non-success HTTP status.
    #[error("embedding backend returned error status {status}")]
    Upstream { status: u16 },
    /// Backend responded, but the payload was unparseable / had the wrong shape
    /// / wrong dimension. Indicates a real bug or model/config mismatch, NOT a
    /// transient outage.
    #[error("embedding response could not be used: {0}")]
    Malformed(String),
}

/// 嵌入服务，用于生成文本向量
pub struct EmbeddingService {
    client: Client,
    base_url: String,
    model: String,
    dimension: usize,
    timeout: Duration,
    cache: Cache<String, Vec<f32>>,
    api_type: String,
    api_key: Option<String>,
}

impl EmbeddingService {
    /// 创建新的嵌入服务实例
    pub fn new() -> Result<Self> {
        let config = config::get();
        let timeout = Duration::from_secs(config.embedding.timeout_seconds);

        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;

        // When auto_detect is enabled and hardware capabilities are available,
        // prefer the hardware-routed model over the static config values.
        let (model, dimension) = if config.embedding.auto_detect {
            if let Some(caps) = crate::services::hardware_detector::get() {
                let rec = crate::services::model_router::recommend_embedding(
                    caps,
                    &config.embedding.base_url,
                );
                info!(
                    "Hardware auto-detect: using model='{}' dimension={} ({})",
                    rec.model, rec.dimension, rec.reasoning
                );
                (rec.model, rec.dimension)
            } else {
                (config.embedding.model.clone(), config.embedding.dimension)
            }
        } else {
            (config.embedding.model.clone(), config.embedding.dimension)
        };

        info!(
            "Embedding service initialized: base_url={}, model={}, dimension={}, api_type={}",
            config.embedding.base_url, model, dimension, config.embedding.api_type
        );

        // 初始化嵌入缓存，容量为10000个条目，过期时间为24小时
        let cache = Cache::builder()
            .max_capacity(10000)
            .time_to_live(Duration::from_secs(24 * 60 * 60))
            .build();

        Ok(Self {
            client,
            base_url: config.embedding.base_url.clone(),
            model,
            dimension,
            timeout,
            cache,
            api_type: config.embedding.api_type.clone(),
            api_key: config.embedding.api_key.clone(),
        })
    }

    /// 轻量可达性探测：只确认 embedding 后端在线，**不做推理**。
    ///
    /// 用于 readiness 探针（`/readyz`）。embedding 是硬依赖——向量在 LTM 写入与
    /// 检索的热路径上生成，后端不可达则这两条路径都不可用，所以实例应被摘出
    /// 负载均衡。
    ///
    /// 刻意**不**调用 [`Self::generate_embedding`]：kubelet 会按秒级频率轮询
    /// readiness，用真实推理做探针会给 embedding 后端持续施加负载，而推理延迟
    /// （常在 100ms–1s）也会让探针超时率显著上升。这里只打后端的模型列表端点，
    /// 那是 O(1) 的元数据查询。
    ///
    /// 探测**不**消耗 [`Self::cache`]，也不写入它。
    pub async fn probe(&self) -> Result<()> {
        // Ollama: GET /api/tags；OpenAI 兼容: GET /models。两者都只返回模型元数据。
        let url = if self.api_type == "openai" {
            format!("{}/models", self.base_url)
        } else {
            format!("{}/api/tags", self.base_url)
        };

        let mut request = self.client.get(&url).timeout(self.timeout);
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("embedding backend unreachable at {url}: {e}"))?;

        if !response.status().is_success() {
            anyhow::bail!("embedding backend at {url} returned {}", response.status());
        }
        Ok(())
    }

    /// 生成文本的向量嵌入
    #[instrument(skip(self))]
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // 检查缓存
        if let Some(embedding) = self.cache.get(text) {
            info!("Cache hit for embedding: text_length={}", text.len());
            return Ok(embedding);
        }

        info!(
            "Cache miss for embedding, generating new: text_length={}",
            text.len()
        );

        let embedding = if self.api_type == "openai" {
            self.generate_embedding_openai(text).await?
        } else {
            self.generate_embedding_ollama(text).await?
        };

        // 验证向量维度
        if embedding.len() != self.dimension {
            error!(
                "Embedding dimension mismatch: expected={}, got={}",
                self.dimension,
                embedding.len()
            );
            return Err(anyhow::Error::new(EmbeddingError::Malformed(format!(
                "dimension mismatch: expected {}, got {}",
                self.dimension,
                embedding.len()
            ))));
        }

        // 将生成的嵌入存入缓存
        self.cache.insert(text.to_string(), embedding.clone()).await;
        info!(
            "Embedding generated and cached successfully, dimension={}",
            embedding.len()
        );
        Ok(embedding)
    }

    /// 通过 Ollama API 生成嵌入
    async fn generate_embedding_ollama(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);

        let request_body = json!({
            "model": self.model,
            "prompt": text
        });

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send request to Ollama embeddings API: {}", e);
                anyhow::Error::new(EmbeddingError::Unavailable(e.to_string()))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "Ollama embeddings API returned error: status={}, body={}",
                status, error_text
            );
            return Err(anyhow::Error::new(EmbeddingError::Upstream {
                status: status.as_u16(),
            }));
        }

        let ollama_response: OllamaEmbeddingResponse = response.json().await.map_err(|e| {
            error!("Failed to parse Ollama embedding response: {}", e);
            anyhow::Error::new(EmbeddingError::Malformed(e.to_string()))
        })?;

        Ok(ollama_response.embedding)
    }

    /// 通过 OpenAI 兼容 API 生成嵌入（含 DashScope）
    async fn generate_embedding_openai(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/embeddings", self.base_url);

        let request_body = json!({
            "model": self.model,
            "input": text
        });

        let mut request = self.client.post(&url).json(&request_body);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request.send().await.map_err(|e| {
            error!(
                "Failed to send request to OpenAI-compatible embeddings API: {}",
                e
            );
            anyhow::Error::new(EmbeddingError::Unavailable(e.to_string()))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "OpenAI-compatible embeddings API returned error: status={}, body={}",
                status, error_text
            );
            return Err(anyhow::Error::new(EmbeddingError::Upstream {
                status: status.as_u16(),
            }));
        }

        let openai_response: OpenAIEmbeddingResponse = response.json().await.map_err(|e| {
            error!(
                "Failed to parse OpenAI-compatible embedding response: {}",
                e
            );
            anyhow::Error::new(EmbeddingError::Malformed(e.to_string()))
        })?;

        openai_response
            .data
            .first()
            .map(|d| d.embedding.clone())
            .ok_or_else(|| {
                anyhow::Error::new(EmbeddingError::Malformed(
                    "no embedding data in response".to_string(),
                ))
            })
    }

    /// 批量生成文本向量
    #[instrument(skip(self))]
    pub async fn generate_embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        info!("Generating embeddings for {} texts", texts.len());

        let mut embeddings = Vec::with_capacity(texts.len());

        for text in texts {
            let embedding = self.generate_embedding(text).await?;
            embeddings.push(embedding);
        }

        info!("Generated {} embeddings successfully", embeddings.len());
        Ok(embeddings)
    }

    /// 获取向量维度
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// 获取模型名称
    pub fn model(&self) -> &str {
        &self.model
    }
}

/// Ollama 嵌入 API 响应
#[derive(Debug, Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f32>,
}

/// OpenAI 兼容嵌入 API 响应（含 DashScope）
#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<OpenAIEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingData {
    embedding: Vec<f32>,
}

/// 全局嵌入服务实例
static EMBEDDING_SERVICE: once_cell::sync::OnceCell<EmbeddingService> =
    once_cell::sync::OnceCell::new();

/// 获取全局嵌入服务实例
pub fn get_embedding_service() -> Result<&'static EmbeddingService> {
    EMBEDDING_SERVICE
        .get_or_try_init(|| EmbeddingService::new())
        .map_err(|e| anyhow::anyhow!("Failed to initialize embedding service: {}", e))
}
