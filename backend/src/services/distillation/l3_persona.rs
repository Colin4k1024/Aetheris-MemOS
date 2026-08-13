use anyhow::Result;
use chrono::Utc;
fn now_str() -> String { Utc::now().to_rfc3339() }
use tracing::{info, warn};

use super::prompts;
use super::repository::DistillationRepository;
use super::types::*;

pub struct L3PersonaGenerator {
    llm_base_url: String,
    llm_model: String,
}

impl L3PersonaGenerator {
    pub fn new(llm_base_url: &str, llm_model: &str) -> Self {
        Self {
            llm_base_url: llm_base_url.to_string(),
            llm_model: llm_model.to_string(),
        }
    }

    pub async fn generate(
        &self,
        existing_persona: Option<&Persona>,
        scenes: &[SceneBlock],
        user_id: &str,
        agent_id: Option<&str>,
        tenant_id: &str,
    ) -> Result<Persona> {
        if scenes.is_empty() {
            return Err(anyhow::anyhow!("No scenes available for persona generation"));
        }

        let existing_text = existing_persona
            .map(|p| p.content.as_str())
            .unwrap_or("(尚无画像)");

        let scene_contents = scenes.iter()
            .map(|s| format!("## {}\n{}\n", s.name, s.content))
            .collect::<Vec<_>>()
            .join("\n---\n");

        let user_prompt = prompts::format_l3_persona_user_prompt(existing_text, &scene_contents);

        let persona_content = self.call_llm(
            prompts::L3_PERSONA_SYSTEM_PROMPT,
            &user_prompt,
        ).await?;

        let now = now_str();
        let new_version = existing_persona.map(|p| p.version + 1).unwrap_or(1);
        let scene_ids: Vec<String> = scenes.iter().map(|s| s.id.clone()).collect();

        let persona = Persona {
            id: existing_persona
                .map(|p| p.id.clone())
                .unwrap_or_else(|| DistillationRepository::generate_id()),
            user_id: user_id.to_string(),
            agent_id: agent_id.map(|s| s.to_string()),
            tenant_id: tenant_id.to_string(),
            content: persona_content.trim().to_string(),
            version: new_version,
            generated_from_scenes: scene_ids,
            created_at: existing_persona
                .map(|p| p.created_at.clone())
                .unwrap_or_else(|| now.clone()),
            updated_at: now,
        };

        info!(
            "L3 persona generated: user={}, version={}, content_len={}",
            user_id, new_version, persona.content.len()
        );

        Ok(persona)
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
                "temperature": 0.5,
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
            return Err(anyhow::anyhow!("LLM persona generation failed: {}", response.status()));
        }

        #[derive(serde::Deserialize)]
        struct OllamaResponse { response: String }

        let resp: OllamaResponse = response.json().await?;
        Ok(resp.response)
    }
}
