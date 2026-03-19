//! Visualization Data Router
//!
//! API endpoints that support frontend visualization widgets.

use axum::extract::Query;
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;

use crate::db::ltm::LTMRepository;
use crate::db::kg::KGRepository;
use crate::{json_ok, JsonResult};

/// Timeline entry for visualization
#[derive(Serialize, ToSchema)]
pub struct TimelineEntry {
    pub id: String,
    pub title: String,
    pub timestamp: String,
    pub layer: String,
    pub importance: f64,
}

/// Importance heatmap cell
#[derive(Serialize, ToSchema)]
pub struct HeatmapCell {
    pub x: i32,
    pub y: i32,
    pub value: f64,
    pub memory_id: Option<String>,
}

/// Knowledge graph node for visualization
#[derive(Serialize, ToSchema)]
pub struct VisualGraphNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub importance: f64,
}

/// Knowledge graph edge for visualization
#[derive(Serialize, ToSchema)]
pub struct VisualGraphEdge {
    pub source: String,
    pub target: String,
    pub label: String,
    pub strength: f64,
}

/// Knowledge graph visualization data
#[derive(Serialize, ToSchema)]
pub struct GraphVisualization {
    pub nodes: Vec<VisualGraphNode>,
    pub edges: Vec<VisualGraphEdge>,
}

/// Importance heatmap data
#[derive(Serialize, ToSchema)]
pub struct HeatmapData {
    pub cells: Vec<HeatmapCell>,
    pub max_value: f64,
    pub min_value: f64,
}

/// Timeline query parameters
#[derive(Deserialize, ToSchema)]
pub struct TimelineQuery {
    pub limit: Option<i32>,
    pub layer: Option<String>,
}

/// Get timeline data for visualization
pub async fn get_timeline(
    Query(query): Query<TimelineQuery>,
) -> JsonResult<Vec<TimelineEntry>> {
    info!("Getting timeline data");

    let limit = query.limit.unwrap_or(50);

    // Get LTM entries for timeline
    let entries = LTMRepository::list_entries(None, None, Some(limit), Some(0))
        .await?
        .entries;

    let timeline: Vec<TimelineEntry> = entries
        .into_iter()
        .map(|e| TimelineEntry {
            id: e.entry_id,
            title: e.title.unwrap_or_else(|| "Untitled".to_string()),
            timestamp: e.created_at,
            layer: "ltm".to_string(),
            importance: e.quality_score.unwrap_or(0.5) as f64,
        })
        .collect();

    json_ok(timeline)
}

/// Get knowledge graph visualization data
pub async fn get_graph_visualization(
    Query(query): Query<TimelineQuery>,
) -> JsonResult<GraphVisualization> {
    info!("Getting graph visualization data");

    let limit = query.limit.unwrap_or(100) as i32;

    // Get entities from KG
    let entities = KGRepository::list_entities(None, Some(limit), Some(0))
        .await?
        .entities;

    let nodes: Vec<VisualGraphNode> = entities
        .iter()
        .map(|e| VisualGraphNode {
            id: e.entity_id.clone(),
            label: e.entity_name.clone(),
            node_type: e.entity_type.clone(),
            importance: e.confidence_score as f64,
        })
        .collect();

    // For edges, we would need to query relations
    // For now, return empty edges
    let edges = Vec::new();

    json_ok(GraphVisualization { nodes, edges })
}

/// Get importance heatmap data
pub async fn get_heatmap(
    Query(query): Query<TimelineQuery>,
) -> JsonResult<HeatmapData> {
    info!("Getting heatmap data");

    let limit = query.limit.unwrap_or(100) as i32;

    // Get LTM entries for heatmap
    let entries = LTMRepository::list_entries(None, None, Some(limit), Some(0))
        .await?
        .entries;

    // Create heatmap cells (7 days x 24 hours grid)
    let mut cells = Vec::new();
    let mut max_value: f64 = 0.0;
    let mut min_value: f64 = f64::MAX;

    for (i, entry) in entries.iter().enumerate() {
        let importance = entry.quality_score.unwrap_or(0.5) as f64;
        let x = (i / 24) as i32 % 7; // Day of week
        let y = (i % 24) as i32;      // Hour of day

        cells.push(HeatmapCell {
            x,
            y,
            value: importance,
            memory_id: Some(entry.entry_id.clone()),
        });

        max_value = max_value.max(importance);
        min_value = min_value.min(importance);
    }

    if cells.is_empty() {
        min_value = 0.0;
    }

    json_ok(HeatmapData {
        cells,
        max_value,
        min_value,
    })
}

/// Get memory statistics for dashboard
#[derive(Serialize, ToSchema)]
pub struct MemoryStatsDashboard {
    pub total_memories: usize,
    pub by_layer: std::collections::HashMap<String, usize>,
    pub avg_importance: f64,
}

/// Get dashboard statistics
pub async fn get_dashboard_stats() -> JsonResult<MemoryStatsDashboard> {
    info!("Getting dashboard statistics");

    // Get LTM count
    let ltm_count = LTMRepository::list_entries(None, None, Some(1000), Some(0))
        .await?
        .total;

    let mut by_layer = std::collections::HashMap::new();
    by_layer.insert("ltm".to_string(), ltm_count);
    // Would need to query other layers as well

    // Calculate average importance from LTM
    let avg_importance = if ltm_count > 0 {
        let entries = LTMRepository::list_entries(None, None, Some(ltm_count as i32), Some(0))
            .await?
            .entries;

        let sum: f64 = entries
            .iter()
            .map(|e| e.quality_score.unwrap_or(0.5) as f64)
            .sum();
        sum / ltm_count as f64
    } else {
        0.0
    };

    json_ok(MemoryStatsDashboard {
        total_memories: ltm_count,
        by_layer,
        avg_importance,
    })
}
