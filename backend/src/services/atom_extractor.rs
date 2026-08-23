use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::db::stm::SessionMessage;
use crate::services::llm::get_llm_service;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedAtom {
    pub atom_type: String,
    pub scene_name: String,
    pub content: String,
    pub priority: f32,
    pub source_message_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub atoms: Vec<ExtractedAtom>,
    pub scene_segments: Vec<SceneSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneSegment {
    pub scene_name: String,
    pub message_range: (usize, usize),
    pub summary: String,
}

pub struct AtomExtractor;

impl AtomExtractor {
    pub async fn extract_from_messages(messages: &[SessionMessage]) -> Result<ExtractionResult> {
        if messages.is_empty() {
            return Ok(ExtractionResult::default());
        }

        info!("Extracting atoms from {} messages", messages.len());

        let prompt = Self::build_extraction_prompt(messages);
        let llm = get_llm_service()?;
        let response_text = llm.call_llm_public(&prompt).await?;

        info!(
            "LLM extraction response received, length={}",
            response_text.len()
        );

        let result = Self::parse_extraction_response(&response_text)?;
        info!(
            "Extracted {} atoms across {} scenes",
            result.atoms.len(),
            result.scene_segments.len()
        );

        Ok(result)
    }

    fn build_extraction_prompt(messages: &[SessionMessage]) -> String {
        let mut conversation = String::new();
        for (i, msg) in messages.iter().enumerate() {
            conversation.push_str(&format!("[{}] {}: {}\n", i, msg.role, msg.content));
        }

        format!(
            r#"You are a memory extraction specialist. Analyze the following conversation and extract structured memory atoms.

## Task
1. **Scene Segmentation**: Divide the conversation into topical scenes (coherent discussion segments about one topic/task).
2. **Memory Extraction**: From each scene, extract memory atoms of three types:
   - `persona`: User attributes, preferences, style, expertise, habits
   - `episodic`: Events, decisions, outcomes, actions taken
   - `instruction`: Long-term behavioral rules for the AI assistant

## Output Format
Return ONLY valid JSON in this exact structure:
{{
  "scene_segments": [
    {{
      "scene_name": "short_snake_case_topic_name",
      "message_range": [start_index, end_index],
      "summary": "One sentence summary of the scene"
    }}
  ],
  "atoms": [
    {{
      "atom_type": "persona|episodic|instruction",
      "scene_name": "matching_scene_name",
      "content": "Clear, concise statement of the memory (1-3 sentences)",
      "priority": 0.1-1.0,
      "source_message_ids": ["msg_id_1", "msg_id_2"]
    }}
  ]
}}

## Rules
- Each atom should be self-contained and useful without context
- Priority: 1.0 = critical/permanent info, 0.5 = useful context, 0.1 = minor detail
- Maximum 20 atoms per conversation
- Scene names must be unique, lowercase, snake_case
- source_message_ids should reference the [index] numbers from the conversation

## Conversation
{}"#,
            conversation
        )
    }

    fn parse_extraction_response(response: &str) -> Result<ExtractionResult> {
        // Try direct parse first
        if let Ok(result) = serde_json::from_str::<ExtractionResult>(response) {
            return Ok(result);
        }

        // Try extracting JSON from text (LLM might wrap it in markdown)
        let start = response.find('{').unwrap_or(0);
        let end = response.rfind('}').map(|i| i + 1).unwrap_or(response.len());
        let json_text = &response[start..end];

        match serde_json::from_str::<ExtractionResult>(json_text) {
            Ok(result) => Ok(result),
            Err(e) => {
                warn!("Failed to parse extraction response: {}", e);
                warn!(
                    "Response (first 500 chars): {}",
                    &response[..response.len().min(500)]
                );
                // Return empty result rather than failing entirely
                Ok(ExtractionResult::default())
            }
        }
    }
}
