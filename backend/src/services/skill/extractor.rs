use anyhow::Result;
use tracing::{info, warn};

use super::types::*;

/// Extract reusable Skill candidates from an execution transcript via the LLM.
/// Candidates are **suggestions only** — the caller reviews them and publishes
/// the chosen ones via `POST /v1/skills` (they are NOT auto-published). #90.
pub struct SkillExtractor;

impl SkillExtractor {
    pub fn new() -> Self {
        Self
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

        // Use the unified LLM service (Ollama + OpenAI-compatible), not a
        // raw reqwest call — consistent with the distillation L2/L3 ports
        // (#101/#102) and correct for OpenAI-compatible deployments.
        let full_prompt = format!("{}\n\n{}", system_prompt, user_prompt);
        let llm = crate::services::llm::get_llm_service()
            .map_err(|e| anyhow::anyhow!("LLM service unavailable: {e}"))?;
        let response = llm
            .call_llm_public(&full_prompt)
            .await
            .map_err(|e| anyhow::anyhow!("Skill extraction LLM call failed: {e}"))?;

        let skills = parse_skill_response(&response)?;
        info!("Extracted {} skill candidates from transcript", skills.len());
        Ok(skills)
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
