use anyhow::Result;
use tracing::{info, warn};

use super::prompts;
use super::types::*;

pub struct L1Deduplicator {
    llm_base_url: String,
    llm_model: String,
    top_k: usize,
}

impl L1Deduplicator {
    pub fn new(llm_base_url: &str, llm_model: &str, top_k: usize) -> Self {
        Self {
            llm_base_url: llm_base_url.to_string(),
            llm_model: llm_model.to_string(),
            top_k,
        }
    }

    pub async fn deduplicate(
        &self,
        new_atoms: &[MemoryAtom],
        existing_atoms: &[MemoryAtom],
    ) -> Result<Vec<DedupResult>> {
        if new_atoms.is_empty() {
            return Ok(vec![]);
        }

        if existing_atoms.is_empty() {
            return Ok(new_atoms.iter().map(|_| DedupResult {
                decision: DedupDecision::Keep,
                existing_id: None,
                merged_content: None,
            }).collect());
        }

        let new_memories_text = new_atoms.iter()
            .enumerate()
            .map(|(i, a)| format!("[New-{}] (type: {}, priority: {}) {}", i, a.atom_type, a.priority, a.content))
            .collect::<Vec<_>>()
            .join("\n");

        let existing_text = existing_atoms.iter()
            .map(|a| format!("[{}] (type: {}, priority: {}) {}", a.id, a.atom_type, a.priority, a.content))
            .collect::<Vec<_>>()
            .join("\n");

        let user_prompt = prompts::format_l1_dedup_user_prompt(&new_memories_text, &existing_text);

        let response = self.call_llm(
            prompts::L1_DEDUP_SYSTEM_PROMPT,
            &user_prompt,
        ).await?;

        let results = parse_dedup_response(&response, new_atoms.len())?;

        let kept = results.iter().filter(|r| r.decision == DedupDecision::Keep).count();
        let merged = results.iter().filter(|r| r.decision == DedupDecision::Merge).count();
        let superseded = results.iter().filter(|r| r.decision == DedupDecision::Supersede).count();
        let discarded = results.iter().filter(|r| r.decision == DedupDecision::Discard).count();

        info!(
            "Dedup results: keep={}, merge={}, supersede={}, discard={}",
            kept, merged, superseded, discarded
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
                "temperature": 0.1,
                "num_predict": 2048
            }
        });

        let response = client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("LLM dedup call failed: {}", response.status()));
        }

        #[derive(serde::Deserialize)]
        struct OllamaResponse { response: String }

        let resp: OllamaResponse = response.json().await?;
        Ok(resp.response)
    }
}

fn parse_dedup_response(response: &str, expected_count: usize) -> Result<Vec<DedupResult>> {
    let trimmed = response.trim();
    let json_start = trimmed.find('[').unwrap_or(0);
    let json_end = trimmed.rfind(']').map(|i| i + 1).unwrap_or(trimmed.len());
    let json_text = &trimmed[json_start..json_end];

    let parsed: Vec<DedupResult> = serde_json::from_str(json_text)
        .unwrap_or_else(|e| {
            warn!("Failed to parse dedup response: {}, defaulting to keep all", e);
            (0..expected_count).map(|_| DedupResult {
                decision: DedupDecision::Keep,
                existing_id: None,
                merged_content: None,
            }).collect()
        });

    if parsed.len() != expected_count {
        warn!(
            "Dedup response count mismatch: expected {}, got {}. Padding with keep.",
            expected_count, parsed.len()
        );
        let mut results = parsed;
        while results.len() < expected_count {
            results.push(DedupResult {
                decision: DedupDecision::Keep,
                existing_id: None,
                merged_content: None,
            });
        }
        results.truncate(expected_count);
        return Ok(results);
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dedup_response() {
        let response = r#"[
            {"decision": "keep", "existing_id": null, "merged_content": null},
            {"decision": "supersede", "existing_id": "atom-123", "merged_content": null},
            {"decision": "discard", "existing_id": "atom-456", "merged_content": null}
        ]"#;

        let results = parse_dedup_response(response, 3).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].decision, DedupDecision::Keep);
        assert_eq!(results[1].decision, DedupDecision::Supersede);
        assert_eq!(results[1].existing_id, Some("atom-123".to_string()));
        assert_eq!(results[2].decision, DedupDecision::Discard);
    }
}
