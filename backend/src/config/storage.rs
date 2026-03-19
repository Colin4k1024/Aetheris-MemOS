//! Storage Configuration Module
//!
//! This module provides configuration for different storage backends.

use serde::{Deserialize, Serialize};

/// Storage backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    /// PostgreSQL - for production deployments
    Postgres,
    /// SQLite - for local-first deployments
    Sqlite,
}

impl Default for StorageBackend {
    fn default() -> Self {
        // Default to SQLite for local development
        Self::Sqlite
    }
}

impl StorageBackend {
    /// Get the backend type from a connection string
    pub fn from_url(url: &str) -> Self {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            Self::Postgres
        } else if url.starts_with("sqlite://")
            || url.ends_with(".db")
            || url.ends_with(".sqlite")
            || url.ends_with(".sqlite3")
        {
            Self::Sqlite
        } else {
            // Default to SQLite for local files
            Self::Sqlite
        }
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage backend type
    #[serde(default)]
    pub backend: StorageBackend,
    /// Database connection URL
    /// For SQLite: file path or sqlite://path
    /// For PostgreSQL: postgres://user:pass@host/db
    #[serde(alias = "database_url")]
    pub url: String,
    /// Enable in-memory SQLite database (for testing)
    #[serde(default)]
    pub in_memory: bool,
    /// SQLite journal mode
    #[serde(default = "default_journal_mode")]
    pub journal_mode: String,
}

fn default_journal_mode() -> String {
    "WAL".to_string()
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::Sqlite,
            url: "sqlite://data/adaptive_memory.db".to_string(),
            in_memory: false,
            journal_mode: "WAL".to_string(),
        }
    }
}

impl StorageConfig {
    /// Create a new storage config from URL
    pub fn from_url(url: impl Into<String>) -> Self {
        let url = url.into();
        let backend = StorageBackend::from_url(&url);
        Self {
            backend,
            url,
            in_memory: false,
            journal_mode: "WAL".to_string(),
        }
    }

    /// Create an in-memory SQLite database
    pub fn in_memory() -> Self {
        Self {
            backend: StorageBackend::Sqlite,
            url: "sqlite::memory:".to_string(),
            in_memory: true,
            journal_mode: "MEMORY".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_backend_from_url() {
        assert_eq!(
            StorageBackend::from_url("postgres://localhost/db"),
            StorageBackend::Postgres
        );
        assert_eq!(
            StorageBackend::from_url("postgresql://localhost/db"),
            StorageBackend::Postgres
        );
        assert_eq!(
            StorageBackend::from_url("sqlite://data.db"),
            StorageBackend::Sqlite
        );
        assert_eq!(StorageBackend::from_url("data.db"), StorageBackend::Sqlite);
    }

    #[test]
    fn test_storage_config_in_memory() {
        let config = StorageConfig::in_memory();
        assert_eq!(config.backend, StorageBackend::Sqlite);
        assert!(config.in_memory);
    }
}
