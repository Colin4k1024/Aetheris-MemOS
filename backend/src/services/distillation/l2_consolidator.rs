//! L2 consolidation parse helpers — backend-agnostic, reused by the PG
//! distillation worker (`worker.rs`). The SQLite `L2Consolidator` struct +
//! its `consolidate`/`call_llm`/`build_scene_block` (which targeted the dead
//! SQLite pipeline) were removed when the SQLite distillation island was
//! deleted (#88 unify); the parse logic is the only live part.

use anyhow::Result;
use tracing::warn;

/// One scene update parsed from the LLM's L2 consolidation response. Reused by
/// the PG distillation worker.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SceneUpdate {
    pub scene_id: String,
    pub name: String,
    pub summary: String,
    pub content: String,
    pub atom_ids: Vec<String>,
}

/// Parse the LLM's L2 consolidation response (a JSON array of `SceneUpdate`,
/// possibly wrapped in prose) into a typed list.
pub fn parse_consolidation_response(response: &str) -> Result<Vec<SceneUpdate>> {
    let trimmed = response.trim();
    let json_start = trimmed.find('[').unwrap_or(0);
    let json_end = trimmed.rfind(']').map(|i| i + 1).unwrap_or(trimmed.len());
    let json_text = &trimmed[json_start..json_end];

    serde_json::from_str::<Vec<SceneUpdate>>(json_text).map_err(|e| {
        warn!("Failed to parse L2 consolidation response: {}", e);
        anyhow::anyhow!("L2 JSON parse error: {}", e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_l2_consolidation_json_array() {
        // The LLM wraps the JSON array in prose; the parser must extract the
        // array and deserialize SceneUpdate rows.
        let resp = r#"以下是整合结果：
[
  {"scene_id":"new","name":"setup","summary":"项目初始化","content":"用户完成了项目初始化","atom_ids":["a1","a2"]}
]
完成"#;
        let updates = parse_consolidation_response(resp).unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].name, "setup");
        assert_eq!(
            updates[0].atom_ids,
            vec!["a1".to_string(), "a2".to_string()]
        );
    }

    #[test]
    fn parse_returns_err_on_non_json() {
        assert!(parse_consolidation_response("no json array here").is_err());
    }
}
