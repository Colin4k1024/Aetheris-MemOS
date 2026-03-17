//! HTTP Client for Adaptive Memory System

use reqwest::Client;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

use crate::models::*;

/// Adaptive Memory client error
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("API error: {status} - {message}")]
    Api { status: reqwest::StatusCode, message: String },
}

/// Adaptive Memory client
pub struct Client {
    base_url: String,
    client: Client,
    api_key: Option<String>,
}

impl Client {
    /// Create a new client
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: Client::new(),
            api_key: None,
        }
    }

    /// Set API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Build URL from path
    fn build_url(&self, path: &str) -> String {
        format!("{}/api/{}", self.base_url.trim_end_matches('/'), path.trim_start_matches('/'))
    }

    /// Make a request
    async fn request<T: DeserializeOwned, B: Serialize>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<B>,
    ) -> Result<T, Error> {
        let url = self.build_url(path);
        let mut request = self.client.request(method, url);

        if let Some(ref key) = self.api_key {
            request = request.bearer_auth(key);
        }

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let status = response.status();
            let message = response.text().await.unwrap_or_default();
            Err(Error::Api { status, message })
        }
    }

    // === Storage ===

    /// Store content in STM
    pub async fn store_stm(&self, req: StoreStmRequest) -> Result<StoreStmResponse, Error> {
        self.request(reqwest::Method::POST, "v1/memory/storage/stm", Some(req)).await
    }

    /// Store content in LTM
    pub async fn store_ltm(&self, req: StoreLtmRequest) -> Result<StoreLtmResponse, Error> {
        self.request(reqwest::Method::POST, "v1/memory/storage/ltm", Some(req)).await
    }

    // === Search ===

    /// Search in LTM
    pub async fn search_ltm(
        &self,
        query: &str,
        user_id: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<SearchResult>, Error> {
        #[derive(Serialize)]
        struct SearchRequest<'a> {
            query: &'a str,
            user_id: Option<&'a str>,
            limit: Option<usize>,
        }

        self.request(
            reqwest::Method::POST,
            "v1/memory/search/ltm",
            Some(SearchRequest {
                query,
                user_id,
                limit,
            }),
        )
        .await
    }

    /// List sessions
    pub async fn list_sessions(
        &self,
        user_id: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<Session>, Error> {
        self.request(
            reqwest::Method::GET,
            "v1/memory/storage/sessions",
            None::<&()>,
        )
        .await
    }

    // === MCP ===

    /// Initialize MCP
    pub async fn initialize_mcp(&self) -> Result<serde_json::Value, Error> {
        self.request(reqwest::Method::POST, "mcp/initialize", None::<&()>).await
    }

    /// List MCP tools
    pub async fn list_mcp_tools(&self) -> Result<serde_json::Value, Error> {
        self.request(reqwest::Method::GET, "mcp/tools", None::<&()>).await
    }

    /// Call MCP tool
    pub async fn call_mcp_tool(
        &self,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Error> {
        #[derive(Serialize)]
        struct ToolCall {
            name: String,
            arguments: Option<serde_json::Value>,
        }

        self.request(
            reqwest::Method::POST,
            "mcp/tools/call",
            Some(ToolCall {
                name: name.to_string(),
                arguments,
            }),
        )
        .await
    }

    // === Adaptive ===

    /// Select memory configuration
    pub async fn select_memory_config(
        &self,
        task_description: &str,
    ) -> Result<serde_json::Value, Error> {
        #[derive(Serialize)]
        struct SelectRequest<'a> {
            task_description: &'a str,
        }

        self.request(
            reqwest::Method::POST,
            "v1/memory/adaptive/select",
            Some(SelectRequest {
                task_description,
            }),
        )
        .await
    }

    // === Health ===

    /// Health check
    pub async fn health_check(&self) -> Result<serde_json::Value, Error> {
        self.request(reqwest::Method::GET, "v1/memory/health", None::<&()>).await
    }
}

/// Asynchronous client
pub struct AsyncClient {
    base_url: String,
    client: Client,
}

impl AsyncClient {
    /// Create a new async client
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: Client::new(base_url),
        }
    }

    /// Set API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.client = self.client.with_api_key(api_key);
        self
    }

    /// Store content in STM
    pub async fn store_stm(&self, req: StoreStmRequest) -> Result<StoreStmResponse, Error> {
        self.client.store_stm(req).await
    }

    /// Store content in LTM
    pub async fn store_ltm(&self, req: StoreLtmRequest) -> Result<StoreLtmResponse, Error> {
        self.client.store_ltm(req).await
    }

    /// Search in LTM
    pub async fn search_ltm(
        &self,
        query: &str,
        user_id: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<SearchResult>, Error> {
        self.client.search_ltm(query, user_id, limit).await
    }

    /// Health check
    pub async fn health_check(&self) -> Result<serde_json::Value, Error> {
        self.client.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = Client::new("http://localhost:8008");
        assert_eq!(client.base_url, "http://localhost:8008");
    }

    #[test]
    fn test_url_building() {
        let client = Client::new("http://localhost:8008");
        assert_eq!(
            client.build_url("v1/memory/storage/stm"),
            "http://localhost:8008/api/v1/memory/storage/stm"
        );
    }
}
