//! Intelligent model routing based on detected hardware capabilities.
//!
//! Given a [`HardwareCapabilities`] snapshot, the router selects the optimal
//! embedding model and LLM from a priority chain:
//!
//! `Apple Silicon (Metal + ANE)` → `CUDA high-VRAM` → `CUDA low-VRAM`
//!   → `CPU (enough RAM)` → `configured endpoint (cloud / remote)`
//!
//! All functions are pure (stateless); pass `hardware_detector::get()` as
//! the first argument.

use serde::Serialize;

use crate::services::hardware_detector::HardwareCapabilities;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Recommended embedding model configuration.
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingRecommendation {
    /// Ollama / OpenAI-compatible model name.
    pub model: String,
    /// Base URL of the inference endpoint.
    pub base_url: String,
    /// Expected embedding dimension (used to size the Qdrant collection).
    pub dimension: usize,
    /// Human-readable explanation of why this config was chosen.
    pub reasoning: String,
}

/// Recommended LLM configuration.
#[derive(Debug, Clone, Serialize)]
pub struct LlmRecommendation {
    /// Ollama / OpenAI-compatible model name.
    pub model: String,
    /// Base URL of the inference endpoint.
    pub base_url: String,
    /// Human-readable explanation of why this config was chosen.
    pub reasoning: String,
}

// ---------------------------------------------------------------------------
// Routing logic
// ---------------------------------------------------------------------------

/// Recommend the best embedding model for the detected hardware.
///
/// `configured_base_url` is the base URL already set in `EmbeddingConfig`;
/// the recommendation reuses it so network topology is preserved.
pub fn recommend_embedding(
    caps: &HardwareCapabilities,
    configured_base_url: &str,
) -> EmbeddingRecommendation {
    if caps.is_apple_silicon {
        // Apple Silicon: Ollama uses Metal + Apple Neural Engine automatically.
        EmbeddingRecommendation {
            model: "nomic-embed-text".to_string(),
            base_url: configured_base_url.to_string(),
            dimension: 768,
            reasoning: "Apple Silicon detected: Ollama will use Metal + ANE acceleration; \
                        nomic-embed-text offers the best latency / quality trade-off"
                .to_string(),
        }
    } else if caps.has_cuda && caps.cuda_total_vram_mb >= 8192 {
        // CUDA GPU with ≥ 8 GB VRAM: use a larger, higher-accuracy model.
        EmbeddingRecommendation {
            model: "mxbai-embed-large".to_string(),
            base_url: configured_base_url.to_string(),
            dimension: 1024,
            reasoning: format!(
                "CUDA GPU with {} MB VRAM: using mxbai-embed-large for higher accuracy (1024-d)",
                caps.cuda_total_vram_mb
            ),
        }
    } else if caps.has_cuda {
        // CUDA GPU with less VRAM: balanced model.
        EmbeddingRecommendation {
            model: "nomic-embed-text".to_string(),
            base_url: configured_base_url.to_string(),
            dimension: 768,
            reasoning: format!(
                "CUDA GPU with {} MB VRAM: nomic-embed-text balances quality and memory usage",
                caps.cuda_total_vram_mb
            ),
        }
    } else if caps.total_ram_mb >= 8192 {
        // CPU-only but enough RAM to run Ollama locally.
        EmbeddingRecommendation {
            model: "nomic-embed-text".to_string(),
            base_url: configured_base_url.to_string(),
            dimension: 768,
            reasoning: format!(
                "CPU-only with {} MB RAM: sufficient for nomic-embed-text via Ollama",
                caps.total_ram_mb
            ),
        }
    } else {
        // Low memory: keep whatever the operator configured (remote/cloud endpoint).
        EmbeddingRecommendation {
            model: "nomic-embed-text".to_string(),
            base_url: configured_base_url.to_string(),
            dimension: 768,
            reasoning: format!(
                "Limited hardware ({} MB RAM, no GPU): using configured endpoint as-is",
                caps.total_ram_mb
            ),
        }
    }
}

/// Recommend the best LLM for the detected hardware.
pub fn recommend_llm(
    caps: &HardwareCapabilities,
    configured_base_url: &str,
) -> LlmRecommendation {
    if caps.is_apple_silicon {
        LlmRecommendation {
            model: "llama3.2".to_string(),
            base_url: configured_base_url.to_string(),
            reasoning: "Apple Silicon: llama3.2 via Ollama with Metal/ANE acceleration"
                .to_string(),
        }
    } else if caps.has_cuda && caps.cuda_total_vram_mb >= 16384 {
        LlmRecommendation {
            model: "llama3.1:8b".to_string(),
            base_url: configured_base_url.to_string(),
            reasoning: format!(
                "High-VRAM CUDA GPU ({} MB): full Llama 3.1 8B via Ollama",
                caps.cuda_total_vram_mb
            ),
        }
    } else if caps.has_cuda && caps.cuda_total_vram_mb >= 6144 {
        LlmRecommendation {
            model: "llama3.2:3b".to_string(),
            base_url: configured_base_url.to_string(),
            reasoning: format!(
                "Mid-range CUDA GPU ({} MB VRAM): Llama 3.2 3B via Ollama",
                caps.cuda_total_vram_mb
            ),
        }
    } else if caps.has_cuda {
        LlmRecommendation {
            model: "llama3.2:1b".to_string(),
            base_url: configured_base_url.to_string(),
            reasoning: format!(
                "Low-VRAM CUDA GPU ({} MB): compact Llama 3.2 1B to fit in VRAM",
                caps.cuda_total_vram_mb
            ),
        }
    } else if caps.total_ram_mb >= 8192 {
        LlmRecommendation {
            model: "llama3.2:3b".to_string(),
            base_url: configured_base_url.to_string(),
            reasoning: format!(
                "CPU-only with {} MB RAM: Llama 3.2 3B for balanced quality",
                caps.total_ram_mb
            ),
        }
    } else {
        LlmRecommendation {
            model: "llama3.2:1b".to_string(),
            base_url: configured_base_url.to_string(),
            reasoning: format!(
                "Limited hardware ({} MB RAM): smallest model to reduce memory pressure",
                caps.total_ram_mb
            ),
        }
    }
}
