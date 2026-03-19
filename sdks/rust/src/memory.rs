//! Memory API client

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{Config, Result};

/// Memory layer types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    /// Short-term memory
    Stm,
    /// Long-term memory
    Ltm,
    /// Knowledge graph
    Kg,
    /// Multimodal memory
    Mm,
}

impl Default for Layer {
    fn default() -> Self {
        Layer::Stm
    }
}

impl std::fmt::Display for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Layer::Stm => write!(f, "stm"),
            Layer::Ltm => write!(f, "ltm"),
            Layer::Kg => write!(f, "kg"),
            Layer::Mm => write!(f, "mm"),
        }
    }
}

/// Write memory request
#[derive(Debug, Serialize)]
struct WriteRequest {
    name: String,
    arguments: WriteArguments,
}

/// Write memory arguments
#[derive(Debug, Serialize)]
struct WriteArguments {
    content: String,
    #[serde(rename = "layer")]
    layer: String,
    #[serde(rename = "user_id")]
    user_id: Option<String>,
    #[serde(rename = "session_id")]
    session_id: Option<String>,
    #[serde(rename = "agent_id")]
    agent_id: Option<String>,
    #[serde(rename = "entity_name")]
    entity_name: Option<String>,
    #[serde(rename = "entity_type")]
    entity_type: Option<String>,
}

/// Write memory response
#[derive(Debug, Deserialize)]
pub struct WriteResponse {
    #[serde(rename = "success")]
    pub success: Option<bool>,
    #[serde(rename = "layer")]
    pub layer: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(rename = "messageId")]
    pub message_id: Option<String>,
    #[serde(rename = "entryId")]
    pub entry_id: Option<String>,
    #[serde(rename = "entityId")]
    pub entity_id: Option<String>,
}

/// Search memory request
#[derive(Debug, Serialize)]
struct SearchRequest {
    name: String,
    arguments: SearchArguments,
}

/// Search arguments
#[derive(Debug, Serialize)]
struct SearchArguments {
    query: String,
    #[serde(rename = "layer")]
    layer: String,
    limit: u32,
    #[serde(rename = "user_id")]
    user_id: Option<String>,
    #[serde(rename = "session_id")]
    session_id: Option<String>,
}

/// Search result item
#[derive(Debug, Deserialize)]
pub struct SearchResult {
    pub id: Option<String>,
    pub score: Option<f64>,
    pub content: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub entity_type: Option<String>,
    pub description: Option<String>,
}

/// List memory request
#[derive(Debug, Serialize)]
struct ListRequest {
    name: String,
    arguments: ListArguments,
}

/// List arguments
#[derive(Debug, Serialize)]
struct ListArguments {
    #[serde(rename = "layer")]
    layer: String,
    limit: u32,
    offset: u32,
    #[serde(rename = "user_id")]
    user_id: Option<String>,
}

/// Session info
#[derive(Debug, Deserialize)]
pub struct Session {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    #[serde(rename = "agentId")]
    pub agent_id: Option<String>,
    #[serde(rename = "sessionType")]
    pub session_type: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
}

/// LTM Entry
#[derive(Debug, Deserialize)]
pub struct LtmEntry {
    pub id: Option<String>,
    pub content: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
}

/// Synchronous Memory Client
pub struct MemoryClient {
    client: Client,
    config: Arc<Config>,
}

impl MemoryClient {
    /// Create a new memory client
    pub fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout))
            .build()?;

        Ok(Self {
            client,
            config: Arc::new(config),
        })
    }

    /// Write memory to specified layer
    pub fn write(
        &self,
        content: impl Into<String>,
        layer: Layer,
    ) -> Result<WriteResponse> {
        let request = WriteRequest {
            name: "memory_write".to_string(),
            arguments: WriteArguments {
                content: content.into(),
                layer: layer.to_string(),
                user_id: None,
                session_id: None,
                agent_id: None,
                entity_name: None,
                entity_type: None,
            },
        };

        let mut req = self.client.post(format!("{}/api/mcp/mcp/tools/call", self.config.base_url));

        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.json(&request).send()?;

        if !response.status().is_success() {
            return Err(crate::Error::Api(format!(
                "{}: {}",
                response.status(),
                response.text().unwrap_or_default()
            )));
        }

        let data: serde_json::Value = response.json()?;
        let content = data.get("content").and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .cloned()
            .unwrap_or_default();

        let text = content.get("text").and_then(|t| t.as_str()).unwrap_or("{}");
        let result: WriteResponse = serde_json::from_str(text)
            .unwrap_or(WriteResponse {
                success: Some(true),
                layer: Some(layer.to_string()),
                session_id: None,
                message_id: None,
                entry_id: None,
                entity_id: None,
            });

        Ok(result)
    }

    /// Search memory in specified layer
    pub fn search(
        &self,
        query: impl Into<String>,
        layer: Layer,
        limit: u32,
    ) -> Result<Vec<SearchResult>> {
        let request = SearchRequest {
            name: "memory_search".to_string(),
            arguments: SearchArguments {
                query: query.into(),
                layer: layer.to_string(),
                limit,
                user_id: None,
                session_id: None,
            },
        };

        let mut req = self.client.post(format!("{}/api/mcp/mcp/tools/call", self.config.base_url));

        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.json(&request).send()?;

        if !response.status().is_success() {
            return Err(crate::Error::Api(format!(
                "{}: {}",
                response.status(),
                response.text().unwrap_or_default()
            )));
        }

        let data: serde_json::Value = response.json()?;
        let content = data.get("content").and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .cloned()
            .unwrap_or_default();

        let text = content.get("text").and_then(|t| t.as_str()).unwrap_or("{}");
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap_or_default();

        let results = parsed.get("results")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }

    /// List memories in specified layer
    pub fn list(&self, layer: Layer, limit: u32, offset: u32) -> Result<serde_json::Value> {
        let request = ListRequest {
            name: "memory_list".to_string(),
            arguments: ListArguments {
                layer: layer.to_string(),
                limit,
                offset,
                user_id: None,
            },
        };

        let mut req = self.client.post(format!("{}/api/mcp/mcp/tools/call", self.config.base_url));

        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.json(&request).send()?;

        if !response.status().is_success() {
            return Err(crate::Error::Api(format!(
                "{}: {}",
                response.status(),
                response.text().unwrap_or_default()
            )));
        }

        let data: serde_json::Value = response.json()?;
        let content = data.get("content").and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .cloned()
            .unwrap_or_default();

        let text = content.get("text").and_then(|t| t.as_str()).unwrap_or("{}");
        let result: serde_json::Value = serde_json::from_str(text).unwrap_or_default();

        Ok(result)
    }
}

/// Asynchronous Memory Client
pub struct AsyncMemoryClient {
    client: reqwest::Client,
    config: Arc<Config>,
}

impl AsyncMemoryClient {
    /// Create a new async memory client
    pub async fn new(config: Config) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout))
            .build()?;

        Ok(Self {
            client,
            config: Arc::new(config),
        })
    }

    /// Write memory (async)
    pub async fn write(
        &self,
        content: impl Into<String>,
        layer: Layer,
    ) -> Result<WriteResponse> {
        let request = WriteRequest {
            name: "memory_write".to_string(),
            arguments: WriteArguments {
                content: content.into(),
                layer: layer.to_string(),
                user_id: None,
                session_id: None,
                agent_id: None,
                entity_name: None,
                entity_type: None,
            },
        };

        let mut req = self.client.post(format!("{}/api/mcp/mcp/tools/call", self.config.base_url));

        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.json(&request).send().await?;

        if !response.status().is_success() {
            return Err(crate::Error::Api(format!(
                "{}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let data: serde_json::Value = response.json().await?;
        let content = data.get("content").and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .cloned()
            .unwrap_or_default();

        let text = content.get("text").and_then(|t| t.as_str()).unwrap_or("{}");
        let result: WriteResponse = serde_json::from_str(text)
            .unwrap_or(WriteResponse {
                success: Some(true),
                layer: Some(layer.to_string()),
                session_id: None,
                message_id: None,
                entry_id: None,
                entity_id: None,
            });

        Ok(result)
    }

    /// Search memory (async)
    pub async fn search(
        &self,
        query: impl Into<String>,
        layer: Layer,
        limit: u32,
    ) -> Result<Vec<SearchResult>> {
        let request = SearchRequest {
            name: "memory_search".to_string(),
            arguments: SearchArguments {
                query: query.into(),
                layer: layer.to_string(),
                limit,
                user_id: None,
                session_id: None,
            },
        };

        let mut req = self.client.post(format!("{}/api/mcp/mcp/tools/call", self.config.base_url));

        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.json(&request).send().await?;

        if !response.status().is_success() {
            return Err(crate::Error::Api(format!(
                "{}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let data: serde_json::Value = response.json().await?;
        let content = data.get("content").and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .cloned()
            .unwrap_or_default();

        let text = content.get("text").and_then(|t| t.as_str()).unwrap_or("{}");
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap_or_default();

        let results = parsed.get("results")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_display() {
        assert_eq!(Layer::Stm.to_string(), "stm");
        assert_eq!(Layer::Ltm.to_string(), "ltm");
        assert_eq!(Layer::Kg.to_string(), "kg");
        assert_eq!(Layer::Mm.to_string(), "mm");
    }
}
