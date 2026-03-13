use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PerformancePrediction {
    #[serde(rename = "efficiency_gain")]
    pub efficiency_gain: f64,
    #[serde(rename = "coherence_gain")]
    pub coherence_gain: f64,
    #[serde(rename = "resource_cost")]
    pub resource_cost: f64,
    #[serde(rename = "cost_benefit_ratio")]
    pub cost_benefit_ratio: Option<f64>,
    #[serde(rename = "confidence_score")]
    pub confidence_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PerformanceBaseline {
    #[serde(rename = "efficiency_gain")]
    pub efficiency_gain: f64,
    #[serde(rename = "coherence_gain")]
    pub coherence_gain: f64,
    #[serde(rename = "resource_cost")]
    pub resource_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PerformanceBaselines {
    pub stm: PerformanceBaseline,
    pub ltm: PerformanceBaseline,
    pub kg: PerformanceBaseline,
    pub mm: PerformanceBaseline,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MarginalDecayFactors {
    #[serde(rename = "stm_to_ltm")]
    pub stm_to_ltm: f64,
    #[serde(rename = "ltm_to_kg")]
    pub ltm_to_kg: f64,
    #[serde(rename = "kg_to_mm")]
    pub kg_to_mm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PerformanceBreakdown {
    #[serde(rename = "stm_contribution")]
    pub stm_contribution: f64,
    #[serde(rename = "ltm_contribution")]
    pub ltm_contribution: f64,
    #[serde(rename = "kg_contribution")]
    pub kg_contribution: f64,
    #[serde(rename = "mm_contribution")]
    pub mm_contribution: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PerformanceMetrics {
    #[serde(rename = "efficiency_score")]
    pub efficiency_score: f64,
    #[serde(rename = "coherence_score")]
    pub coherence_score: f64,
    #[serde(rename = "response_time_ms")]
    pub response_time_ms: u64,
    #[serde(rename = "memory_usage_mb")]
    pub memory_usage_mb: u64,
    #[serde(rename = "cpu_usage_percent")]
    pub cpu_usage_percent: u8,
}
