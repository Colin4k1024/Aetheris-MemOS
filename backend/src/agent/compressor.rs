//! Memory Compressor - Compress STM to LTM
//!
//! This module handles automatic memory compression from short-term to long-term memory.
//! Uses LLM for intelligent summarization and key phrase extraction.

use crate::kernel::error::MemoryResult;
use crate::kernel::types::*;
use crate::services::llm::LLMService;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, instrument};

/// Memory compressor that compresses STM entries for LTM storage.
pub struct MemoryCompressor {
    batch_size: usize,
    llm_service: Arc<RwLock<Option<LLMService>>>,
}

impl MemoryCompressor {
    /// Create a new MemoryCompressor with the given batch size.
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size,
            llm_service: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the LLM service for the compressor.
    #[instrument(skip(self))]
    pub async fn init_llm(&self) -> Result<()> {
        let mut service = self.llm_service.write().await;
        match LLMService::new() {
            Ok(llm) => {
                info!("LLM service initialized for MemoryCompressor");
                *service = Some(llm);
                Ok(())
            }
            Err(e) => {
                error!("Failed to initialize LLM service: {}", e);
                Err(e)
            }
        }
    }

    /// Check if LLM service is available.
    pub async fn is_llm_available(&self) -> bool {
        self.llm_service.read().await.is_some()
    }

    /// Compress a batch of memories.
    ///
    /// Uses LLM-based summarization when available, falls back to simple extraction.
    #[instrument(skip(self, entries))]
    pub async fn compress(&self, entries: &[MemoryEntry]) -> Vec<CompressedMemory> {
        let mut compressed = Vec::new();

        // Process in batches
        for chunk in entries.chunks(self.batch_size) {
            for entry in chunk {
                let summary = self.generate_summary(entry).await;
                let key_phrases = self.extract_key_phrases(entry).await;

                compressed.push(CompressedMemory {
                    original_id: entry.id.clone(),
                    summary,
                    key_phrases,
                    importance: entry.metadata.importance,
                    source_layer: entry.layer,
                });
            }
        }

        info!(
            "Compressed {} entries into {} compressed memories",
            entries.len(),
            compressed.len()
        );
        compressed
    }

    /// Generate a summary of the memory entry using LLM.
    async fn generate_summary(&self, entry: &MemoryEntry) -> String {
        let text = match &entry.content {
            MemoryContent::Text(s) => s.clone(),
            MemoryContent::Json(json) => serde_json::to_string(json).unwrap_or_default(),
            MemoryContent::Binary(_) => "[Binary data]".to_string(),
            MemoryContent::Graph(data) => {
                // Summarize graph as text
                let node_count = data.nodes.len();
                let edge_count = data.edges.len();
                format!("Graph with {} nodes and {} edges", node_count, edge_count)
            }
        };

        // If text is too short, just return it as-is
        if text.len() < 100 {
            return text;
        }

        // Try to use LLM service
        let service = self.llm_service.read().await;
        if let Some(llm) = service.as_ref() {
            match llm.summarize(&text).await {
                Ok(summary) => {
                    info!("Generated LLM summary, length={}", summary.len());
                    return summary;
                }
                Err(e) => {
                    error!(
                        "LLM summarization failed: {}, falling back to simple extraction",
                        e
                    );
                }
            }
        }
        drop(service);

        // Fallback: simple extraction
        if text.len() > 200 {
            format!("{}...", &text[..200])
        } else {
            text
        }
    }

    /// Extract key phrases from the memory entry using LLM.
    async fn extract_key_phrases(&self, entry: &MemoryEntry) -> Vec<String> {
        let text = match &entry.content {
            MemoryContent::Text(s) => s.clone(),
            _ => return vec![],
        };

        // If text is too short, use simple extraction
        if text.len() < 50 {
            return self.simple_key_phrase_extraction(&text);
        }

        // Try to use LLM service for structured extraction
        let service = self.llm_service.read().await;
        if let Some(llm) = service.as_ref() {
            match llm.summarize_and_extract(&text).await {
                Ok(extraction) => {
                    info!(
                        "Extracted {} entities and {} key facts via LLM",
                        extraction.entities.len(),
                        extraction.key_facts.len()
                    );

                    // Combine entity names and key facts as key phrases
                    let mut phrases: Vec<String> =
                        extraction.entities.iter().map(|e| e.name.clone()).collect();

                    // Add key facts as phrases (truncated)
                    phrases.extend(extraction.key_facts.into_iter().take(5).map(|fact| {
                        if fact.len() > 50 {
                            format!("{}...", &fact[..50])
                        } else {
                            fact
                        }
                    }));

                    phrases.truncate(10);
                    return phrases;
                }
                Err(e) => {
                    error!(
                        "LLM key phrase extraction failed: {}, falling back to simple extraction",
                        e
                    );
                }
            }
        }
        drop(service);

        // Fallback: simple key phrase extraction
        self.simple_key_phrase_extraction(&text)
    }

    /// Simple keyword extraction (fallback when LLM is unavailable).
    fn simple_key_phrase_extraction(&self, text: &str) -> Vec<String> {
        // Common stop words to filter out
        let stop_words = [
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with",
            "by", "from", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
            "do", "does", "did", "will", "would", "could", "should", "may", "might", "must",
            "shall", "can", "need", "this", "that", "these", "those", "it", "its", "they", "them",
            "their", "we", "us", "our", "you", "your", "he", "she", "him", "her", "his",
        ];

        let words: Vec<&str> = text.split_whitespace().collect();
        let mut phrases = Vec::new();

        // Extract 2-3 word phrases (bigrams and trigrams)
        for i in 0..words.len().saturating_sub(1) {
            let w1 = words[i].trim_matches(|c: char| !c.is_alphanumeric());
            let w2 = words[i + 1].trim_matches(|c: char| !c.is_alphanumeric());

            if w1.len() > 3 && w2.len() > 3 {
                let lower_w1 = w1.to_lowercase();
                let lower_w2 = w2.to_lowercase();

                // Skip if either word is a stop word
                if !stop_words.contains(&lower_w1.as_str())
                    && !stop_words.contains(&lower_w2.as_str())
                {
                    phrases.push(format!("{} {}", w1, w2));
                }
            }
        }

        phrases.truncate(10);
        phrases
    }
}

/// Compressed memory representation.
#[derive(Debug, Clone)]
pub struct CompressedMemory {
    /// Original memory entry ID
    pub original_id: MemoryId,
    /// Generated summary
    pub summary: String,
    /// Extracted key phrases
    pub key_phrases: Vec<String>,
    /// Importance score from original entry
    pub importance: f64,
    /// Source layer (STM, LTM, etc.)
    pub source_layer: LayerType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_compressor_creation() {
        let compressor = MemoryCompressor::new(10);
        assert_eq!(compressor.batch_size, 10);
    }

    #[tokio::test]
    async fn test_simple_key_phrase_extraction() {
        let compressor = MemoryCompressor::new(10);
        let text = "The machine learning model processes natural language efficiently";
        let phrases = compressor.simple_key_phrase_extraction(text);

        assert!(!phrases.is_empty());
        assert!(phrases.iter().any(|p| p.contains("machine learning")));
    }

    #[tokio::test]
    async fn test_compress_short_text() {
        let compressor = MemoryCompressor::new(10);

        let entry = MemoryEntry {
            id: MemoryId::new(),
            content: MemoryContent::Text("Short text".to_string()),
            metadata: MemoryMetadata::default(),
            layer: LayerType::Stm,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };

        let result = compressor.compress(&[entry]).await;
        assert_eq!(result.len(), 1);
    }
}
