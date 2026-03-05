use tracing::{error, info};
use ulid::Ulid;

use crate::db::pool;
use crate::models::*;
use crate::AppError;

pub struct MemoryConfigRepository;

impl MemoryConfigRepository {
    /// 创建新的记忆配置
    pub async fn create(
        user_id: &str,
        agent_id: &str,
        config_name: &str,
        config_type: &str,
        memory_config: &MemoryConfig,
    ) -> Result<String, AppError> {
        let config_id = Ulid::new().to_string();
        let pool = pool();

        // 将 MemoryConfig 转换为数据库格式
        let stm_enabled = 1;
        let ltm_enabled = if memory_config.memory_weights.ltm > 0.0 { 1 } else { 0 };
        let kg_enabled = if memory_config.memory_weights.kg > 0.0 { 1 } else { 0 };
        let mm_enabled = if memory_config.enable_multimodal { 1 } else { 0 };

        let secondary_memory_str = memory_config
            .secondary_memory
            .iter()
            .map(|m| format!("{:?}", m).to_lowercase())
            .collect::<Vec<_>>()
            .join(",");

        sqlx::query(
            r#"
            INSERT INTO memory_configurations (
                config_id, user_id, agent_id, config_name, config_type,
                stm_enabled, stm_max_length, stm_retention_hours,
                ltm_enabled, ltm_max_entries, ltm_quality_threshold,
                kg_enabled, kg_max_entities, kg_confidence_threshold,
                mm_enabled, mm_max_entries, mm_modality_types,
                max_response_time_ms, max_memory_usage_mb, max_cpu_usage_percent,
                status
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8,
                $9, $10, $11,
                $12, $13, $14,
                $15, $16, $17,
                $18, $19, $20,
                'active'
            )
            "#,
        )
        .bind(&config_id)
        .bind(user_id)
        .bind(agent_id)
        .bind(config_name)
        .bind(config_type)
        .bind(stm_enabled)
        .bind(4096i32)
        .bind(24i32)
        .bind(ltm_enabled)
        .bind(10000i32)
        .bind(0.5f64)
        .bind(kg_enabled)
        .bind(1000i32)
        .bind(0.7f64)
        .bind(mm_enabled)
        .bind(1000i32)
        .bind(if memory_config.enable_multimodal {
            Some(secondary_memory_str.as_str())
        } else {
            None::<&str>
        })
        .bind(2000i32)
        .bind(1024i32)
        .bind(80i32)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create memory configuration: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Created memory configuration: {}", config_id);
        Ok(config_id)
    }

    /// 根据 config_id 获取配置
    pub async fn get_by_id(config_id: &str) -> Result<Option<MemoryConfigRow>, AppError> {
        let pool = pool();

        let row = sqlx::query_as::<_, MemoryConfigRow>(
            r#"
            SELECT 
                config_id, user_id, agent_id, config_name, config_type,
                stm_enabled, stm_max_length, stm_retention_hours,
                ltm_enabled, ltm_max_entries, ltm_quality_threshold,
                kg_enabled, kg_max_entities, kg_confidence_threshold,
                mm_enabled, mm_max_entries, mm_modality_types,
                max_response_time_ms, max_memory_usage_mb, max_cpu_usage_percent,
                created_at::text, updated_at::text, status
            FROM memory_configurations
            WHERE config_id = $1
            "#,
        )
        .bind(config_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to get memory configuration: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(row)
    }

    /// 根据 user_id 和 agent_id 获取最新配置
    pub async fn get_latest(
        user_id: &str,
        agent_id: &str,
    ) -> Result<Option<MemoryConfigRow>, AppError> {
        let pool = pool();

        let row = sqlx::query_as::<_, MemoryConfigRow>(
            r#"
            SELECT 
                config_id, user_id, agent_id, config_name, config_type,
                stm_enabled, stm_max_length, stm_retention_hours,
                ltm_enabled, ltm_max_entries, ltm_quality_threshold,
                kg_enabled, kg_max_entities, kg_confidence_threshold,
                mm_enabled, mm_max_entries, mm_modality_types,
                max_response_time_ms, max_memory_usage_mb, max_cpu_usage_percent,
                created_at::text, updated_at::text, status
            FROM memory_configurations
            WHERE user_id = $1 AND agent_id = $2 AND status = 'active'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(agent_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to get latest memory configuration: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(row)
    }

    /// 获取配置列表（支持分页和筛选）
    pub async fn list(
        page: Option<u32>,
        page_size: Option<u32>,
        user_id: Option<&str>,
        agent_id: Option<&str>,
        status: Option<&str>,
        config_type: Option<&str>,
    ) -> Result<(Vec<MemoryConfigRow>, u64), AppError> {
        let pool = pool();
        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(20);
        let offset = (page - 1) * page_size;


        // 简化实现：先获取所有数据，然后在内存中筛选（对于小数据集足够）
        // 对于大数据集，应该使用动态 SQL 构建
        let all_rows = sqlx::query_as::<_, MemoryConfigRow>(
            r#"
            SELECT 
                config_id, user_id, agent_id, config_name, config_type,
                stm_enabled, stm_max_length, stm_retention_hours,
                ltm_enabled, ltm_max_entries, ltm_quality_threshold,
                kg_enabled, kg_max_entities, kg_confidence_threshold,
                mm_enabled, mm_max_entries, mm_modality_types,
                max_response_time_ms, max_memory_usage_mb, max_cpu_usage_percent,
                created_at::text, updated_at::text, status
            FROM memory_configurations
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to list memory configurations: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        // 在内存中筛选
        let filtered: Vec<MemoryConfigRow> = all_rows
            .into_iter()
            .filter(|row| {
                if let Some(uid) = user_id {
                    if row.user_id != uid {
                        return false;
                    }
                }
                if let Some(aid) = agent_id {
                    if row.agent_id != aid {
                        return false;
                    }
                }
                if let Some(st) = status {
                    if row.status != st {
                        return false;
                    }
                }
                if let Some(ct) = config_type {
                    if row.config_type != ct {
                        return false;
                    }
                }
                true
            })
            .collect();

        let total = filtered.len() as u64;

        // 分页
        let start = offset as usize;
        let paginated: Vec<MemoryConfigRow> = if start < filtered.len() {
            filtered.into_iter().skip(start).take(page_size as usize).collect()
        } else {
            Vec::new()
        };

        Ok((paginated, total))
    }

    /// 更新配置的所有字段
    pub async fn update(
        config_id: &str,
        row: &MemoryConfigRow,
    ) -> Result<(), AppError> {
        let pool = pool();

        sqlx::query(
            r#"
            UPDATE memory_configurations
            SET 
                user_id = $1,
                agent_id = $2,
                config_name = $3,
                config_type = $4,
                stm_enabled = $5,
                stm_max_length = $6,
                stm_retention_hours = $7,
                ltm_enabled = $8,
                ltm_max_entries = $9,
                ltm_quality_threshold = $10,
                kg_enabled = $11,
                kg_max_entities = $12,
                kg_confidence_threshold = $13,
                mm_enabled = $14,
                mm_max_entries = $15,
                mm_modality_types = $16,
                max_response_time_ms = $17,
                max_memory_usage_mb = $18,
                max_cpu_usage_percent = $19,
                status = $20,
                updated_at = CURRENT_TIMESTAMP
            WHERE config_id = $21
            "#,
        )
        .bind(&row.user_id)
        .bind(&row.agent_id)
        .bind(&row.config_name)
        .bind(&row.config_type)
        .bind(row.stm_enabled)
        .bind(row.stm_max_length)
        .bind(row.stm_retention_hours)
        .bind(row.ltm_enabled)
        .bind(row.ltm_max_entries)
        .bind(row.ltm_quality_threshold)
        .bind(row.kg_enabled)
        .bind(row.kg_max_entities)
        .bind(row.kg_confidence_threshold)
        .bind(row.mm_enabled)
        .bind(row.mm_max_entries)
        .bind(&row.mm_modality_types)
        .bind(row.max_response_time_ms)
        .bind(row.max_memory_usage_mb)
        .bind(row.max_cpu_usage_percent)
        .bind(&row.status)
        .bind(config_id)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to update memory configuration: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Updated memory configuration: {}", config_id);
        Ok(())
    }

    /// 更新配置状态
    pub async fn update_status(
        config_id: &str,
        status: &str,
    ) -> Result<(), AppError> {
        let pool = pool();

        sqlx::query(
            r#"
            UPDATE memory_configurations
            SET status = $1, updated_at = CURRENT_TIMESTAMP
            WHERE config_id = $2
            "#,
        )
        .bind(status)
        .bind(config_id)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to update memory configuration status: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Updated memory configuration status: {} -> {}", config_id, status);
        Ok(())
    }

    /// 软删除配置（更新状态为 inactive）
    pub async fn delete(config_id: &str) -> Result<(), AppError> {
        Self::update_status(config_id, "inactive").await
    }

    /// 将数据库行转换为 MemoryConfig
    pub fn row_to_memory_config(row: &MemoryConfigRow) -> MemoryConfig {
        let primary_memory = if row.stm_enabled == 1 {
            MemoryType::Stm
        } else if row.ltm_enabled == 1 {
            MemoryType::Ltm
        } else if row.kg_enabled == 1 {
            MemoryType::Kg
        } else {
            MemoryType::Mm
        };

        let mut secondary_memory = Vec::new();
        if row.ltm_enabled == 1 {
            secondary_memory.push(MemoryType::Ltm);
        }
        if row.kg_enabled == 1 {
            secondary_memory.push(MemoryType::Kg);
        }
        if row.mm_enabled == 1 {
            secondary_memory.push(MemoryType::Mm);
        }

        MemoryConfig {
            primary_memory,
            secondary_memory,
            memory_weights: MemoryWeights {
                stm: if row.stm_enabled == 1 { 1.0 } else { 0.0 },
                ltm: if row.ltm_enabled == 1 { 0.8 } else { 0.0 },
                kg: if row.kg_enabled == 1 { 0.7 } else { 0.0 },
                mm: if row.mm_enabled == 1 { 0.6 } else { 0.0 },
            },
            reasoning_depth: "medium".to_string(),
            enable_multimodal: row.mm_enabled == 1,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, salvo::oapi::ToSchema)]
pub struct MemoryConfigRow {
    pub config_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub config_name: String,
    pub config_type: String,
    pub stm_enabled: i32,
    pub stm_max_length: i32,
    pub stm_retention_hours: i32,
    pub ltm_enabled: i32,
    pub ltm_max_entries: i32,
    pub ltm_quality_threshold: f64,
    pub kg_enabled: i32,
    pub kg_max_entities: i32,
    pub kg_confidence_threshold: f64,
    pub mm_enabled: i32,
    pub mm_max_entries: i32,
    pub mm_modality_types: Option<String>,
    pub max_response_time_ms: i32,
    pub max_memory_usage_mb: i32,
    pub max_cpu_usage_percent: i32,
    pub created_at: String,
    pub updated_at: String,
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_row_to_memory_config() {
        let row = MemoryConfigRow {
            config_id: "test".to_string(),
            user_id: "user_1".to_string(),
            agent_id: "agent_1".to_string(),
            config_name: "test".to_string(),
            config_type: "custom".to_string(),
            stm_enabled: 1,
            stm_max_length: 4096,
            stm_retention_hours: 24,
            ltm_enabled: 1,
            ltm_max_entries: 10000,
            ltm_quality_threshold: 0.5,
            kg_enabled: 0,
            kg_max_entities: 1000,
            kg_confidence_threshold: 0.7,
            mm_enabled: 0,
            mm_max_entries: 1000,
            mm_modality_types: None,
            max_response_time_ms: 2000,
            max_memory_usage_mb: 1024,
            max_cpu_usage_percent: 80,
            created_at: "2024-01-01T00:00:00".to_string(),
            updated_at: "2024-01-01T00:00:00".to_string(),
            status: "active".to_string(),
        };

        let config = MemoryConfigRepository::row_to_memory_config(&row);
        assert_eq!(config.memory_weights.stm, 1.0);
        assert_eq!(config.memory_weights.ltm, 0.8);
        assert_eq!(config.memory_weights.kg, 0.0);
        assert_eq!(config.memory_weights.mm, 0.0);
    }
}
