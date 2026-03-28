//! Validated Request Types for High-Risk Endpoints
//!
//! Provides schema-based validation wrappers for external input per D-05.
//! These types validate and sanitize input before reaching business logic.

use serde::{Deserialize, Serialize};

use crate::hoops::validation::{contains_sql_injection, contains_xss, validate_content_length, ValidationError};

/// MCP tool call params (mirrors routers::mcp::ToolCallParams for validation)
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
}

/// Maximum length for tool names
const MAX_TOOL_NAME_LENGTH: usize = 100;

/// Maximum content length for memory writes (1MB)
const MAX_MEMORY_CONTENT_LENGTH: usize = 1024 * 1024;

/// Validated MCP tool call request
///
/// Validates:
/// - name: max 100 chars, alphanumeric + underscore only
/// - SQL injection detection in name field
/// - arguments: passed through for tool-specific validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

impl ValidatedToolCall {
    /// Create a validated tool call from raw params
    pub fn from_raw(params: ToolCallParams) -> Result<Self, ValidationError> {
        let name = params.name.trim().to_string();

        // Validate name is not empty
        if name.is_empty() {
            return Err(ValidationError::MissingField("name".into()));
        }

        // Validate name length
        if name.len() > MAX_TOOL_NAME_LENGTH {
            return Err(ValidationError::ExceedsMaxLength(
                "name".into(),
                MAX_TOOL_NAME_LENGTH,
            ));
        }

        // Validate name contains only allowed characters
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(ValidationError::DisallowedCharacters(
                "name".into(),
                "only alphanumeric and underscore allowed".into(),
            ));
        }

        // SQL injection detection in name (defense in depth)
        if contains_sql_injection(&name) {
            return Err(ValidationError::SqlInjectionAttempt("name".into()));
        }

        Ok(Self {
            name,
            arguments: params.arguments.unwrap_or(serde_json::Value::Null),
        })
    }
}

/// Validated memory write request
///
/// Validates:
/// - content: max 1MB, XSS check
/// - layer: must be stm, ltm, kg, or mm
/// - user_id, session_id: optional strings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedMemoryWrite {
    pub content: String,
    pub layer: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
}

impl ValidatedMemoryWrite {
    /// Create a validated memory write from JSON
    pub fn from_json(value: serde_json::Value) -> Result<Self, ValidationError> {
        // Extract and validate content
        let content = value["content"]
            .as_str()
            .ok_or_else(|| ValidationError::MissingField("content".into()))?
            .to_string();

        // Validate content length (max 1MB)
        validate_content_length(&content, MAX_MEMORY_CONTENT_LENGTH)?;

        // Check for XSS in content
        if contains_xss(&content) {
            return Err(ValidationError::XssAttempt("content".into()));
        }

        // Extract and validate layer
        let layer = value["layer"]
            .as_str()
            .ok_or_else(|| ValidationError::MissingField("layer".into()))?
            .to_lowercase();

        if !["stm", "ltm", "kg", "mm"].contains(&layer.as_str()) {
            return Err(ValidationError::InvalidFormat(
                "layer".into(),
                "must be stm, ltm, kg, or mm".into(),
            ));
        }

        // Extract optional fields
        let user_id = value["user_id"].as_str().map(String::from);
        let session_id = value["session_id"].as_str().map(String::from);

        Ok(Self {
            content,
            layer,
            user_id,
            session_id,
        })
    }
}

/// Search query validated request
///
/// Validates:
/// - query: max 1000 chars
/// - XSS check on query string
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedSearchQuery {
    pub query: String,
    pub layer: Option<String>,
    pub limit: Option<i32>,
}

impl ValidatedSearchQuery {
    /// Create a validated search query from JSON
    pub fn from_json(value: serde_json::Value) -> Result<Self, ValidationError> {
        let query = value["query"]
            .as_str()
            .ok_or_else(|| ValidationError::MissingField("query".into()))?
            .to_string();

        // Validate query length (max 1000 chars)
        if query.len() > 1000 {
            return Err(ValidationError::ExceedsMaxLength("query".into(), 1000));
        }

        // Check for XSS in query
        if contains_xss(&query) {
            return Err(ValidationError::XssAttempt("query".into()));
        }

        let layer = value["layer"].as_str().map(String::from);
        let limit = value["limit"].as_i64().map(|v| v as i32);

        Ok(Self {
            query,
            layer,
            limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_tool_call_passes() {
        let result = ValidatedToolCall::from_raw(ToolCallParams {
            name: "memory_write".into(),
            arguments: Some(serde_json::json!({"content": "hello", "layer": "stm"})),
        });
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.name, "memory_write");
    }

    #[test]
    fn test_tool_call_rejects_empty_name() {
        let result = ValidatedToolCall::from_raw(ToolCallParams {
            name: "".into(),
            arguments: None,
        });
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::MissingField(_)));
    }

    #[test]
    fn test_tool_call_rejects_sql_injection_in_name() {
        // Use pattern that passes character whitelist but contains SQL injection keyword
        let result = ValidatedToolCall::from_raw(ToolCallParams {
            name: "DROPTABLE".into(),
            arguments: None,
        });
        assert!(result.is_err());
        // Pattern passes character whitelist (all alphanumeric) but contains SQL keyword
        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::SqlInjectionAttempt(_)));
    }

    #[test]
    fn test_tool_call_rejects_invalid_characters() {
        let result = ValidatedToolCall::from_raw(ToolCallParams {
            name: "my-tool".into(),
            arguments: None,
        });
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::DisallowedCharacters(..)));
    }

    #[test]
    fn test_tool_call_rejects_long_name() {
        let long_name = "a".repeat(101);
        let result = ValidatedToolCall::from_raw(ToolCallParams {
            name: long_name,
            arguments: None,
        });
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::ExceedsMaxLength(..)));
    }

    #[test]
    fn test_memory_write_valid() {
        let json = serde_json::json!({
            "content": "Hello, World!",
            "layer": "stm",
            "user_id": "user1",
            "session_id": "session1"
        });
        let result = ValidatedMemoryWrite::from_json(json);
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.content, "Hello, World!");
        assert_eq!(validated.layer, "stm");
    }

    #[test]
    fn test_memory_write_rejects_xss() {
        let json = serde_json::json!({
            "content": "<script>alert('xss')</script>",
            "layer": "stm"
        });
        let result = ValidatedMemoryWrite::from_json(json);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::XssAttempt(_)));
    }

    #[test]
    fn test_memory_write_rejects_invalid_layer() {
        let json = serde_json::json!({
            "content": "hello",
            "layer": "invalid"
        });
        let result = ValidatedMemoryWrite::from_json(json);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::InvalidFormat(..)));
    }

    #[test]
    fn test_memory_write_rejects_missing_content() {
        let json = serde_json::json!({
            "layer": "stm"
        });
        let result = ValidatedMemoryWrite::from_json(json);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::MissingField(..)));
    }

    #[test]
    fn test_search_query_valid() {
        let json = serde_json::json!({
            "query": "search term",
            "layer": "ltm",
            "limit": 10
        });
        let result = ValidatedSearchQuery::from_json(json);
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.query, "search term");
        assert_eq!(validated.layer, Some("ltm".to_string()));
        assert_eq!(validated.limit, Some(10));
    }

    #[test]
    fn test_search_query_rejects_xss() {
        let json = serde_json::json!({
            "query": "<script>alert(1)</script>"
        });
        let result = ValidatedSearchQuery::from_json(json);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::XssAttempt(_)));
    }

    #[test]
    fn test_search_query_rejects_long_query() {
        let json = serde_json::json!({
            "query": "a".repeat(1001)
        });
        let result = ValidatedSearchQuery::from_json(json);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::ExceedsMaxLength(..)));
    }
}
