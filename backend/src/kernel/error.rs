//! Memory Kernel - Unified Error Types
//!
//! This module defines the core error types for the Memory Kernel system.

use thiserror::Error;

/// Unified error type for Memory Kernel operations.
#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Memory not found: {0}")]
    NotFound(String),

    #[error("Memory already exists: {0}")]
    AlreadyExists(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Resource constraint violated: {0}")]
    ResourceConstraint(String),

    #[error("Layer error: {0}")]
    Layer(String),

    #[error("Policy error: {0}")]
    Policy(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type MemoryResult<T> = Result<T, MemoryError>;

impl From<sqlx::Error> for MemoryError {
    fn from(err: sqlx::Error) -> Self {
        MemoryError::Storage(err.to_string())
    }
}

impl From<serde_json::Error> for MemoryError {
    fn from(err: serde_json::Error) -> Self {
        MemoryError::Serialization(err.to_string())
    }
}

impl From<std::io::Error> for MemoryError {
    fn from(err: std::io::Error) -> Self {
        MemoryError::Storage(err.to_string())
    }
}
