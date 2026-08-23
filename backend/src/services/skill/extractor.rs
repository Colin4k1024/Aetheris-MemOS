use anyhow::Result;
use tracing::{info, warn};

use super::types::*;

pub struct SkillExtractor {
    llm_base_url: String,
    llm_model: String,
}

impl SkillExtractor {
    pub fn new(llm_base_url: &str, llm_model: &str) -> Self {
        Self {
            llm_base_url: llm_base_url.to_string(),
            llm_model: llm_model.to_string(),
        }
    }

    pub async fn extract_from_transcript(
        &self,
        transcript: &str,
        reason: Option<&str>,
    ) -> Result<Vec<SkillCreateRequest>> {
        let system_prompt = SKILL_EXTRACTION_PROMPT;

        let user_prompt = if let Some(reason) = reason {
            format!("## 提取提示\n{}\n\n---\n\n{}", reason, transcript)
        } else {
            transcript.to_string()
        };

        let response = self.call_llm(system_prompt, &user_prompt).await?;
        let skills = parse_skill_response(&response)?;

        info!("Extracted {} skill candidates from transcript", skills.len());
        Ok(skills)
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
            "options": { "temperature": 0.3, "num_predict": 4096 }
        });

        let response = client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Skill extraction LLM call failed"));
        }

        #[derive(serde::Deserialize)]
        struct OllamaResponse { response: String }

        let resp: OllamaResponse = response.json().await?;
        Ok(resp.response)
    }
}

fn parse_skill_response(response: &str) -> Result<Vec<SkillCreateRequest>> {
    let trimmed = response.trim();
    let json_start = trimmed.find('[').unwrap_or(0);
    let json_end = trimmed.rfind(']').map(|i| i + 1).unwrap_or(trimmed.len());
    let json_text = &trimmed[json_start..json_end];

    serde_json::from_str(json_text).map_err(|e| {
        warn!("Failed to parse skill extraction response: {}", e);
        anyhow::anyhow!("Skill JSON parse error: {}", e)
    })
}

const SKILL_EXTRACTION_PROMPT: &str = r#"你是技能提取专家。从对话记录中识别可复用的工作模式，提取为结构化 Skill。

一个 Skill 包含：
- name: 简短名称
- description: 描述这个技能做什么
- trigger_conditions: 什么情况下应该使用这个技能（字符串数组）
- execution_steps: 执行步骤（对象数组，每个包含 order, description, tool_calls, expected_output）
- validation_rules: 如何验证执行结果正确（字符串数组）
- tags: 标签（字符串数组）

提取原则：
1. 只提取有明确步骤和可重复模式的工作流
2. 忽略一次性操作和简单问答
3. 每个技能必须是自包含、可独立执行的
4. 优先提取高价值、高复用的模式

返回 JSON 数组：
[
  {
    "name": "...",
    "description": "...",
    "trigger_conditions": ["..."],
    "execution_steps": [{"order": 1, "description": "...", "tool_calls": [], "expected_output": null}],
    "validation_rules": ["..."],
    "tags": ["..."]
  }
]

如果没有发现可提取的技能，返回空数组 []。"#;
