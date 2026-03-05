use tracing::{error, info};
use ulid::Ulid;

use crate::db::pool;
use crate::models::*;
use crate::AppError;

pub struct PerformanceMetricsRepository;

impl PerformanceMetricsRepository {
    /// 创建性能指标记录
    pub async fn create(
        session_id: Option<&str>,
        config_id: &str,
        metrics: &PerformanceMetrics,
    ) -> Result<String, AppError> {
        let metric_id = Ulid::new().to_string();
        let pool = pool();

        sqlx::query(
            r#"
            INSERT INTO performance_metrics (
                metric_id, session_id, config_id,
                response_time_ms, memory_usage_mb, cpu_usage_percent,
                stm_usage_count, ltm_usage_count, kg_usage_count, mm_usage_count,
                accuracy_score, coherence_score, user_satisfaction,
                error_count
            ) VALUES (
                $1, $2, $3,
                $4, $5, $6,
                $7, $8, $9, $10,
                $11, $12, $13,
                $14
            )
            "#,
        )
        .bind(&metric_id)
        .bind(session_id)
        .bind(config_id)
        .bind(metrics.response_time_ms as i32)
        .bind(metrics.memory_usage_mb as f64)
        .bind(metrics.cpu_usage_percent as f64)
        .bind(0i32)
        .bind(0i32)
        .bind(0i32)
        .bind(0i32)
        .bind(Some(metrics.efficiency_score))
        .bind(Some(metrics.coherence_score))
        .bind(None::<f64>)
        .bind(0i32)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create performance metric: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Created performance metric: {}", metric_id);
        Ok(metric_id)
    }

    /// 根据 metric_id 获取指标
    pub async fn get_by_id(metric_id: &str) -> Result<Option<PerformanceMetricRow>, AppError> {
        let pool = pool();

        let row = sqlx::query_as::<_, PerformanceMetricRow>(
            r#"
            SELECT 
                metric_id, session_id, config_id, timestamp, date_hour,
                response_time_ms, memory_usage_mb, cpu_usage_percent,
                stm_usage_count, ltm_usage_count, kg_usage_count, mm_usage_count,
                accuracy_score, coherence_score, user_satisfaction,
                error_count, error_types
            FROM performance_metrics
            WHERE metric_id = $1
            "#,
        )
        .bind(metric_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to get performance metric: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(row)
    }

    /// 根据 config_id 和时间范围获取指标
    pub async fn get_by_config_and_time_range(
        config_id: &str,
        start_time: Option<&str>,
        end_time: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<PerformanceMetricRow>, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(100);

        let rows = if let (Some(start), Some(end)) = (start_time, end_time) {
            sqlx::query_as::<_, PerformanceMetricRow>(
                r#"
                SELECT 
                    metric_id, session_id, config_id, timestamp, date_hour,
                    response_time_ms, memory_usage_mb, cpu_usage_percent,
                    stm_usage_count, ltm_usage_count, kg_usage_count, mm_usage_count,
                    accuracy_score, coherence_score, user_satisfaction,
                    error_count, error_types
                FROM performance_metrics
                WHERE config_id = $1 AND timestamp >= $2::timestamptz AND timestamp <= $3::timestamptz
                ORDER BY timestamp DESC
                LIMIT $4
                "#,
            )
            .bind(config_id)
            .bind(start)
            .bind(end)
            .bind(limit)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_as::<_, PerformanceMetricRow>(
                r#"
                SELECT 
                    metric_id, session_id, config_id, timestamp, date_hour,
                    response_time_ms, memory_usage_mb, cpu_usage_percent,
                    stm_usage_count, ltm_usage_count, kg_usage_count, mm_usage_count,
                    accuracy_score, coherence_score, user_satisfaction,
                    error_count, error_types
                FROM performance_metrics
                WHERE config_id = $1
                ORDER BY timestamp DESC
                LIMIT $2
                "#,
            )
            .bind(config_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        .map_err(|e| {
            error!("Failed to get performance metrics: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(rows)
    }

    /// 获取聚合统计数据
    pub async fn get_aggregated_stats(
        config_id: &str,
        start_time: Option<&str>,
        end_time: Option<&str>,
    ) -> Result<AggregatedStats, AppError> {
        let pool = pool();

        let row = if let (Some(start), Some(end)) = (start_time, end_time) {
            sqlx::query_as::<_, AggregatedStatsRow>(
                r#"
                SELECT 
                    AVG(response_time_ms) as avg_response_time,
                    AVG(memory_usage_mb) as avg_memory_usage,
                    AVG(cpu_usage_percent) as avg_cpu_usage,
                    AVG(accuracy_score) as avg_accuracy,
                    AVG(coherence_score) as avg_coherence,
                    MAX(response_time_ms) as max_response_time,
                    MAX(memory_usage_mb) as max_memory_usage,
                    MAX(cpu_usage_percent) as max_cpu_usage,
                    MIN(response_time_ms) as min_response_time,
                    MIN(memory_usage_mb) as min_memory_usage,
                    MIN(cpu_usage_percent) as min_cpu_usage,
                    COUNT(*) as count
                FROM performance_metrics
                WHERE config_id = $1 AND timestamp >= $2::timestamptz AND timestamp <= $3::timestamptz
                "#,
            )
            .bind(config_id)
            .bind(start)
            .bind(end)
            .fetch_one(pool)
            .await
        } else {
            sqlx::query_as::<_, AggregatedStatsRow>(
                r#"
                SELECT 
                    AVG(response_time_ms) as avg_response_time,
                    AVG(memory_usage_mb) as avg_memory_usage,
                    AVG(cpu_usage_percent) as avg_cpu_usage,
                    AVG(accuracy_score) as avg_accuracy,
                    AVG(coherence_score) as avg_coherence,
                    MAX(response_time_ms) as max_response_time,
                    MAX(memory_usage_mb) as max_memory_usage,
                    MAX(cpu_usage_percent) as max_cpu_usage,
                    MIN(response_time_ms) as min_response_time,
                    MIN(memory_usage_mb) as min_memory_usage,
                    MIN(cpu_usage_percent) as min_cpu_usage,
                    COUNT(*) as count
                FROM performance_metrics
                WHERE config_id = $1
                "#,
            )
            .bind(config_id)
            .fetch_one(pool)
            .await
        }
        .map_err(|e| {
            error!("Failed to get aggregated stats: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(AggregatedStats {
            avg_response_time: row.avg_response_time.map(|v| v as u64),
            avg_memory_usage: row.avg_memory_usage.map(|v| v as u64),
            avg_cpu_usage: row.avg_cpu_usage.map(|v| v as u8),
            avg_accuracy: row.avg_accuracy,
            avg_coherence: row.avg_coherence,
            max_response_time: row.max_response_time.map(|v| v as u64),
            max_memory_usage: row.max_memory_usage.map(|v| v as u64),
            max_cpu_usage: row.max_cpu_usage.map(|v| v as u8),
            min_response_time: row.min_response_time.map(|v| v as u64),
            min_memory_usage: row.min_memory_usage.map(|v| v as u64),
            min_cpu_usage: row.min_cpu_usage.map(|v| v as u8),
            count: row.count.unwrap_or(0) as usize,
        })
    }

    /// 将数据库行转换为 PerformanceMetrics
    pub fn row_to_performance_metrics(row: &PerformanceMetricRow) -> PerformanceMetrics {
        PerformanceMetrics {
            efficiency_score: row.accuracy_score.unwrap_or(0.0),
            coherence_score: row.coherence_score.unwrap_or(0.0),
            response_time_ms: row.response_time_ms.map(|v| v as u64).unwrap_or(0),
            memory_usage_mb: row.memory_usage_mb.map(|v| v as u64).unwrap_or(0),
            cpu_usage_percent: row.cpu_usage_percent.map(|v| v as u8).unwrap_or(0),
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct PerformanceMetricRow {
    pub metric_id: String,
    pub session_id: Option<String>,
    pub config_id: String,
    pub timestamp: String,
    pub date_hour: Option<String>,
    pub response_time_ms: Option<i32>,
    pub memory_usage_mb: Option<f64>,
    pub cpu_usage_percent: Option<f64>,
    pub stm_usage_count: i32,
    pub ltm_usage_count: i32,
    pub kg_usage_count: i32,
    pub mm_usage_count: i32,
    pub accuracy_score: Option<f64>,
    pub coherence_score: Option<f64>,
    pub user_satisfaction: Option<f64>,
    pub error_count: i32,
    pub error_types: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct AggregatedStatsRow {
    avg_response_time: Option<f64>,
    avg_memory_usage: Option<f64>,
    avg_cpu_usage: Option<f64>,
    avg_accuracy: Option<f64>,
    avg_coherence: Option<f64>,
    max_response_time: Option<i32>,
    max_memory_usage: Option<f64>,
    max_cpu_usage: Option<f64>,
    min_response_time: Option<i32>,
    min_memory_usage: Option<f64>,
    min_cpu_usage: Option<f64>,
    count: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AggregatedStats {
    pub avg_response_time: Option<u64>,
    pub avg_memory_usage: Option<u64>,
    pub avg_cpu_usage: Option<u8>,
    pub avg_accuracy: Option<f64>,
    pub avg_coherence: Option<f64>,
    pub max_response_time: Option<u64>,
    pub max_memory_usage: Option<u64>,
    pub max_cpu_usage: Option<u8>,
    pub min_response_time: Option<u64>,
    pub min_memory_usage: Option<u64>,
    pub min_cpu_usage: Option<u8>,
    pub count: usize,
}
