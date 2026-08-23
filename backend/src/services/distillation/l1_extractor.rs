use anyhow::Result;
use chrono::Utc;
fn now_str() -> String { Utc::now().to_rfc3339() }
use tracing::{error, info, warn};

use crate::db::stm::SessionMessage;
use crate::services::embedding::EmbeddingService;
use crate::services::llm::LLMService;

use super::prompts;
use super::repository::DistillationRepository;
use super::types::*;

pub struct L1Extractor {
    llm_base_url: String,
    llm_model: String,
}

impl L1Extractor {
    pub fn new(llm_base_url: &str, llm_model: &str) -> Self {
        Self {
            llm_base_url: llm_base_url.to_string(),
            llm_model: llm_model.to_string(),
        }
    }

    pub async fn extract_from_messages(
        &self,
        messages: &[SessionMessage],
        session_id: &str,
        user_id: &str,
        agent_id: &str,
        tenant_id: &str,
        previous_scene: Option<&str>,
    ) -> Result<Vec<(ExtractedScene, Vec<MemoryAtom>)>> {
        if messages.is_empty() {
            return Ok(vec![]);
        }

        let background_count = messages.len().min(5);
        let background_messages = &messages[..background_count];
        let new_messages = &messages[background_count..];

        if new_messages.is_empty() {
            return Ok(vec![]);
        }

        let bg_text = format_messages(background_messages);
        let new_text = format_messages(new_messages);
        let prev_scene = previous_scene.unwrap_or("(无)");

        let user_prompt = prompts::format_l1_extraction_user_prompt(prev_scene, &bg_text, &new_text);

        let response = self.call_llm(
            prompts::L1_EXTRACTION_SYSTEM_PROMPT,
            &user_prompt,
        ).await?;

        let scenes: Vec<ExtractedScene> = parse_extraction_response(&response)?;

        let mut results = Vec::new();
        for scene in scenes {
            let atoms: Vec<MemoryAtom> = scene.memories.iter()
                .filter(|m| m.priority >= 50 || m.priority == -1)
                .map(|m| {
                    MemoryAtom {
                        id: DistillationRepository::generate_id(),
                        atom_type: MemoryAtomType::from_str(&m.memory_type)
                            .unwrap_or(MemoryAtomType::Episodic),
                        content: m.content.clone(),
                        priority: m.priority,
                        scene_name: scene.scene_name.clone(),
                        source_message_ids: m.source_message_ids.clone(),
                        session_id: session_id.to_string(),
                        user_id: user_id.to_string(),
                        agent_id: agent_id.to_string(),
                        tenant_id: tenant_id.to_string(),
                        metadata: m.metadata.clone(),
                        version: 1,
                        created_at: now_str(),
                        updated_at: now_str(),
                    }
                })
                .collect();

            results.push((scene, atoms));
        }

        info!(
            "L1 extraction complete: {} scenes, {} atoms",
            results.len(),
            results.iter().map(|(_, a)| a.len()).sum::<usize>()
        );

        Ok(results)
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
                "temperature": 0.3,
                "num_predict": 4096
            }
        });

        let response = client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("LLM API error: status={}, body={}", status, text));
        }

        #[derive(serde::Deserialize)]
        struct OllamaResponse {
            response: String,
        }

        let resp: OllamaResponse = response.json().await?;
        Ok(resp.response)
    }
}

fn format_messages(messages: &[SessionMessage]) -> String {
    messages
        .iter()
        .map(|m| format!("[{}] {}: {}", m.message_id, m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_extraction_response(response: &str) -> Result<Vec<ExtractedScene>> {
    let trimmed = response.trim();

    let json_start = trimmed.find('[').unwrap_or(0);
    let json_end = trimmed.rfind(']').map(|i| i + 1).unwrap_or(trimmed.len());
    let json_text = &trimmed[json_start..json_end];

    serde_json::from_str::<Vec<ExtractedScene>>(json_text)
        .map_err(|e| {
            warn!("Failed to parse L1 extraction response: {}", e);
            anyhow::anyhow!("JSON parse error: {}", e)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_extraction_response() {
        let response = r#"[
            {
                "scene_name": "讨论项目架构",
                "message_ids": ["msg1", "msg2"],
                "memories": [
                    {
                        "content": "用户是一名资深后端工程师，主要使用 Rust 语言",
                        "type": "persona",
                        "priority": 85,
                        "source_message_ids": ["msg1"],
                        "metadata": {}
                    }
                ]
            }
        ]"#;

        let scenes = parse_extraction_response(response).unwrap();
        assert_eq!(scenes.len(), 1);
        assert_eq!(scenes[0].scene_name, "讨论项目架构");
        assert_eq!(scenes[0].memories.len(), 1);
        assert_eq!(scenes[0].memories[0].memory_type, "persona");
        assert_eq!(scenes[0].memories[0].priority, 85);
    }

    #[test]
    fn test_parse_empty_memories() {
        let response = r#"[{"scene_name": "闲聊", "message_ids": ["m1"], "memories": []}]"#;
        let scenes = parse_extraction_response(response).unwrap();
        assert_eq!(scenes.len(), 1);
        assert!(scenes[0].memories.is_empty());
    }
}
