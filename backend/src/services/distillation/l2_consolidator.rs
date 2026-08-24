use anyhow::Result;
use chrono::Utc;
fn now_str() -> String { Utc::now().to_rfc3339() }
use tracing::{info, warn};

use super::prompts;
use super::repository::DistillationRepository;
use super::types::*;

pub struct L2Consolidator {
    llm_base_url: String,
    llm_model: String,
    max_scenes: usize,
}

impl L2Consolidator {
    pub fn new(llm_base_url: &str, llm_model: &str, max_scenes: usize) -> Self {
        Self {
            llm_base_url: llm_base_url.to_string(),
            llm_model: llm_model.to_string(),
            max_scenes,
        }
    }

    pub async fn consolidate(
        &self,
        new_atoms: &[MemoryAtom],
        existing_scenes: &[SceneBlock],
        user_id: &str,
        tenant_id: &str,
    ) -> Result<L2ConsolidationResult> {
        if new_atoms.is_empty() {
            return Ok(L2ConsolidationResult {
                success: true,
                scenes_created: 0,
                scenes_updated: 0,
                atoms_processed: 0,
            });
        }

        let atoms_text = new_atoms.iter()
            .map(|a| format!(
                "- [{}] (scene: {}, type: {}, priority: {}) {}",
                a.id, a.scene_name, a.atom_type, a.priority, a.content
            ))
            .collect::<Vec<_>>()
            .join("\n");

        let scenes_text = if existing_scenes.is_empty() {
            "(无现有场景)".to_string()
        } else {
            existing_scenes.iter()
                .map(|s| format!("- [{}] {} — {}", s.id, s.name, s.summary))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let user_prompt = prompts::format_l2_consolidation_user_prompt(&atoms_text, &scenes_text);

        let response = self.call_llm(
            prompts::L2_CONSOLIDATION_SYSTEM_PROMPT,
            &user_prompt,
        ).await?;

        let scene_updates = parse_consolidation_response(&response)?;

        let mut created = 0;
        let mut updated = 0;

        for update in &scene_updates {
            if update.scene_id == "new" || !existing_scenes.iter().any(|s| s.id == update.scene_id) {
                created += 1;
            } else {
                updated += 1;
            }
        }

        info!(
            "L2 consolidation: {} scenes created, {} updated, {} atoms processed",
            created, updated, new_atoms.len()
        );

        Ok(L2ConsolidationResult {
            success: true,
            scenes_created: created,
            scenes_updated: updated,
            atoms_processed: new_atoms.len(),
        })
    }

    pub fn build_scene_block(
        update: &SceneUpdate,
        user_id: &str,
        tenant_id: &str,
        existing_id: Option<&str>,
    ) -> SceneBlock {
        let now = now_str();
        SceneBlock {
            id: existing_id.unwrap_or(&DistillationRepository::generate_id()).to_string(),
            name: update.name.clone(),
            summary: update.summary.clone(),
            content: update.content.clone(),
            heat: 0.0,
            atom_ids: update.atom_ids.clone(),
            user_id: user_id.to_string(),
            tenant_id: tenant_id.to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
        }
    }

    async fn call_llm(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let client = reqwest::Client::new();
        let url = format!("{}/api/generate", self.llm_base_url);

        let full_prompt = format!(
            "<|system|>\n{}\n<|user|>\n{}\n<|assistant|>",
            system_prompt, user_prompt
        );

        let body = serde_json::json!({
            "model": self.llm_model,
            "prompt": full_prompt,
            "stream": false,
            "options": {
                "temperature": 0.4,
                "num_predict": 8192
            }
        });

        let response = client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(180))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("LLM L2 consolidation call failed: {}", response.status()));
        }

        #[derive(serde::Deserialize)]
        struct OllamaResponse { response: String }

        let resp: OllamaResponse = response.json().await?;
        Ok(resp.response)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SceneUpdate {
    pub scene_id: String,
    pub name: String,
    pub summary: String,
    pub content: String,
    pub atom_ids: Vec<String>,
}

pub fn parse_consolidation_response(response: &str) -> Result<Vec<SceneUpdate>> {
    let trimmed = response.trim();
    let json_start = trimmed.find('[').unwrap_or(0);
    let json_end = trimmed.rfind(']').map(|i| i + 1).unwrap_or(trimmed.len());
    let json_text = &trimmed[json_start..json_end];

    serde_json::from_str::<Vec<SceneUpdate>>(json_text)
        .map_err(|e| {
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
        assert_eq!(updates[0].atom_ids, vec!["a1".to_string(), "a2".to_string()]);
    }

    #[test]
    fn parse_returns_err_on_non_json() {
        assert!(parse_consolidation_response("no json array here").is_err());
    }
}
