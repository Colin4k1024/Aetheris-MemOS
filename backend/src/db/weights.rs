#![allow(dead_code)]

use tracing::{error, info};
use ulid::Ulid;

use crate::db::pool;
use crate::models::*;
use crate::services::weight_adjuster::AdjustmentReasons;
use crate::AppError;

pub struct WeightHistoryRepository;

impl WeightHistoryRepository {
    /// 创建权重调整历史记录
    pub async fn create(
        task_id: &str,
        old_weights: &MemoryWeights,
        new_weights: &MemoryWeights,
        adjustment_reasons: &AdjustmentReasons,
        performance_impact: f32,
        strategy_metadata: Option<&str>,
    ) -> Result<String, AppError> {
        let history_id = Ulid::new().to_string();
        let pool = pool();

        // 将 MemoryWeights 序列化为 JSON 字符串
        let old_weights_json = serde_json::to_string(old_weights).map_err(|e| {
            error!("Failed to serialize old weights: {}", e);
            AppError::Internal(format!("Serialization error: {}", e))
        })?;

        let new_weights_json = serde_json::to_string(new_weights).map_err(|e| {
            error!("Failed to serialize new weights: {}", e);
            AppError::Internal(format!("Serialization error: {}", e))
        })?;

        // 合并调整原因
        let reasons_json = serde_json::to_string(adjustment_reasons).map_err(|e| {
            error!("Failed to serialize adjustment reasons: {}", e);
            AppError::Internal(format!("Serialization error: {}", e))
        })?;

        sqlx::query(
            r#"
            INSERT INTO weight_adjustment_history (
                history_id, task_id,
                old_weights_json, new_weights_json,
                adjustment_reasons_json, performance_impact, strategy_metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&history_id)
        .bind(task_id)
        .bind(&old_weights_json)
        .bind(&new_weights_json)
        .bind(&reasons_json)
        .bind(performance_impact)
        .bind(strategy_metadata)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create weight adjustment history: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Created weight adjustment history: {}", history_id);
        Ok(history_id)
    }

    /// 根据时间范围获取权重调整历史
    pub async fn get_by_time_range(
        start_time: Option<&str>,
        end_time: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<WeightHistoryRow>, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(100);

        let rows = if let (Some(start), Some(end)) = (start_time, end_time) {
            sqlx::query_as::<_, WeightHistoryRow>(
                r#"
                SELECT
                    history_id, task_id, timestamp::text as timestamp,
                    old_weights_json, new_weights_json,
                    adjustment_reasons_json, performance_impact,
                    strategy_metadata
                FROM weight_adjustment_history
                WHERE timestamp >= $1::timestamptz AND timestamp <= $2::timestamptz
                ORDER BY timestamp DESC
                LIMIT $3
                "#,
            )
            .bind(start)
            .bind(end)
            .bind(limit)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_as::<_, WeightHistoryRow>(
                r#"
                SELECT
                    history_id, task_id, timestamp::text as timestamp,
                    old_weights_json, new_weights_json,
                    adjustment_reasons_json, performance_impact,
                    strategy_metadata
                FROM weight_adjustment_history
                ORDER BY timestamp DESC
                LIMIT $1
                "#,
            )
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        .map_err(|e| {
            error!("Failed to get weight adjustment history: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(rows)
    }

    /// 根据 task_id 获取权重调整历史
    pub async fn get_by_task_id(
        task_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<WeightHistoryRow>, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(100);

        let rows = sqlx::query_as::<_, WeightHistoryRow>(
            r#"
            SELECT
                history_id, task_id, timestamp::text as timestamp,
                old_weights_json, new_weights_json,
                adjustment_reasons_json, performance_impact,
                strategy_metadata
            FROM weight_adjustment_history
            WHERE task_id = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get weight adjustment history by task_id: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(rows)
    }

    /// 获取统计摘要
    pub async fn get_summary(
        start_time: Option<&str>,
        end_time: Option<&str>,
    ) -> Result<WeightHistorySummary, AppError> {
        let pool = pool();

        let row = if let (Some(start), Some(end)) = (start_time, end_time) {
            sqlx::query_as::<_, WeightHistorySummaryRow>(
                r#"
                SELECT 
                    COUNT(*) as total_adjustments,
                    AVG(performance_impact) as avg_performance_impact
                FROM weight_adjustment_history
                WHERE timestamp >= $1::timestamptz AND timestamp <= $2::timestamptz
                "#,
            )
            .bind(start)
            .bind(end)
            .fetch_one(pool)
            .await
        } else {
            sqlx::query_as::<_, WeightHistorySummaryRow>(
                r#"
                SELECT 
                    COUNT(*) as total_adjustments,
                    AVG(performance_impact) as avg_performance_impact
                FROM weight_adjustment_history
                "#,
            )
            .fetch_one(pool)
            .await
        }
        .map_err(|e| {
            error!("Failed to get weight adjustment summary: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        // 获取最常见的调整类型（简化实现）
        let most_common = "ltm_weight_increase".to_string();

        Ok(WeightHistorySummary {
            total_adjustments: row.total_adjustments.unwrap_or(0) as usize,
            average_performance_impact: row.avg_performance_impact.unwrap_or(0.0) as f32,
            most_common_adjustment: most_common,
        })
    }

    /// 将数据库行转换为 HistoryItem
    pub fn row_to_history_item(row: &WeightHistoryRow) -> Result<HistoryItem, AppError> {
        let old_weights: MemoryWeights =
            serde_json::from_str(&row.old_weights_json).map_err(|e| {
                error!("Failed to deserialize old weights: {}", e);
                AppError::Internal(format!("Deserialization error: {}", e))
            })?;

        let new_weights: MemoryWeights =
            serde_json::from_str(&row.new_weights_json).map_err(|e| {
                error!("Failed to deserialize new weights: {}", e);
                AppError::Internal(format!("Deserialization error: {}", e))
            })?;

        let reasons: AdjustmentReasons = serde_json::from_str(&row.adjustment_reasons_json)
            .map_err(|e| {
                error!("Failed to deserialize adjustment reasons: {}", e);
                AppError::Internal(format!("Deserialization error: {}", e))
            })?;

        // 合并调整原因
        let reason = format!(
            "STM: {}; LTM: {}; KG: {}; MM: {}",
            reasons.stm, reasons.ltm, reasons.kg, reasons.mm
        );

        Ok(HistoryItem {
            timestamp: row.timestamp.clone(),
            task_id: row.task_id.clone(),
            old_weights,
            new_weights,
            reason,
            performance_impact: row.performance_impact,
            strategy_metadata: row.strategy_metadata.clone(),
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct WeightHistoryRow {
    pub history_id: String,
    pub task_id: String,
    pub timestamp: String,
    pub old_weights_json: String,
    pub new_weights_json: String,
    pub adjustment_reasons_json: String,
    pub performance_impact: f32,
    pub strategy_metadata: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct WeightHistorySummaryRow {
    total_adjustments: Option<i64>,
    avg_performance_impact: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct WeightHistorySummary {
    pub total_adjustments: usize,
    pub average_performance_impact: f32,
    pub most_common_adjustment: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HistoryItem {
    pub timestamp: String,
    pub task_id: String,
    pub old_weights: MemoryWeights,
    pub new_weights: MemoryWeights,
    pub reason: String,
    pub performance_impact: f32,
    /// JSON array of strategy names used for this adjustment (e.g. ["MarginalBenefit","LinearDecay"]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_metadata: Option<String>,
}
