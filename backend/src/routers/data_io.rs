//! Data Import/Export Router
//!
//! This module provides endpoints for exporting and importing memory data.

use axum::{
    extract::Query,
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use tracing::info;

use crate::db::{kg::KGRepository, ltm::LTMRepository, mm::MMRepository, pool, stm::STMRepository};
use crate::tenant::{RequestTenantContext, TenantId};
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
        .route_layer(middleware::from_fn(crate::hoops::jwt::auth_middleware))
}

/// Export data handler
async fn export_data(
    tenant_ctx: RequestTenantContext,
    Query(query): Query<ExportQuery>,
) -> JsonResult<serde_json::Value> {
    if !matches!(query.layer.as_str(), "stm" | "ltm" | "kg" | "mm" | "all") {
        return Err(AppError::BadRequest(format!(
            "Unsupported export layer: {}. Use stm, ltm, kg, mm, or all",
            query.layer
        )));
    }
    if !(1..=1_000).contains(&query.limit) {
        return Err(AppError::BadRequest(
            "Export limit must be between 1 and 1000".to_string(),
        ));
    }

    info!(
        "Exporting data: tenant_id={}, layer={}, format={}, limit={}",
        tenant_ctx.tenant_id, query.layer, query.format, query.limit
    );

    let limit = query.limit;
    let format = query.format.to_lowercase();

    match format.as_str() {
        "json" => export_as_json(&query.layer, limit, &tenant_ctx.tenant_id).await,
        "markdown" => export_as_markdown(&query.layer, limit, &tenant_ctx.tenant_id).await,
        _ => Err(AppError::BadRequest(format!(
            "Unsupported format: {}. Use 'json' or 'markdown'",
            format
        ))),
    }
}

async fn export_as_json(
    layer: &str,
    limit: i32,
    tenant_id: &TenantId,
) -> JsonResult<serde_json::Value> {
    let mut data = serde_json::json!({});
    let pool = pool();

    match layer {
        "stm" | "all" => {
            let response =
                STMRepository::list_sessions(&pool, tenant_id, None, None, Some(limit), Some(0))
                    .await?;
            data["stm"] = serde_json::json!({
                "sessions": response.sessions,
                "count": response.sessions.len()
            });
        }
        _ => {}
    }

    match layer {
        "ltm" | "all" => {
            let response =
                LTMRepository::list_entries(&pool, tenant_id, None, None, Some(limit), Some(0))
                    .await?;
            data["ltm"] = serde_json::json!({
                "entries": response.entries,
                "count": response.entries.len()
            });
        }
        _ => {}
    }

    match layer {
        "kg" | "all" => {
            let response =
                KGRepository::list_entities(&pool, tenant_id, None, Some(limit), Some(0)).await?;
            data["kg"] = serde_json::json!({
                "entities": response.entities,
                "count": response.entities.len()
            });
        }
        _ => {}
    }

    match layer {
        "mm" | "all" => {
            let response =
                MMRepository::list_entries(None, Some(limit), Some(0), tenant_id.as_str())
                    .await?;
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
        "format": "json",
        "tenant_id": tenant_id.as_str()
    });

    json_ok(data)
}

async fn export_as_markdown(
    layer: &str,
    limit: i32,
    tenant_id: &TenantId,
) -> JsonResult<serde_json::Value> {
    let mut content = String::new();
    let pool = pool();

    content.push_str("# Adaptive Memory System Export\n\n");
    content.push_str(&format!(
        "Exported at: {}\n\n",
        chrono::Utc::now().to_rfc3339()
    ));

    if layer == "stm" || layer == "all" {
        content.push_str("## Short-Term Memory (STM)\n\n");
        let response =
            STMRepository::list_sessions(&pool, tenant_id, None, None, Some(limit), Some(0))
                .await?;
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
        let response =
            LTMRepository::list_entries(&pool, tenant_id, None, None, Some(limit), Some(0)).await?;
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
        let response =
            KGRepository::list_entities(&pool, tenant_id, None, Some(limit), Some(0)).await?;
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
        let response =
            MMRepository::list_entries(None, Some(limit), Some(0), tenant_id.as_str())
                .await?;
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
        "layer": layer,
        "tenant_id": tenant_id.as_str()
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
async fn import_data(
    _tenant_ctx: RequestTenantContext,
    Json(_req): Json<ImportRequest>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "success": false,
            "supported": false,
            "message": "Data import is not supported. No data was written."
        })),
    )
}
