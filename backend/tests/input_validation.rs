//! Input Validation Tests for Injection Attacks
//!
//! Integration tests for SQL injection, XSS, and other input validation scenarios.
//! These tests verify that the validation layer properly rejects malicious input.

use axum::http::StatusCode;
use backend::hoops::validation::{
    contains_sql_injection, contains_xss, validate_content_length, ValidationError,
};
use backend::models::validation::{
    ToolCallParams, ValidatedMemoryWrite, ValidatedSearchQuery, ValidatedToolCall,
};

/// Test SQL injection detection in various patterns
#[test]
fn test_sql_injection_detected_semicolon_terminated() {
    // Classic SQL injection with semicolon terminator
    let result = ValidatedToolCall::from_raw(ToolCallParams {
        name: "'; DROP TABLE users;--".into(),
        arguments: None,
    });
    // Should be rejected - either as disallowed characters or SQL injection
    assert!(result.is_err());
    let err = result.unwrap_err();
    // The semicolon and spaces make it fail character validation first
    assert!(matches!(err, ValidationError::DisallowedCharacters(..)));
}

/// Test SQL injection with SQL keyword
#[test]
fn test_sql_injection_keyword_detected() {
    // Tool name that passes character whitelist but contains SQL keyword
    let result = ValidatedToolCall::from_raw(ToolCallParams {
        name: "DROPTABLE".into(),
        arguments: None,
    });
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::SqlInjectionAttempt(..)
    ));
}

/// Test SQL injection with UNION SELECT
#[test]
fn test_sql_injection_union_select() {
    let result = ValidatedToolCall::from_raw(ToolCallParams {
        name: "UNIONSELECT".into(),
        arguments: None,
    });
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::SqlInjectionAttempt(..)
    ));
}

/// Test XSS detection in memory content
#[test]
fn test_xss_detected_script_tag() {
    let json = serde_json::json!({
        "content": "<script>alert('xss')</script>",
        "layer": "stm"
    });
    let result = ValidatedMemoryWrite::from_json(json);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::XssAttempt(..)
    ));
}

/// Test XSS detection with javascript protocol
#[test]
fn test_xss_detected_javascript_protocol() {
    let json = serde_json::json!({
        "content": "javascript:alert(1)",
        "layer": "ltm"
    });
    let result = ValidatedMemoryWrite::from_json(json);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::XssAttempt(..)
    ));
}

/// Test XSS detection with event handlers
#[test]
fn test_xss_detected_event_handler() {
    let json = serde_json::json!({
        "content": "<img onerror=alert(1)>",
        "layer": "kg"
    });
    let result = ValidatedMemoryWrite::from_json(json);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::XssAttempt(..)
    ));
}

/// Test valid tool call passes validation
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

/// Test valid memory write passes validation
#[test]
fn test_valid_memory_write_passes() {
    let json = serde_json::json!({
        "content": "Hello, World! This is valid content.",
        "layer": "stm",
        "user_id": "user123",
        "session_id": "session456"
    });
    let result = ValidatedMemoryWrite::from_json(json);
    assert!(result.is_ok());
    let validated = result.unwrap();
    assert_eq!(validated.content, "Hello, World! This is valid content.");
    assert_eq!(validated.layer, "stm");
    assert_eq!(validated.user_id, Some("user123".to_string()));
}

/// Test invalid layer is rejected
#[test]
fn test_invalid_layer_rejected() {
    let json = serde_json::json!({
        "content": "hello",
        "layer": "invalid_layer"
    });
    let result = ValidatedMemoryWrite::from_json(json);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::InvalidFormat(..)
    ));
}

/// Test content exceeds max length is rejected
#[test]
fn test_content_exceeds_max_length_rejected() {
    let long_content = "x".repeat(1024 * 1024 + 1); // 1MB + 1 byte
    let json = serde_json::json!({
        "content": long_content,
        "layer": "stm"
    });
    let result = ValidatedMemoryWrite::from_json(json);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::ExceedsMaxLength(..)
    ));
}

/// Test missing required field is rejected
#[test]
fn test_missing_content_field_rejected() {
    let json = serde_json::json!({
        "layer": "stm"
    });
    let result = ValidatedMemoryWrite::from_json(json);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::MissingField(..)
    ));
}

/// Test missing layer field is rejected
#[test]
fn test_missing_layer_field_rejected() {
    let json = serde_json::json!({
        "content": "hello"
    });
    let result = ValidatedMemoryWrite::from_json(json);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::MissingField(..)
    ));
}

/// Test search query XSS detection
#[test]
fn test_search_query_xss_detected() {
    let json = serde_json::json!({
        "query": "<script>alert('xss')</script>"
    });
    let result = ValidatedSearchQuery::from_json(json);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::XssAttempt(..)
    ));
}

/// Test search query length limit
#[test]
fn test_search_query_exceeds_length_limit() {
    let json = serde_json::json!({
        "query": "a".repeat(1001)
    });
    let result = ValidatedSearchQuery::from_json(json);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::ExceedsMaxLength(..)
    ));
}

/// Test contains_sql_injection function directly
#[test]
fn test_contains_sql_injection_function() {
    // Positive cases
    assert!(contains_sql_injection("'; DROP TABLE"));
    assert!(contains_sql_injection(" UNION SELECT "));
    assert!(contains_sql_injection("exec xp_cmdshell"));
    assert!(contains_sql_injection("benchmark(1000000,MD5('test'))"));
    assert!(contains_sql_injection("sleep(5)"));

    // Negative cases
    assert!(!contains_sql_injection("Hello, World!"));
    assert!(!contains_sql_injection("Order 12345"));
    assert!(!contains_sql_injection("user_name"));
}

/// Test contains_xss function directly
#[test]
fn test_contains_xss_function() {
    // Positive cases
    assert!(contains_xss("<script>alert(1)</script>"));
    assert!(contains_xss("javascript:alert(1)"));
    assert!(contains_xss("<img onerror=alert(1)>"));
    assert!(contains_xss("<iframe src='x'>"));
    assert!(contains_xss("onclick=alert(1)"));

    // Negative cases
    assert!(!contains_xss("Hello, World!"));
    assert!(!contains_xss("Just some text"));
    assert!(!contains_xss("User message: Hello"));
}

/// Test validate_content_length function directly
#[test]
fn test_validate_content_length_function() {
    // Valid
    assert!(validate_content_length("short", 100).is_ok());
    assert!(validate_content_length("", 100).is_ok());
    assert!(validate_content_length("x".repeat(100).as_str(), 100).is_ok());

    // Invalid - exceeds limit
    assert!(validate_content_length("x".repeat(101).as_str(), 100).is_err());
    assert!(validate_content_length("a".repeat(1024 * 1024 + 1).as_str(), 1024 * 1024).is_err());
}

/// Test layer case insensitivity
#[test]
fn test_layer_case_insensitive() {
    let json_lower = serde_json::json!({
        "content": "hello",
        "layer": "stm"
    });
    let json_upper = serde_json::json!({
        "content": "hello",
        "layer": "STM"
    });
    let json_mixed = serde_json::json!({
        "content": "hello",
        "layer": "StM"
    });

    assert!(ValidatedMemoryWrite::from_json(json_lower).is_ok());
    assert!(ValidatedMemoryWrite::from_json(json_upper).is_ok());
    assert!(ValidatedMemoryWrite::from_json(json_mixed).is_ok());
}

/// Test tool name with underscore passes
#[test]
fn test_tool_name_with_underscore_passes() {
    let result = ValidatedToolCall::from_raw(ToolCallParams {
        name: "memory_write_v2".into(),
        arguments: None,
    });
    assert!(result.is_ok());
}

/// Test tool name with hyphen fails
#[test]
fn test_tool_name_with_hyphen_fails() {
    let result = ValidatedToolCall::from_raw(ToolCallParams {
        name: "memory-write".into(),
        arguments: None,
    });
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::DisallowedCharacters(..)
    ));
}

/// Test tool name length limit
#[test]
fn test_tool_name_length_limit() {
    // 100 chars - valid
    let result = ValidatedToolCall::from_raw(ToolCallParams {
        name: "a".repeat(100),
        arguments: None,
    });
    assert!(result.is_ok());

    // 101 chars - invalid
    let result = ValidatedToolCall::from_raw(ToolCallParams {
        name: "a".repeat(101),
        arguments: None,
    });
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::ExceedsMaxLength(..)
    ));
}

/// Test empty tool name fails
#[test]
fn test_empty_tool_name_fails() {
    let result = ValidatedToolCall::from_raw(ToolCallParams {
        name: "".into(),
        arguments: None,
    });
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::MissingField(..)
    ));
}

/// Test ValidationError status codes
#[test]
fn test_validation_error_status_codes() {
    assert_eq!(
        ValidationError::MissingField("test".into()).status_code(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        ValidationError::ExceedsMaxLength("test".into(), 100).status_code(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        ValidationError::SqlInjectionAttempt("test".into()).status_code(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        ValidationError::XssAttempt("test".into()).status_code(),
        StatusCode::BAD_REQUEST
    );
}

/// Test ValidationError error codes
#[test]
fn test_validation_error_codes() {
    assert_eq!(
        ValidationError::MissingField("test".into()).error_code(),
        "MISSING_FIELD"
    );
    assert_eq!(
        ValidationError::ExceedsMaxLength("test".into(), 100).error_code(),
        "EXCEEDS_MAX_LENGTH"
    );
    assert_eq!(
        ValidationError::DisallowedCharacters("test".into(), "bad".into()).error_code(),
        "DISALLOWED_CHARS"
    );
    assert_eq!(
        ValidationError::InvalidFormat("test".into(), "invalid".into()).error_code(),
        "INVALID_FORMAT"
    );
    assert_eq!(
        ValidationError::SqlInjectionAttempt("test".into()).error_code(),
        "SQL_INJECTION"
    );
    assert_eq!(
        ValidationError::XssAttempt("test".into()).error_code(),
        "XSS_ATTEMPT"
    );
}
