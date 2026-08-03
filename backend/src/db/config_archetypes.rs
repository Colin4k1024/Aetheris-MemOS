//! Config archetype repository (P3-lite).
//!
//! Provides access to the `config_archetypes` table which stores the candidate
//! configuration space for the recommendation engine. The scheduler uses these
//! archetypes to generate candidates for argmax selection.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::error;

use crate::db::pool;
use crate::AppError;

/// Config archetype from the database.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConfigArchetype {
    pub archetype_id: String,
    pub name: String,
    pub description: Option<String>,
    pub config_json: String,
    pub is_active: bool,
    pub created_at: String,
}

/// Parsed config values from the JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchetypeConfig {
    pub stm_weight: f64,
    pub ltm_weight: f64,
    pub kg_weight: f64,
    pub mm_weight: f64,
    pub reasoning_depth: String,
    pub enable_multimodal: bool,
    pub primary_memory: String,
    pub secondary_memory: Vec<String>,
}

impl ConfigArchetype {
    /// Parse the config_json into structured values.
    pub fn parse_config(&self) -> Result<ArchetypeConfig, AppError> {
        serde_json::from_str(&self.config_json).map_err(|e| {
            error!(
                "Failed to parse config_json for archetype {}: {}",
                self.archetype_id, e
            );
            AppError::Internal(format!("Invalid config_json: {}", e))
        })
    }
}

/// Repository for config archetypes.
pub struct ConfigArchetypeRepository;

impl ConfigArchetypeRepository {
    /// Get all active config archetypes.
    pub async fn list_active() -> Result<Vec<ConfigArchetype>, AppError> {
        let pool = pool();
        let archetypes = sqlx::query_as::<_, ConfigArchetype>(
            r#"
            SELECT archetype_id, name, description, config_json, is_active, created_at::text
            FROM config_archetypes
            WHERE is_active = true
            ORDER BY archetype_id
            "#,
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to list config archetypes: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(archetypes)
    }

    /// Get a specific config archetype by ID.
    pub async fn get_by_id(archetype_id: &str) -> Result<Option<ConfigArchetype>, AppError> {
        let pool = pool();
        let archetype = sqlx::query_as::<_, ConfigArchetype>(
            r#"
            SELECT archetype_id, name, description, config_json, is_active, created_at::text
            FROM config_archetypes
            WHERE archetype_id = $1
            "#,
        )
        .bind(archetype_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to get config archetype {}: {}", archetype_id, e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(archetype)
    }

    /// Create a new config archetype.
    pub async fn create(
        archetype_id: &str,
        name: &str,
        description: Option<&str>,
        config_json: &str,
    ) -> Result<(), AppError> {
        let pool = pool();
        sqlx::query(
            r#"
            INSERT INTO config_archetypes (archetype_id, name, description, config_json)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (archetype_id) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                config_json = EXCLUDED.config_json
            "#,
        )
        .bind(archetype_id)
        .bind(name)
        .bind(description)
        .bind(config_json)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create config archetype {}: {}", archetype_id, e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(())
    }

    /// Update a config archetype's active status.
    pub async fn set_active(archetype_id: &str, is_active: bool) -> Result<(), AppError> {
        let pool = pool();
        sqlx::query(
            r#"
            UPDATE config_archetypes
            SET is_active = $2
            WHERE archetype_id = $1
            "#,
        )
        .bind(archetype_id)
        .bind(is_active)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to update config archetype {}: {}", archetype_id, e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(())
    }
}
