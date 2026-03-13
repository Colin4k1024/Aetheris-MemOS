use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResourceStatus {
    #[serde(rename = "memory_usage_mb")]
    pub memory_usage_mb: u64,
    #[serde(rename = "memory_usage_percent")]
    pub memory_usage_percent: u8,
    #[serde(rename = "cpu_usage_percent")]
    pub cpu_usage_percent: u8,
    #[serde(rename = "response_time_ms")]
    pub response_time_ms: u64,
    #[serde(rename = "storage_usage_percent")]
    pub storage_usage_percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResourceLimits {
    #[serde(rename = "memory_limit_mb")]
    pub memory_limit_mb: u64,
    #[serde(rename = "cpu_limit_percent")]
    pub cpu_limit_percent: u8,
    #[serde(rename = "response_time_limit_ms")]
    pub response_time_limit_ms: u64,
    #[serde(rename = "storage_limit_percent")]
    pub storage_limit_percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CurrentResourceStatus {
    #[serde(rename = "current_status")]
    pub current_status: ResourceStatus,
    #[serde(rename = "resource_limits")]
    pub resource_limits: ResourceLimits,
    pub status: String,
    pub alerts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResourceRequirements {
    #[serde(rename = "estimated_memory_mb")]
    pub estimated_memory_mb: u64,
    #[serde(rename = "estimated_cpu_percent")]
    pub estimated_cpu_percent: u8,
    #[serde(rename = "estimated_response_time_ms")]
    pub estimated_response_time_ms: u64,
}
