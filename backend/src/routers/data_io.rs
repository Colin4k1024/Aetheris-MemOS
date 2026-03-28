//! Data Import/Export Router
//!
//! This module provides endpoints for exporting and importing memory data.

use axum::{
    extract::Query,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use tracing::info;

use crate::db::{kg::KGRepository, ltm::LTMRepository, mm::MMRepository, pool, stm::STMRepository};
use crate::tenant::TenantId;
use crate::{json_ok, AppError, JsonResult};

/// Export format
#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    /// Export format: "json" or "markdown"
    #[serde(default = "default_format")]
    pub format: String,

    /// Memory layer to export: "stm", "ltm", "kg", "mm", or "all"
    #[serde(default = "default_layer")]
    pub layer: String,

    /// Limit number of records
    #[serde(default = "default_limit")]
    pub limit: i32,
}

fn default_format() -> String {
    "json".to_string()
}

fn default_layer() -> String {
    "all".to_string()
}

fn default_limit() -> i32 {
    100
}

/// Build data I/O router
pub fn router() -> Router {
    Router::new()
        .route("/data/export", get(export_data))
        .route("/data/import", post(import_data))
}

/// Export data handler
async fn export_data(Query(query): Query<ExportQuery>) -> JsonResult<serde_json::Value> {
    info!(
        "Exporting data: layer={}, format={}, limit={}",
        query.layer, query.format, query.limit
    );

    let limit = query.limit;
    let format = query.format.to_lowercase();

    match format.as_str() {
        "json" => export_as_json(&query.layer, limit).await,
        "markdown" => export_as_markdown(&query.layer, limit).await,
        _ => Err(AppError::BadRequest(format!(
            "Unsupported format: {}. Use 'json' or 'markdown'",
            format
        ))),
    }
}

async fn export_as_json(layer: &str, limit: i32) -> JsonResult<serde_json::Value> {
    let mut data = serde_json::json!({});
    let pool = pool();
    // Default tenant for export (backward compatibility)
    let default_tenant = TenantId::from_string("default");

    match layer {
        "stm" | "all" => {
            let response = STMRepository::list_sessions(&pool, &default_tenant, None, None, Some(limit), Some(0)).await?;
            data["stm"] = serde_json::json!({
                "sessions": response.sessions,
                "count": response.sessions.len()
            });
        }
        _ => {}
    }

    match layer {
        "ltm" | "all" => {
            let response = LTMRepository::list_entries(&pool, &default_tenant, None, None, Some(limit), Some(0)).await?;
            data["ltm"] = serde_json::json!({
                "entries": response.entries,
                "count": response.entries.len()
            });
        }
        _ => {}
    }

    match layer {
        "kg" | "all" => {
            let response = KGRepository::list_entities(&pool, &default_tenant, None, Some(limit), Some(0)).await?;
            data["kg"] = serde_json::json!({
                "entities": response.entities,
                "count": response.entities.len()
            });
        }
        _ => {}
    }

    match layer {
        "mm" | "all" => {
            let response = MMRepository::list_entries(None, Some(limit), Some(0)).await?;
            data["mm"] = serde_json::json!({
                "entries": response.entries,
                "count": response.entries.len()
            });
        }
        _ => {}
    }

    data["metadata"] = serde_json::json!({
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "layer": layer,
        "format": "json"
    });

    json_ok(data)
}

async fn export_as_markdown(layer: &str, limit: i32) -> JsonResult<serde_json::Value> {
    let mut content = String::new();
    let pool = pool();
    // Default tenant for export (backward compatibility)
    let default_tenant = TenantId::from_string("default");

    content.push_str("# Adaptive Memory System Export\n\n");
    content.push_str(&format!("Exported at: {}\n\n", chrono::Utc::now().to_rfc3339()));

    if layer == "stm" || layer == "all" {
        content.push_str("## Short-Term Memory (STM)\n\n");
        let response = STMRepository::list_sessions(&pool, &default_tenant, None, None, Some(limit), Some(0)).await?;
        for session in response.sessions {
            content.push_str(&format!(
                "### Session: {}\n- User: {}\n- Agent: {}\n- Type: {}\n- Status: {}\n- Created: {}\n\n",
                session.session_id,
                session.user_id,
                session.agent_id,
                session.session_type,
                session.status,
                session.created_at
            ));
        }
    }

    if layer == "ltm" || layer == "all" {
        content.push_str("## Long-Term Memory (LTM)\n\n");
        let response = LTMRepository::list_entries(&pool, &default_tenant, None, None, Some(limit), Some(0)).await?;
        for entry in response.entries {
            content.push_str(&format!(
                "### {}\n{}\n\n---\n\n",
                entry.title.as_deref().unwrap_or("Untitled"),
                entry.content
            ));
        }
    }

    if layer == "kg" || layer == "all" {
        content.push_str("## Knowledge Graph (KG)\n\n");
        let response = KGRepository::list_entities(&pool, &default_tenant, None, Some(limit), Some(0)).await?;
        for entity in response.entities {
            content.push_str(&format!(
                "### {} ({})\n{}\n\n",
                entity.entity_name,
                entity.entity_type,
                entity.description.as_deref().unwrap_or("")
            ));
        }
    }

    if layer == "mm" || layer == "all" {
        content.push_str("## Multimodal Memory (MM)\n\n");
        let response = MMRepository::list_entries(None, Some(limit), Some(0)).await?;
        for entry in response.entries {
            content.push_str(&format!(
                "### {} ({})\nType: {}\nQuality: {:.2}\n\n",
                entry.title.as_deref().unwrap_or("Untitled"),
                entry.entry_id,
                entry.modality_type,
                entry.quality_score
            ));
            if let Some(text) = &entry.text_content {
                content.push_str(&format!("{}\n\n", text));
            }
        }
    }

    json_ok(serde_json::json!({
        "format": "markdown",
        "content": content,
        "layer": layer
    }))
}

/// Import data request
#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    /// Data format: "json"
    pub format: String,

    /// Data to import
    pub data: serde_json::Value,

    /// Import mode: "merge" or "replace"
    #[serde(default = "default_import_mode")]
    pub mode: String,
}

fn default_import_mode() -> String {
    "merge".to_string()
}

/// Import data handler
async fn import_data(Json(req): Json<ImportRequest>) -> JsonResult<serde_json::Value> {
    info!("Importing data: format={}, mode={}", req.format, req.mode);

    if req.format.to_lowercase() != "json" {
        return Err(AppError::BadRequest(format!(
            "Unsupported format: {}. Only 'json' is supported for import",
            req.format
        )));
    }

    let mut imported = serde_json::json!({
        "stm": 0,
        "ltm": 0,
        "kg": 0,
        "mm": 0
    });

    // Import STM data
    if let Some(stm_data) = req.data.get("stm") {
        if let Some(sessions) = stm_data.get("sessions").and_then(|s| s.as_array()) {
            // Note: Actual import would create sessions here
            imported["stm"] = serde_json::json!(sessions.len());
            info!("Would import {} STM sessions", sessions.len());
        }
    }

    // Import LTM data
    if let Some(ltm_data) = req.data.get("ltm") {
        if let Some(entries) = ltm_data.get("entries").and_then(|e| e.as_array()) {
            imported["ltm"] = serde_json::json!(entries.len());
            info!("Would import {} LTM entries", entries.len());
        }
    }

    // Import KG data
    if let Some(kg_data) = req.data.get("kg") {
        if let Some(entities) = kg_data.get("entities").and_then(|e| e.as_array()) {
            imported["kg"] = serde_json::json!(entities.len());
            info!("Would import {} KG entities", entities.len());
        }
    }

    // Import MM data
    if let Some(mm_data) = req.data.get("mm") {
        if let Some(entries) = mm_data.get("entries").and_then(|e| e.as_array()) {
            imported["mm"] = serde_json::json!(entries.len());
            info!("Would import {} MM entries", entries.len());
        }
    }

    json_ok(serde_json::json!({
        "success": true,
        "imported": imported,
        "message": "Data import functionality is a placeholder. Full import requires implementing session/entity creation."
    }))
}
