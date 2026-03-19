//! Storage Configuration Module
//!
//! This module provides configuration for different storage backends.

use std::path::PathBuf;

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

    /// Resolve the cross-platform SQLite data directory and return a ready-to-use config.
    ///
    /// Resolution order:
    /// 1. `OPENWEBUI_DATA_DIR` env var
    /// 2. `ADAPTIVE_MEMORY_DATA_DIR` env var  
    /// 3. OS-specific data directory (`~/.local/share` on Linux, `~/Library/Application Support` on macOS, `%APPDATA%` on Windows)
    /// 4. `~/.adaptive-memory` (HOME-based fallback)
    /// 5. Current working directory
    pub fn resolve_local_sqlite(db_filename: &str) -> Self {
        let data_dir = resolve_data_directory();
        let db_path = data_dir.join(db_filename);

        // Log for diagnostics
        if let Some(p) = db_path.to_str() {
            eprintln!("[storage] SQLite database path resolved to: {}", p);
        }

        let url = format!(
            "sqlite:{}",
            db_path.to_str().unwrap_or("data/adaptive_memory.db")
        );
        Self {
            backend: StorageBackend::Sqlite,
            url,
            in_memory: false,
            journal_mode: "WAL".to_string(),
        }
    }
}

/// Resolve the platform data directory for the application.
pub fn resolve_data_directory() -> PathBuf {
    // 1. Explicit override env vars
    for env_var in &["OPENWEBUI_DATA_DIR", "ADAPTIVE_MEMORY_DATA_DIR"] {
        if let Ok(dir) = std::env::var(env_var) {
            if !dir.is_empty() {
                let path = PathBuf::from(&dir).join("adaptive-memory");
                if std::fs::create_dir_all(&path).is_ok() {
                    return path;
                }
            }
        }
    }

    // 2. OS-specific application data directory
    if let Some(data_dir) = os_data_dir() {
        let path = data_dir.join("adaptive-memory");
        if std::fs::create_dir_all(&path).is_ok() {
            return path;
        }
    }

    // 3. HOME-based fallback
    if let Some(home) = home_dir() {
        let path = home.join(".adaptive-memory");
        if std::fs::create_dir_all(&path).is_ok() {
            return path;
        }
    }

    // 4. Current working directory fallback
    let path = PathBuf::from("data");
    let _ = std::fs::create_dir_all(&path);
    path
}

/// Returns the OS-specific application data directory.
fn os_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library").join("Application Support"))
    }

    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA").ok().map(PathBuf::from)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // XDG base directory spec for Linux and other Unix systems
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".local").join("share")))
    }
}

/// Returns the user's home directory in a cross-platform way.
fn home_dir() -> Option<PathBuf> {
    // Prefer explicit HOME env var over deprecated std::env::home_dir()
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("USERPROFILE").ok().map(PathBuf::from))
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
