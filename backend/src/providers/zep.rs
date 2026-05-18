//! Zep Provider - HTTP API integration

use std::time::Instant;

use crate::kernel::error::{MemoryError, MemoryResult};
use crate::kernel::provider::*;
use crate::kernel::types::*;
use crate::providers::circuit_breaker::CircuitBreaker;
use crate::providers::config::ExternalProviderConfig;
use crate::providers::validate_path_segment;

pub struct ZepProvider {
    config: ExternalProviderConfig,
    client: reqwest::Client,
    circuit_breaker: CircuitBreaker,
}

impl ZepProvider {
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
                "Zep provider circuit breaker is open".to_string(),
            ));
        }
        Ok(())
    }

    fn build_request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url(), path);
        let mut req = self.client.request(method, &url);
        if let Some(key) = self.config.resolve_api_key() {
            req = req.header("Authorization", format!("Api-Key {key}"));
        }
        req
    }
}

#[async_trait::async_trait]
impl MemoryProvider for ZepProvider {
    fn provider_name(&self) -> &str {
        "zep"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_vector_search: true,
            supports_graph: true,
            supports_metadata_filter: true,
            supports_eviction: false,
            max_entry_size_bytes: Some(10 * 1024 * 1024),
        }
    }

    async fn store(&self, entry: MemoryEntry) -> MemoryResult<MemoryId> {
        self.check_circuit()?;

        let content_text = match &entry.content {
            MemoryContent::Text(t) => t.clone(),
            MemoryContent::Json(v) => serde_json::to_string(v).unwrap_or_default(),
            _ => String::new(),
        };

        let session_id = entry.metadata.session_id.as_deref().unwrap_or("default");
        validate_path_segment(session_id)?;
        let body = serde_json::json!({
            "messages": [{
                "role_type": "assistant",
                "content": content_text,
                "metadata": entry.metadata.extra,
            }]
        });

        let resp = self
            .build_request(
                reqwest::Method::POST,
                &format!("/api/v2/sessions/{session_id}/messages"),
            )
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                self.circuit_breaker.record_success();
                Ok(entry.id)
            }
            Ok(r) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!(
                    "Zep API error: {}",
                    r.status()
                )))
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!("Zep request failed: {e}")))
            }
        }
    }

    async fn retrieve(&self, id: &MemoryId) -> MemoryResult<MemoryEntry> {
        self.check_circuit()?;
        validate_path_segment(id.as_str())?;

        let resp = self
            .build_request(
                reqwest::Method::GET,
                &format!("/api/v2/sessions/{}/messages", id.as_str()),
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
                let content = data["messages"][0]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let now = chrono::Utc::now().timestamp();
                Ok(MemoryEntry {
                    id: id.clone(),
                    layer: LayerType::Ltm,
                    content: MemoryContent::Text(content),
                    metadata: MemoryMetadata::default(),
                    created_at: now,
                    updated_at: now,
                })
            }
            Ok(r) if r.status().as_u16() == 404 => {
                self.circuit_breaker.record_success();
                Err(MemoryError::NotFound(format!(
                    "Zep memory not found: {}",
                    id.as_str()
                )))
            }
            Ok(r) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!(
                    "Zep API error: {}",
                    r.status()
                )))
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!("Zep request failed: {e}")))
            }
        }
    }

    async fn search(&self, query: &MemoryQuery) -> MemoryResult<Vec<MemoryMatch>> {
        self.check_circuit()?;

        let search_text = query.text.as_deref().unwrap_or("");
        let session_id = query.filters.session_id.as_deref().unwrap_or("default");
        validate_path_segment(session_id)?;
        let body = serde_json::json!({
            "text": search_text,
            "limit": query.limit,
            "search_type": "similarity",
        });

        let resp = self
            .build_request(
                reqwest::Method::POST,
                &format!("/api/v2/sessions/{session_id}/search"),
            )
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
                        let content = item["message"]["content"].as_str()?;
                        let score = item["score"].as_f64().unwrap_or(0.0);
                        let now = chrono::Utc::now().timestamp();
                        Some(MemoryMatch {
                            entry: MemoryEntry {
                                id: MemoryId::new(),
                                layer: LayerType::Ltm,
                                content: MemoryContent::Text(content.to_string()),
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
                    "Zep search error: {}",
                    r.status()
                )))
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!("Zep search failed: {e}")))
            }
        }
    }

    async fn update(&self, _id: &MemoryId, _entry: MemoryEntry) -> MemoryResult<()> {
        Err(MemoryError::InvalidOperation(
            "Zep does not support direct message update; delete and re-store instead".to_string(),
        ))
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        self.check_circuit()?;
        validate_path_segment(id.as_str())?;

        let resp = self
            .build_request(
                reqwest::Method::DELETE,
                &format!("/api/v2/sessions/{}/messages", id.as_str()),
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
                    "Zep delete error: {}",
                    r.status()
                )))
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(MemoryError::Storage(format!("Zep delete failed: {e}")))
            }
        }
    }

    async fn health_check(&self) -> MemoryResult<ProviderHealth> {
        let start = Instant::now();
        let resp = self
            .build_request(reqwest::Method::GET, "/healthz")
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
