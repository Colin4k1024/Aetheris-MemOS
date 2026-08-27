//! HTTP Client for Adaptive Memory System

use serde::{de::DeserializeOwned, Serialize};

use crate::models::*;

/// Adaptive Memory client error
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("API error: {status} - {message}")]
    Api {
        status: reqwest::StatusCode,
        message: String,
    },
}

/// Adaptive Memory client
pub struct Client {
    base_url: String,
    http: reqwest::Client,
    api_key: Option<String>,
}

impl Client {
    /// Create a new client
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http: reqwest::Client::new(),
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
        format!(
            "{}/api/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    /// Build a request (without sending it), applying auth, optional query
    /// parameters, and an optional JSON body.
    ///
    /// Extracted as a separate seam so URL/query construction can be
    /// unit-tested without a live server (see the tests below).
    fn build_request<B, Q>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<B>,
        query: Option<&Q>,
    ) -> reqwest::RequestBuilder
    where
        B: Serialize,
        Q: Serialize + ?Sized,
    {
        let url = self.build_url(path);
        let mut request = self.http.request(method, url);

        if let Some(ref key) = self.api_key {
            request = request.bearer_auth(key);
        }

        if let Some(query) = query {
            request = request.query(query);
        }

        if let Some(body) = body {
            request = request.json(&body);
        }

        request
    }

    /// Send a prepared request and deserialize the JSON response.
    async fn send_and_parse<T: DeserializeOwned>(
        request: reqwest::RequestBuilder,
    ) -> Result<T, Error> {
        let response = request.send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let status = response.status();
            let message = response.text().await.unwrap_or_default();
            Err(Error::Api { status, message })
        }
    }

    /// Make a request
    async fn request<T: DeserializeOwned, B: Serialize>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<B>,
    ) -> Result<T, Error> {
        Self::send_and_parse(self.build_request(method, path, body, None::<&()>)).await
    }

    // === Storage ===

    /// Store content in STM
    pub async fn store_stm(&self, req: StoreStmRequest) -> Result<StoreStmResponse, Error> {
        self.request(reqwest::Method::POST, "v1/memory/storage/stm", Some(req))
            .await
    }

    /// Store content in LTM
    pub async fn store_ltm(&self, req: StoreLtmRequest) -> Result<StoreLtmResponse, Error> {
        self.request(reqwest::Method::POST, "v1/memory/storage/ltm", Some(req))
            .await
    }

    // === Memory Governance (#130) ===
    //
    // Thin wrappers over /api/v1/governance — request/response shapes are the
    // server's serde types (documented in the OpenAPI spec at /api-doc/openapi.json).

    /// List beliefs. Non-admin callers are pinned server-side to their own subject.
    pub async fn governance_list_beliefs(
        &self,
        subject: Option<&str>,
        predicate: Option<&str>,
        include_history: bool,
    ) -> Result<serde_json::Value, Error> {
        let query: Vec<(&str, String)> = [
            subject.map(|s| ("subject", s.to_string())),
            predicate.map(|p| ("predicate", p.to_string())),
            include_history.then(|| ("include_history", "true".to_string())),
        ]
        .into_iter()
        .flatten()
        .collect();
        Self::send_and_parse(self.build_request(
            reqwest::Method::GET,
            "v1/governance/beliefs",
            None::<&()>,
            Some(&query),
        ))
        .await
    }

    /// Full traceability for one belief: edge + provenance evidence + audit chain.
    pub async fn governance_belief_trace(&self, belief_id: &str) -> Result<serde_json::Value, Error> {
        Self::send_and_parse(self.build_request::<serde_json::Value, &()>(
            reqwest::Method::GET,
            &format!("v1/governance/beliefs/{belief_id}/trace"),
            None,
            None,
        ))
        .await
    }

    /// Roll the current edge back to its predecessor.
    pub async fn governance_rollback(&self, belief_id: &str) -> Result<serde_json::Value, Error> {
        Self::send_and_parse(self.build_request::<serde_json::Value, &()>(
            reqwest::Method::POST,
            &format!("v1/governance/beliefs/{belief_id}/rollback"),
            None,
            None,
        ))
        .await
    }

    /// The confirmation / quarantine queues (admin-only).
    pub async fn governance_candidates(&self, status: &str) -> Result<serde_json::Value, Error> {
        let query = vec![("status", status.to_string())];
        Self::send_and_parse(self.build_request(
            reqwest::Method::GET,
            "v1/governance/candidates",
            None::<&()>,
            Some(&query),
        ))
        .await
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

    /// Build the `list_sessions` request (without sending it), so query
    /// construction is unit-testable and parameters can't be silently dropped.
    fn list_sessions_request(
        &self,
        user_id: Option<&str>,
        limit: Option<usize>,
    ) -> reqwest::RequestBuilder {
        #[derive(Serialize)]
        struct ListSessionsQuery<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            user_id: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            limit: Option<usize>,
        }

        self.build_request(
            reqwest::Method::GET,
            "v1/memory/storage/sessions",
            None::<&()>,
            Some(&ListSessionsQuery { user_id, limit }),
        )
    }

    /// List sessions
    pub async fn list_sessions(
        &self,
        user_id: Option<&str>,
        limit: Option<usize>,
    ) -> Result<SessionListResponse, Error> {
        Self::send_and_parse(self.list_sessions_request(user_id, limit)).await
    }

    // === MCP ===

    /// Initialize MCP
    pub async fn initialize_mcp(&self) -> Result<serde_json::Value, Error> {
        self.request(reqwest::Method::POST, "initialize", None::<&()>)
            .await
    }

    /// List MCP tools
    pub async fn list_mcp_tools(&self) -> Result<serde_json::Value, Error> {
        self.request(reqwest::Method::GET, "mcp/tools", None::<&()>)
            .await
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
            Some(SelectRequest { task_description }),
        )
        .await
    }

    // === Health ===

    /// Health check
    pub async fn health_check(&self) -> Result<serde_json::Value, Error> {
        self.request(reqwest::Method::GET, "v1/memory/health", None::<&()>)
            .await
    }
}

/// Asynchronous client (thin wrapper around Client for API compatibility)
pub struct AsyncClient {
    inner: Client,
}

impl AsyncClient {
    /// Create a new async client
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            inner: Client::new(base_url),
        }
    }

    /// Set API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.inner = self.inner.with_api_key(api_key);
        self
    }

    /// Store content in STM
    pub async fn store_stm(&self, req: StoreStmRequest) -> Result<StoreStmResponse, Error> {
        self.inner.store_stm(req).await
    }

    /// Store content in LTM
    pub async fn store_ltm(&self, req: StoreLtmRequest) -> Result<StoreLtmResponse, Error> {
        self.inner.store_ltm(req).await
    }

    /// Search in LTM
    pub async fn search_ltm(
        &self,
        query: &str,
        user_id: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<SearchResult>, Error> {
        self.inner.search_ltm(query, user_id, limit).await
    }

    /// Health check
    pub async fn health_check(&self) -> Result<serde_json::Value, Error> {
        self.inner.health_check().await
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

    /// Build a `list_sessions` request and return its final URL string.
    fn list_sessions_url(user_id: Option<&str>, limit: Option<usize>) -> String {
        Client::new("http://localhost:8008")
            .list_sessions_request(user_id, limit)
            .build()
            .expect("request should build")
            .url()
            .as_str()
            .to_string()
    }

    #[test]
    fn list_sessions_sends_all_params() {
        assert_eq!(
            list_sessions_url(Some("alice"), Some(10)),
            "http://localhost:8008/api/v1/memory/storage/sessions?user_id=alice&limit=10"
        );
    }

    #[test]
    fn list_sessions_sends_only_user_id() {
        assert_eq!(
            list_sessions_url(Some("alice"), None),
            "http://localhost:8008/api/v1/memory/storage/sessions?user_id=alice"
        );
    }

    #[test]
    fn list_sessions_sends_only_limit() {
        assert_eq!(
            list_sessions_url(None, Some(10)),
            "http://localhost:8008/api/v1/memory/storage/sessions?limit=10"
        );
    }

    #[test]
    fn list_sessions_omits_absent_params() {
        // Both params absent → no query string at all (no dangling '?').
        assert_eq!(
            list_sessions_url(None, None),
            "http://localhost:8008/api/v1/memory/storage/sessions"
        );
    }

    #[test]
    fn list_sessions_encodes_special_characters() {
        // form-urlencoded (reqwest `.query()`) encodes spaces as '+', which every
        // standard query-string parser (including the Axum backend) decodes back
        // to a space. This differs from percent-encoding ('%20') but is equivalent
        // on decode.
        assert_eq!(
            list_sessions_url(Some("alice smith"), None),
            "http://localhost:8008/api/v1/memory/storage/sessions?user_id=alice+smith"
        );
    }
}
