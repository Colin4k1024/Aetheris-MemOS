//! Agent API client

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{Config, Result};

/// Agent information
#[derive(Debug, Deserialize)]
pub struct Agent {
    pub agent_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}

/// Agent list response
#[derive(Debug, Deserialize)]
pub struct AgentListResponse {
    pub agents: Vec<Agent>,
    pub total: Option<i64>,
}

/// Create agent request
#[derive(Debug, Serialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub description: Option<String>,
}

/// Agent Client
pub struct AgentClient {
    client: Client,
    config: Arc<Config>,
}

impl AgentClient {
    /// Create a new agent client
    pub fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout))
            .build()?;

        Ok(Self {
            client,
            config: Arc::new(config),
        })
    }

    /// List all agents
    pub fn list(&self, limit: i64, offset: i64) -> Result<AgentListResponse> {
        let mut req = self.client.get(format!("{}/api/v1/agents", self.config.base_url))
            .query(&[("limit", limit.to_string()), ("offset", offset.to_string())]);

        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.send()?;

        if !response.status().is_success() {
            return Err(crate::Error::Api(format!(
                "{}: {}",
                response.status(),
                response.text().unwrap_or_default()
            )));
        }

        let result: AgentListResponse = response.json()?;
        Ok(result)
    }

    /// Get agent by ID
    pub fn get(&self, agent_id: &str) -> Result<Agent> {
        let mut req = self.client.get(format!("{}/api/v1/agents/{}", self.config.base_url, agent_id));

        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.send()?;

        if !response.status().is_success() {
            return Err(crate::Error::Api(format!(
                "{}: {}",
                response.status(),
                response.text().unwrap_or_default()
            )));
        }

        let result: Agent = response.json()?;
        Ok(result)
    }

    /// Create a new agent
    pub fn create(&self, request: CreateAgentRequest) -> Result<Agent> {
        let mut req = self.client.post(format!("{}/api/v1/agents", self.config.base_url));

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

        let result: Agent = response.json()?;
        Ok(result)
    }
}
