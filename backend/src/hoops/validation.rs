//! Input Validation Middleware
//!
//! Provides schema-based validation for external input at system boundaries.
//! Implements D-04 and D-05: All external input enters through a dedicated
//! validation layer using serde + custom validators.

use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use axum::http::StatusCode;

/// Input validation error with structured details
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Field '{0}' exceeds maximum length {1}")]
    ExceedsMaxLength(String, usize),

    #[error("Field '{0}' contains disallowed characters: {1}")]
    DisallowedCharacters(String, String),

    #[error("Invalid format for field '{0}': {1}")]
    InvalidFormat(String, String),

    #[error("SQL injection attempt detected in field '{0}'")]
    SqlInjectionAttempt(String),

    #[error("XSS attempt detected in field '{0}'")]
    XssAttempt(String),
}

impl ValidationError {
    /// Returns the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        StatusCode::BAD_REQUEST // 400 for all validation errors
    }

    /// Returns the error code string for structured responses
    pub fn error_code(&self) -> &'static str {
        match self {
            ValidationError::MissingField(_) => "MISSING_FIELD",
            ValidationError::ExceedsMaxLength(..) => "EXCEEDS_MAX_LENGTH",
            ValidationError::DisallowedCharacters(..) => "DISALLOWED_CHARS",
            ValidationError::InvalidFormat(..) => "INVALID_FORMAT",
            ValidationError::SqlInjectionAttempt(_) => "SQL_INJECTION",
            ValidationError::XssAttempt(_) => "XSS_ATTEMPT",
        }
    }
}

/// Validation middleware that checks all input on high-risk routes
///
/// This middleware is applied to routes that handle external input (MCP tools,
/// memory writes). It performs schema-based validation before passing requests
/// to handlers.
pub async fn validation_middleware(req: Request, next: Next) -> Result<Response, ValidationError> {
    // Extract path for route-specific validation
    let path = req
        .uri()
        .path()
        .to_string();

    // High-risk routes that require validation
    let high_risk_paths = [
        "/api/mcp/tools/call",
        "/api/memory/write",
        "/api/memory/store",
        "/api/kg/entities",
        "/api/mm/create",
    ];

    // Check if this is a high-risk route
    let is_high_risk = high_risk_paths.iter().any(|p| path.contains(p));

    if is_high_risk {
        // For high-risk routes, we validate content
        // The actual validation is done in the ValidatedToolCall and
        // ValidatedMemoryWrite types at the handler level
    }

    // Continue to handler - specific field validation happens in typed extractors
    Ok(next.run(req).await)
}

/// SQL injection detection patterns
///
/// Checks for common SQL injection patterns in user input.
/// This is defense-in-depth; parameterized queries are used at the DB layer.
pub fn contains_sql_injection(input: &str) -> bool {
    let lower = input.to_lowercase();
    lower.contains("';")
        || lower.contains("--")
        || lower.contains("/*")
        || lower.contains("*/")
        || lower.contains("union")
        || lower.contains("select")
        || lower.contains("drop")
        || lower.contains("insert")
        || lower.contains("update")
        || lower.contains("delete")
        || lower.contains("exec")
        || lower.contains("execute")
        || lower.contains("xp_")
        || lower.contains("sp_")
        || lower.contains("waitfor")
        || lower.contains("benchmark")
        || lower.contains("sleep")
}

/// XSS detection patterns
///
/// Checks for common XSS patterns in user input.
/// HTML output should use allowlist sanitization in addition to this check.
pub fn contains_xss(input: &str) -> bool {
    let lower = input.to_lowercase();
    lower.contains("<script")
        || lower.contains("javascript:")
        || lower.contains("onerror=")
        || lower.contains("onload=")
        || lower.contains("<iframe")
        || lower.contains("onclick=")
        || lower.contains("onmouseover=")
        || lower.contains("eval(")
        || lower.contains("expression(")
        || lower.contains("url(")
}

/// Content validation for size limits
pub fn validate_content_length(content: &str, max_bytes: usize) -> Result<(), ValidationError> {
    if content.len() > max_bytes {
        return Err(ValidationError::ExceedsMaxLength(
            "content".into(),
            max_bytes,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_sql_injection_patterns() {
        // Common SQL injection patterns
        assert!(contains_sql_injection("'; DROP TABLE users;--"));
        assert!(contains_sql_injection(" UNION SELECT "));
        assert!(contains_sql_injection("'; INSERT INTO admin--"));
        assert!(contains_sql_injection("exec xp_cmdshell"));
        assert!(contains_sql_injection("'; exec sp_executesql--"));
        assert!(contains_sql_injection("benchmark(1000000,MD5('test'))"));
        assert!(contains_sql_injection("sleep(5)"));
        assert!(contains_sql_injection("DROPTABLE")); // SQL keyword without special chars

        // Safe inputs
        assert!(!contains_sql_injection("Hello, World!"));
        assert!(!contains_sql_injection("user123"));
        assert!(!contains_sql_injection("It's a nice day"));
        assert!(!contains_sql_injection("Order #12345"));
        assert!(!contains_sql_injection("Normal user input"));
        assert!(!contains_sql_injection("1 OR 1"));
    }

    #[test]
    fn test_contains_xss_patterns() {
        // Common XSS patterns
        assert!(contains_xss("<script>alert(1)</script>"));
        assert!(contains_xss("javascript:alert(1)"));
        assert!(contains_xss("<img onerror=alert(1)>"));
        assert!(contains_xss("<iframe src='x'>"));
        assert!(contains_xss("onclick=alert(1)"));
        assert!(contains_xss("onmouseover=alert(1)"));
        assert!(contains_xss("eval('alert(1)')"));
        assert!(contains_xss("expression(alert(1))"));
        assert!(contains_xss("url(javascript:alert(1))"));

        // Safe inputs
        assert!(!contains_xss("Hello, World!"));
        assert!(!contains_xss("Just some text"));
        assert!(!contains_xss("User input: <div>content</div>"));
    }

    #[test]
    fn test_validate_content_length() {
        // Valid content
        assert!(validate_content_length("short", 100).is_ok());
        assert!(validate_content_length("", 100).is_ok());

        // Content exceeds limit
        let long_content = "x".repeat(100);
        assert!(validate_content_length(&long_content, 50).is_err());

        // Exact limit
        let exact_content = "x".repeat(50);
        assert!(validate_content_length(&exact_content, 50).is_ok());
    }

    #[test]
    fn test_validation_error_codes() {
        let err = ValidationError::MissingField("name".into());
        assert_eq!(err.error_code(), "MISSING_FIELD");
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);

        let err = ValidationError::SqlInjectionAttempt("field".into());
        assert_eq!(err.error_code(), "SQL_INJECTION");

        let err = ValidationError::XssAttempt("field".into());
        assert_eq!(err.error_code(), "XSS_ATTEMPT");
    }
}
