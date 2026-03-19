//! MCP Protocol API client

use reqwest::blocking::Client;
use serde::Deserialize;
use std::sync::Arc;

use crate::{Config, Result};

/// MCP Tool definition
#[derive(Debug, Deserialize)]
pub struct Tool {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Option<serde_json::Value>,
}

/// Tools list response
#[derive(Debug, Deserialize)]
pub struct ToolsListResponse {
    pub tools: Vec<Tool>,
}

/// MCP Resource definition
#[derive(Debug, Deserialize)]
pub struct Resource {
    pub uri: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

/// Resources list response
#[derive(Debug, Deserialize)]
pub struct ResourcesListResponse {
    pub resources: Vec<Resource>,
}

/// MCP Initialize response
#[derive(Debug, Deserialize)]
pub struct InitializeResponse {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: Option<String>,
    pub capabilities: Option<serde_json::Value>,
    #[serde(rename = "serverInfo")]
    pub server_info: Option<ServerInfo>,
}

/// Server info
#[derive(Debug, Deserialize)]
pub struct ServerInfo {
    pub name: Option<String>,
    pub version: Option<String>,
}

/// MCP Client
pub struct McpClient {
    client: Client,
    config: Arc<Config>,
}

impl McpClient {
    /// Create a new MCP client
    pub fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout))
            .build()?;

        Ok(Self {
            client,
            config: Arc::new(config),
        })
    }

    /// Initialize MCP session
    pub fn initialize(&self) -> Result<InitializeResponse> {
        let mut req = self.client.post(format!("{}/api/mcp/initialize", self.config.base_url));

        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.json(&serde_json::json!({})).send()?;

        if !response.status().is_success() {
            return Err(crate::Error::Api(format!(
                "{}: {}",
                response.status(),
                response.text().unwrap_or_default()
            )));
        }

        let result: InitializeResponse = response.json()?;
        Ok(result)
    }

    /// List available tools
    pub fn list_tools(&self) -> Result<ToolsListResponse> {
        let mut req = self.client.get(format!("{}/api/mcp/mcp/tools", self.config.base_url));

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

        let result: ToolsListResponse = response.json()?;
        Ok(result)
    }

    /// List available resources
    pub fn list_resources(&self) -> Result<ResourcesListResponse> {
        let mut req = self.client.get(format!("{}/api/mcp/mcp/resources", self.config.base_url));

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

        let result: ResourcesListResponse = response.json()?;
        Ok(result)
    }
}
