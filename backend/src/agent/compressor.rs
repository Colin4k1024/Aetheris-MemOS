//! Memory Compressor - Compress STM to LTM
//!
//! This module handles automatic memory compression from short-term to long-term memory.

use crate::kernel::types::*;
use crate::kernel::error::MemoryResult;

/// Memory compressor that compresses STM entries for LTM storage.
pub struct MemoryCompressor {
    batch_size: usize,
}

impl MemoryCompressor {
    pub fn new(batch_size: usize) -> Self {
        Self { batch_size }
    }

    /// Compress a batch of memories.
    /// 
    /// In a production system, this would use:
    /// - LLM-based summarization
    /// - Key phrase extraction
    /// - Semantic compression
    pub async fn compress(&self, entries: &[MemoryEntry]) -> Vec<CompressedMemory> {
        let mut compressed = Vec::new();

        // Process in batches
        for chunk in entries.chunks(self.batch_size) {
            for entry in chunk {
                // In production: use LLM to generate summary
                let summary = self.generate_summary(entry).await;
                
                compressed.push(CompressedMemory {
                    original_id: entry.id.clone(),
                    summary,
                    key_phrases: self.extract_key_phrases(entry),
                    importance: entry.metadata.importance,
                    source_layer: entry.layer,
                });
            }
        }

        compressed
    }

    /// Generate a summary of the memory entry.
    async fn generate_summary(&self, entry: &MemoryEntry) -> String {
        match &entry.content {
            MemoryContent::Text(text) => {
                // Simple extraction: take first N characters as summary
                // In production: use LLM for intelligent summarization
                if text.len() > 200 {
                    format!("{}...", &text[..200])
                } else {
                    text.clone()
                }
            }
            MemoryContent::Json(json) => {
                serde_json::to_string(json).unwrap_or_default()
            }
            MemoryContent::Binary(_) => "[Binary data]".to_string(),
            MemoryContent::Graph(data) => {
                // Summarize graph as text
                let node_count = data.nodes.len();
                let edge_count = data.edges.len();
                format!("Graph with {} nodes and {} edges", node_count, edge_count)
            }
        }
    }

    /// Extract key phrases from the memory entry.
    fn extract_key_phrases(&self, entry: &MemoryEntry) -> Vec<String> {
        let text = match &entry.content {
            MemoryContent::Text(s) => s.clone(),
            _ => return vec![],
        };

        // Simple keyword extraction
        // In production: use NLP for key phrase extraction
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut phrases = Vec::new();

        // Extract 2-3 word phrases
        for i in 0..words.len().saturating_sub(1) {
            if words[i].len() > 3 && words[i + 1].len() > 3 {
                phrases.push(format!("{} {}", words[i], words[i + 1]));
            }
        }

        phrases.truncate(10); // Limit to top 10 phrases
        phrases
    }
}

/// Compressed memory representation.
#[derive(Debug, Clone)]
pub struct CompressedMemory {
    pub original_id: MemoryId,
    pub summary: String,
    pub key_phrases: Vec<String>,
    pub importance: f64,
    pub source_layer: LayerType,
}
