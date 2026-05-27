//! Mem0 Provider - HTTP API integration

use std::time::Instant;

use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::provider::*;
use crate::kernel::types::*;
use crate::providers::circuit_breaker::CircuitBreaker;
use crate::providers::config::ExternalProviderConfig;
use crate::providers::validate_path_segment;

pub struct Mem0Provider {
    config: ExternalProviderConfig,
    client: reqwest::Client,
    circuit_breaker: CircuitBreaker,
}

impl Mem0Provider {
    pub fn new(config: ExternalProviderConfig) -> Self {
        let timeout = std::time::Duration::from_millis(config.timeout_ms);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("failed to build HTTP client — TLS backend unavailable");

        Self {
            config,
            client,
            circuit_breaker: CircuitBreaker::default(),
        }
    }

    fn base_url(&self) -> &str {
        &self.config.api_url
    }

    fn check_circuit(&self) -> MemoryResult<()> {
        if !self.circuit_breaker.is_allowed() {
            return Err(MemoryError::Storage(
                "Mem0 provider circuit breaker is open".to_string(),
            ));
        }
        Ok(())
    }

    fn build_request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url(), path);
        let mut req = self.client.request(method, &url);
        if let Some(key) = self.config.resolve_api_key() {
            req = req.header("Authorization", format!("Bearer {key}"));
        }
        req
    }
}

#[async_trait::async_trait]
impl MemoryProvider for Mem0Provider {
    fn provider_name(&self) -> &str {
        "mem0"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_vector_search: true,
            supports_graph: false,
            supports_metadata_filter: true,
            supports_eviction: false,
            max_entry_size_bytes: Some(10 * 1024 * 1024),
        }
    }

    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId> {
        self.check_circuit()?;

        let body = serde_json::json!({
            "content": entry.content,
            "metadata": entry.metadata,
        });

        let resp = self
            .build_request(reqwest::Method::POST, "/v1/memories/")
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                self.circuit_breaker.record_success();
                let data: serde_json::Value = r
                    .json()
                    .await
                    .map_err(|e| MemoryError::Serialization(e.to_string()))?;
                let id_str = data["id"].as_str().unwrap_or(&entry.id.0);
                Ok(MemoryId::from_string(id_str))
            }
            Ok(r) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!(
                    "Mem0 API error: {}",
                    r.status()
                )))
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!("Mem0 request failed: {e}")))
            }
        }
    }

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry> {
        self.check_circuit()?;
        validate_path_segment(id.as_str())?;

        let resp = self
            .build_request(
                reqwest::Method::GET,
                &format!("/v1/memories/{}/", id.as_str()),
            )
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                self.circuit_breaker.record_success();
                let data: serde_json::Value = r
                    .json()
                    .await
                    .map_err(|e| MemoryError::Serialization(e.to_string()))?;
                let content_text = data["memory"].as_str().unwrap_or("");
                let now = chrono::Utc::now().timestamp();
                Ok(MemoryEntry {
                    id: id.clone(),
                    layer: LayerType::Ltm,
                    content: MemoryContent::Text(content_text.to_string()),
                    metadata: MemoryMetadata::default(),
                    created_at: now,
                    updated_at: now,
                })
            }
            Ok(r) if r.status().as_u16() == 404 => {
                self.circuit_breaker.record_success();
                Err(MemoryError::NotFound(format!(
                    "Mem0 memory not found: {}",
                    id.as_str()
                )))
            }
            Ok(r) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!(
                    "Mem0 API error: {}",
                    r.status()
                )))
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!("Mem0 request failed: {e}")))
            }
        }
    }

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        self.check_circuit()?;

        let search_text = query.text.as_deref().unwrap_or("");
        let body = serde_json::json!({
            "query": search_text,
            "limit": query.limit,
        });

        let resp = self
            .build_request(reqwest::Method::POST, "/v1/memories/search/")
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                self.circuit_breaker.record_success();
                let data: serde_json::Value = r
                    .json()
                    .await
                    .map_err(|e| MemoryError::Serialization(e.to_string()))?;

                let results = data["results"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|item| {
                        let id = item["id"].as_str()?;
                        let memory = item["memory"].as_str().unwrap_or("");
                        let score = item["score"].as_f64().unwrap_or(0.0);
                        let now = chrono::Utc::now().timestamp();
                        Some(MemoryMatch {
                            entry: MemoryEntry {
                                id: MemoryId::from_string(id),
                                layer: LayerType::Ltm,
                                content: MemoryContent::Text(memory.to_string()),
                                metadata: MemoryMetadata::default(),
                                created_at: now,
                                updated_at: now,
                            },
                            score,
                            highlights: vec![],
                        })
                    })
                    .collect();

                Ok(results)
            }
            Ok(r) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!(
                    "Mem0 search error: {}",
                    r.status()
                )))
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!("Mem0 search failed: {e}")))
            }
        }
    }

    async fn update(&self, id: &MemoryId, entry: MemoryEntry) -> MemoryResult<()> {
        self.check_circuit()?;
        validate_path_segment(id.as_str())?;

        let body = serde_json::json!({
            "content": entry.content,
            "metadata": entry.metadata,
        });

        let resp = self
            .build_request(
                reqwest::Method::PUT,
                &format!("/v1/memories/{}/", id.as_str()),
            )
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                self.circuit_breaker.record_success();
                Ok(())
            }
            Ok(r) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!(
                    "Mem0 update error: {}",
                    r.status()
                )))
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!("Mem0 update failed: {e}")))
            }
        }
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        self.check_circuit()?;
        validate_path_segment(id.as_str())?;

        let resp = self
            .build_request(
                reqwest::Method::DELETE,
                &format!("/v1/memories/{}/", id.as_str()),
            )
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                self.circuit_breaker.record_success();
                Ok(())
            }
            Ok(r) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!(
                    "Mem0 delete error: {}",
                    r.status()
                )))
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!("Mem0 delete failed: {e}")))
            }
        }
    }

    async fn health_check(&self) -> MemoryResult<ProviderHealth> {
        let start = Instant::now();
        let resp = self
            .build_request(reqwest::Method::GET, "/v1/health/")
            .send()
            .await;

        let latency_ms = start.elapsed().as_millis() as u64;

        match resp {
            Ok(r) if r.status().is_success() => {
                self.circuit_breaker.record_success();
                Ok(ProviderHealth {
                    status: HealthStatus::Healthy,
                    latency_ms,
                    message: None,
                })
            }
            Ok(r) => {
                self.circuit_breaker.record_failure();
                Ok(ProviderHealth {
                    status: HealthStatus::Degraded,
                    latency_ms,
                    message: Some(format!("HTTP {}", r.status())),
                })
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Ok(ProviderHealth {
                    status: HealthStatus::Unavailable,
                    latency_ms,
                    message: Some(e.to_string()),
                })
            }
        }
    }
}
