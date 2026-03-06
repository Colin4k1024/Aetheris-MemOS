#![allow(dead_code)]

use crate::db::performance::PerformanceMetricsRepository;
use crate::models::*;
use sysinfo::{CpuExt, DiskExt, System, SystemExt};
use tracing::info;

pub struct ResourceMonitor {
    resource_limits: ResourceLimits,
}

impl ResourceMonitor {
    pub fn new() -> Self {
        Self {
            resource_limits: ResourceLimits {
                memory_limit_mb: 1024,
                cpu_limit_percent: 80,
                response_time_limit_ms: 2000,
                storage_limit_percent: 90,
            },
        }
    }

    pub async fn get_current_status(&self) -> CurrentResourceStatus {
        // 使用sysinfo库获取真实的系统资源监控数据
        let mut sys = System::new_all();
        sys.refresh_all();

        // 获取内存使用情况
        let total_memory = sys.total_memory() / 1024 / 1024; // MB
        let used_memory = sys.used_memory() / 1024 / 1024; // MB
        let memory_usage_percent = (used_memory as f64 / total_memory as f64 * 100.0) as u8;

        // 获取CPU使用情况
        let cpu_usage_percent = sys.global_cpu_info().cpu_usage() as u8;

        // 获取存储使用情况
        let mut storage_usage_percent = 0;
        if let Some(disk) = sys.disks().first() {
            let total_space = disk.total_space() / 1024 / 1024 / 1024; // GB
            let available_space = disk.available_space() / 1024 / 1024 / 1024; // GB
            if total_space > 0 {
                storage_usage_percent =
                    ((total_space - available_space) as f64 / total_space as f64 * 100.0) as u8;
            }
        }

        // 响应时间暂时使用模拟数据，实际应从请求处理中获取
        let response_time_ms = 850;

        let current_status = ResourceStatus {
            memory_usage_mb: used_memory,
            memory_usage_percent: memory_usage_percent,
            cpu_usage_percent: cpu_usage_percent,
            response_time_ms: response_time_ms as u64,
            storage_usage_percent: storage_usage_percent,
        };

        // 确定状态级别
        let status = if current_status.memory_usage_percent > 80
            || current_status.cpu_usage_percent > 80
        {
            "critical"
        } else if current_status.memory_usage_percent > 60 || current_status.cpu_usage_percent > 60
        {
            "warning"
        } else {
            "healthy"
        };

        // 生成告警
        let mut alerts = Vec::new();
        if current_status.memory_usage_percent > 80 {
            alerts.push(format!(
                "内存使用率过高: {}%",
                current_status.memory_usage_percent
            ));
        }
        if current_status.cpu_usage_percent > 80 {
            alerts.push(format!(
                "CPU 使用率过高: {}%",
                current_status.cpu_usage_percent
            ));
        }
        if current_status.storage_usage_percent > 90 {
            alerts.push(format!(
                "存储使用率过高: {}%",
                current_status.storage_usage_percent
            ));
        }
        if current_status.response_time_ms > self.resource_limits.response_time_limit_ms {
            alerts.push(format!(
                "响应时间过长: {}ms",
                current_status.response_time_ms
            ));
        }

        CurrentResourceStatus {
            current_status,
            resource_limits: self.resource_limits.clone(),
            status: status.to_string(),
            alerts,
        }
    }

    /// 保存性能指标到数据库
    pub async fn save_performance_metrics(
        &self,
        session_id: Option<&str>,
        config_id: &str,
        metrics: &PerformanceMetrics,
    ) -> Result<String, crate::AppError> {
        let metric_id =
            PerformanceMetricsRepository::create(session_id, config_id, metrics).await?;
        info!("Saved performance metrics: {}", metric_id);
        Ok(metric_id)
    }

    pub fn calculate_cost_benefit_ratio(
        &self,
        performance_prediction: &PerformancePrediction,
        resource_status: &ResourceStatus,
    ) -> f64 {
        // 计算性能得分
        let performance_score = performance_prediction.efficiency_gain * 0.6
            + performance_prediction.coherence_gain * 0.4;

        // 计算资源成本
        let resource_cost = self.calculate_resource_cost(resource_status);

        // 计算成本效益比
        if resource_cost > 0.0 {
            performance_score / resource_cost
        } else {
            f64::INFINITY
        }
    }

    fn calculate_resource_cost(&self, resource_status: &ResourceStatus) -> f64 {
        let memory_cost = (resource_status.memory_usage_percent as f64 / 100.0) * 0.4;
        let cpu_cost = (resource_status.cpu_usage_percent as f64 / 100.0) * 0.4;
        let response_cost = (resource_status.response_time_ms as f64 / 2000.0) * 0.2;

        (memory_cost + cpu_cost + response_cost).min(1.0)
    }

    pub fn get_resource_limits(&self) -> &ResourceLimits {
        &self.resource_limits
    }

    pub fn optimize_config(
        &self,
        current_config: &MemoryConfig,
        _performance_goals: &PerformanceGoals,
    ) -> OptimizationResult {
        let mut suggestions = Vec::new();
        let mut optimized_weights = current_config.memory_weights.clone();

        // 如果 LTM 权重过高且资源紧张，建议降低
        if current_config.memory_weights.ltm > 0.8 {
            suggestions.push(OptimizationSuggestion {
                suggestion_type: "weight_adjustment".to_string(),
                description: format!(
                    "Reduce LTM weight from {:.1} to {:.1}",
                    current_config.memory_weights.ltm,
                    current_config.memory_weights.ltm * 0.75
                ),
                expected_improvement: 0.15,
                risk_level: "low".to_string(),
            });
            optimized_weights.ltm *= 0.75;
        }

        // 如果 KG 权重高但任务简单，建议禁用
        if current_config.memory_weights.kg > 0.5 && current_config.reasoning_depth == "shallow" {
            suggestions.push(OptimizationSuggestion {
                suggestion_type: "memory_disable".to_string(),
                description: "Disable KG memory for simple tasks".to_string(),
                expected_improvement: 0.25,
                risk_level: "medium".to_string(),
            });
            optimized_weights.kg = 0.0;
        }

        let optimized_config = MemoryConfig {
            primary_memory: current_config.primary_memory.clone(),
            secondary_memory: current_config.secondary_memory.clone(),
            memory_weights: optimized_weights,
            reasoning_depth: current_config.reasoning_depth.clone(),
            enable_multimodal: current_config.enable_multimodal,
        };

        OptimizationResult {
            optimization_suggestions: suggestions,
            optimized_config,
            predicted_improvement: PredictedImprovement {
                efficiency_gain: 0.05,
                coherence_gain: 0.02,
                resource_cost_reduction: 0.2,
            },
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, salvo::oapi::ToSchema)]
pub struct PerformanceGoals {
    #[serde(rename = "target_efficiency")]
    pub target_efficiency: f64,
    #[serde(rename = "target_coherence")]
    pub target_coherence: f64,
    #[serde(rename = "max_resource_cost")]
    pub max_resource_cost: f64,
}

#[derive(Debug, Clone, serde::Serialize, salvo::oapi::ToSchema)]
pub struct OptimizationSuggestion {
    #[serde(rename = "type")]
    pub suggestion_type: String,
    pub description: String,
    #[serde(rename = "expected_improvement")]
    pub expected_improvement: f64,
    #[serde(rename = "risk_level")]
    pub risk_level: String,
}

#[derive(Debug, Clone, serde::Serialize, salvo::oapi::ToSchema)]
pub struct OptimizationResult {
    #[serde(rename = "optimization_suggestions")]
    pub optimization_suggestions: Vec<OptimizationSuggestion>,
    #[serde(rename = "optimized_config")]
    pub optimized_config: MemoryConfig,
    #[serde(rename = "predicted_improvement")]
    pub predicted_improvement: PredictedImprovement,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_creation() {
        let monitor = ResourceMonitor::new();
        assert_eq!(monitor.resource_limits.memory_limit_mb, 1024);
        assert_eq!(monitor.resource_limits.cpu_limit_percent, 80);
    }

    #[test]
    fn test_cost_benefit_calculation() {
        let monitor = ResourceMonitor::new();
        let performance_prediction = PerformancePrediction {
            efficiency_gain: 0.8,
            coherence_gain: 1.5,
            resource_cost: 0.5,
            cost_benefit_ratio: Some(1.5),
            confidence_score: Some(0.9),
        };
        let resource_status = ResourceStatus {
            memory_usage_mb: 512,
            memory_usage_percent: 50,
            cpu_usage_percent: 45,
            response_time_ms: 850,
            storage_usage_percent: 40,
        };

        let result = monitor.calculate_cost_benefit_ratio(&performance_prediction, &resource_status);
        assert!(result > 0.0);
    }
}

#[derive(Debug, Clone, serde::Serialize, salvo::oapi::ToSchema)]
pub struct PredictedImprovement {
    #[serde(rename = "efficiency_gain")]
    pub efficiency_gain: f64,
    #[serde(rename = "coherence_gain")]
    pub coherence_gain: f64,
    #[serde(rename = "resource_cost_reduction")]
    pub resource_cost_reduction: f64,
}
