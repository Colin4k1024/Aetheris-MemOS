//! Knowledge Graph API client

use reqwest::blocking::Client;
use serde::Deserialize;
use std::sync::Arc;

use crate::{Config, Result};

/// Knowledge Graph Entity
#[derive(Debug, Deserialize)]
pub struct Entity {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub entity_type: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "confidenceScore")]
    pub confidence_score: Option<f64>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
}

/// Entity list response
#[derive(Debug, Deserialize)]
pub struct EntityListResponse {
    pub entities: Vec<Entity>,
    pub count: Option<usize>,
    #[serde(rename = "entityType")]
    pub entity_type: Option<String>,
}

/// Knowledge Graph Client
pub struct KnowledgeGraphClient {
    client: Client,
    config: Arc<Config>,
}

impl KnowledgeGraphClient {
    /// Create a new knowledge graph client
    pub fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout))
            .build()?;

        Ok(Self {
            client,
            config: Arc::new(config),
        })
    }

    /// Create entity
    pub fn create_entity(
        &self,
        name: String,
        entity_type: String,
        description: Option<String>,
    ) -> Result<serde_json::Value> {
        let payload = serde_json::json!({
            "name": "memory_write",
            "arguments": {
                "content": description.unwrap_or_default(),
                "layer": "kg",
                "entity_name": name,
                "entity_type": entity_type,
            }
        });

        let mut req = self.client.post(format!("{}/api/mcp/mcp/tools/call", self.config.base_url));

        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.json(&payload).send()?;

        if !response.status().is_success() {
            return Err(crate::Error::Api(format!(
                "{}: {}",
                response.status(),
                response.text().unwrap_or_default()
            )));
        }

        let data: serde_json::Value = response.json()?;
        Ok(data)
    }

    /// List entities
    pub fn list_entities(&self, entity_type: Option<&str>, limit: u32) -> Result<EntityListResponse> {
        let payload = serde_json::json!({
            "name": "memory_list",
            "arguments": {
                "layer": "kg",
                "limit": limit,
            }
        });

        let mut req = self.client.post(format!("{}/api/mcp/mcp/tools/call", self.config.base_url));

        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.json(&payload).send()?;

        if !response.status().is_success() {
            return Err(crate::Error::Api(format!(
                "{}: {}",
                response.status(),
                response.text().unwrap_or_default()
            )));
        }

        let data: serde_json::Value = response.json()?;
        let content = data.get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .cloned()
            .unwrap_or_default();

        let text = content.get("text").and_then(|t| t.as_str()).unwrap_or("{}");
        let result: EntityListResponse = serde_json::from_str(text).unwrap_or(EntityListResponse {
            entities: vec![],
            count: Some(0),
            entity_type: entity_type.map(String::from),
        });

        Ok(result)
    }
}
