use anyhow::Result;
use moka::future::Cache;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tracing::{error, info, instrument};

use crate::config;

/// 嵌入服务，用于生成文本向量
pub struct EmbeddingService {
    client: Client,
    base_url: String,
    model: String,
    dimension: usize,
    timeout: Duration,
    cache: Cache<String, Vec<f32>>,
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

        info!(
            "Embedding service initialized: base_url={}, model={}, dimension={}",
            config.embedding.base_url, config.embedding.model, config.embedding.dimension
        );

        // 初始化嵌入缓存，容量为10000个条目，过期时间为24小时
        let cache = Cache::builder()
            .max_capacity(10000)
            .time_to_live(Duration::from_secs(24 * 60 * 60))
            .build();

        Ok(Self {
            client,
            base_url: config.embedding.base_url.clone(),
            model: config.embedding.model.clone(),
            dimension: config.embedding.dimension,
            timeout,
            cache,
        })
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
                anyhow::anyhow!("Failed to generate embedding: {}", e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "Ollama embeddings API returned error: status={}, body={}",
                status, error_text
            );
            return Err(anyhow::anyhow!(
                "Ollama embeddings API error: status={}",
                status
            ));
        }

        let ollama_response: OllamaEmbeddingResponse = response.json().await.map_err(|e| {
            error!("Failed to parse Ollama embedding response: {}", e);
            anyhow::anyhow!("Failed to parse embedding response: {}", e)
        })?;

        let embedding = ollama_response.embedding;

        // 验证向量维度
        if embedding.len() != self.dimension {
            error!(
                "Embedding dimension mismatch: expected={}, got={}",
                self.dimension,
                embedding.len()
            );
            return Err(anyhow::anyhow!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.dimension,
                embedding.len()
            ));
        }

        // 将生成的嵌入存入缓存
        self.cache.insert(text.to_string(), embedding.clone()).await;
        info!(
            "Embedding generated and cached successfully, dimension={}",
            embedding.len()
        );
        Ok(embedding)
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

/// 全局嵌入服务实例
static EMBEDDING_SERVICE: once_cell::sync::OnceCell<EmbeddingService> =
    once_cell::sync::OnceCell::new();

/// 获取全局嵌入服务实例
pub fn get_embedding_service() -> Result<&'static EmbeddingService> {
    EMBEDDING_SERVICE
        .get_or_try_init(|| EmbeddingService::new())
        .map_err(|e| anyhow::anyhow!("Failed to initialize embedding service: {}", e))
}
