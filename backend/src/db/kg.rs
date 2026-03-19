use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use tracing::{error, info};
use ulid::Ulid;

use crate::db::pool;
use crate::AppError;

/// 知识图谱仓库
pub struct KGRepository;

/// 实体列表响应
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EntityListResponse {
    pub entities: Vec<Entity>,
    pub total: usize,
    pub limit: i32,
    pub offset: i32,
}

/// 实体信息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Entity {
    pub entity_id: String,
    pub entity_name: String,
    pub entity_type: String,
    pub description: Option<String>,
    pub attributes: Option<String>,
    pub aliases: Option<String>,
    pub embedding_vector: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_dimension: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
    pub confidence_score: f32,
    pub popularity_score: f32,
    pub relation_count: i32,
    pub mention_count: i32,
    pub status: String,
    // Bi-temporal fields
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
    pub superseded_by: Option<String>,
}

/// 关系信息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Relation {
    pub relation_id: String,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relation_type: String,
    pub relation_name: Option<String>,
    pub description: Option<String>,
    pub properties: Option<String>,
    pub weight: f64,
    pub confidence: f64,
    pub created_at: String,
    pub updated_at: String,
    pub usage_count: i32,
    pub success_count: i32,
    pub status: String,
}

impl KGRepository {
    /// 创建实体
    pub async fn create_entity(
        entity_name: &str,
        entity_type: &str,
        description: Option<&str>,
        attributes: Option<&Value>,
        aliases: Option<&[&str]>,
        embedding_vector: Option<&[f32]>,
        embedding_model: Option<&str>,
        confidence_score: f64,
    ) -> Result<String, AppError> {
        let entity_id = Ulid::new().to_string();
        let pool = pool();

        // 处理属性
        let attributes_json = if let Some(attrs) = attributes {
            serde_json::to_string(attrs).map_err(|e| {
                error!("Failed to serialize attributes: {}", e);
                AppError::Internal(format!("Failed to serialize attributes: {}", e))
            })?
        } else {
            "{}".to_string()
        };

        // 处理别名
        let aliases_json = if let Some(alias_list) = aliases {
            serde_json::to_string(alias_list).map_err(|e| {
                error!("Failed to serialize aliases: {}", e);
                AppError::Internal(format!("Failed to serialize aliases: {}", e))
            })?
        } else {
            "[]".to_string()
        };

        // 处理向量
        let embedding_json = if let Some(vec) = embedding_vector {
            serde_json::to_string(vec).map_err(|e| {
                error!("Failed to serialize embedding vector: {}", e);
                AppError::Internal(format!("Failed to serialize embedding: {}", e))
            })?
        } else {
            "null".to_string()
        };

        let embedding_dimension = embedding_vector.map(|v| v.len() as i32);

        sqlx::query(
            r#"
            INSERT INTO entities (
                entity_id, entity_name, entity_type, description, attributes, aliases,
                embedding_vector, embedding_model, embedding_dimension, confidence_score
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(entity_id.clone())
        .bind(entity_name)
        .bind(entity_type)
        .bind(description)
        .bind(attributes_json)
        .bind(aliases_json)
        .bind(embedding_json)
        .bind(embedding_model)
        .bind(embedding_dimension)
        .bind(confidence_score)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create entity: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(entity_id)
    }

    /// 根据实体名称和类型获取实体
    pub async fn get_entity_by_name(
        entity_name: &str,
        entity_type: Option<&str>,
    ) -> Result<Option<Entity>, AppError> {
        let pool = pool();

        let query = if let Some(et) = entity_type {
            sqlx::query_as::<_, Entity>(
                r#"
                SELECT entity_id, entity_name, entity_type, description, attributes, aliases,
                       embedding_vector, embedding_model, embedding_dimension,
                       created_at, updated_at, confidence_score, popularity_score,
                       relation_count, mention_count, status
                FROM entities
                WHERE entity_name = $1 AND entity_type = $2 AND status = 'active'
                LIMIT 1
                "#,
            )
            .bind(entity_name)
            .bind(et)
        } else {
            sqlx::query_as::<_, Entity>(
                r#"
                SELECT entity_id, entity_name, entity_type, description, attributes, aliases,
                       embedding_vector, embedding_model, embedding_dimension,
                       created_at, updated_at, confidence_score, popularity_score,
                       relation_count, mention_count, status
                FROM entities
                WHERE entity_name = $1 AND status = 'active'
                LIMIT 1
                "#,
            )
            .bind(entity_name)
        };

        let entity = query.fetch_optional(pool).await.map_err(|e| {
            error!("Failed to get entity by name: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(entity)
    }

    /// 根据实体ID获取实体
    pub async fn get_entity_by_id(entity_id: &str) -> Result<Option<Entity>, AppError> {
        let pool = pool();

        let entity = sqlx::query_as::<_, Entity>(
            r#"
            SELECT entity_id, entity_name, entity_type, description, attributes, aliases,
                   embedding_vector, embedding_model, embedding_dimension,
                   created_at, updated_at, confidence_score, popularity_score,
                   relation_count, mention_count, status
            FROM entities
            WHERE entity_id = $1 AND status = 'active'
            LIMIT 1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to get entity by id: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(entity)
    }

    /// 创建关系
    pub async fn create_relation(
        source_entity_id: &str,
        target_entity_id: &str,
        relation_type: &str,
        weight: f64,
        confidence: f64,
        properties: Option<&Value>,
    ) -> Result<String, AppError> {
        let relation_id = Ulid::new().to_string();
        let pool = pool();

        // 处理属性
        let properties_json = if let Some(props) = properties {
            serde_json::to_string(props).map_err(|e| {
                error!("Failed to serialize properties: {}", e);
                AppError::Internal(format!("Failed to serialize properties: {}", e))
            })?
        } else {
            "{}".to_string()
        };

        sqlx::query(
            r#"
            INSERT INTO relations (
                relation_id, source_entity_id, target_entity_id, relation_type,
                weight, confidence, properties
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(relation_id.clone())
        .bind(source_entity_id)
        .bind(target_entity_id)
        .bind(relation_type)
        .bind(weight)
        .bind(confidence)
        .bind(properties_json)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create relation: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        // 更新源实体和目标实体的关系计数
        sqlx::query("UPDATE entities SET relation_count = relation_count + 1 WHERE entity_id = $1")
            .bind(source_entity_id)
            .execute(pool)
            .await
            .map_err(|e| {
                error!("Failed to update source entity relation count: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;

        sqlx::query("UPDATE entities SET relation_count = relation_count + 1 WHERE entity_id = $1")
            .bind(target_entity_id)
            .execute(pool)
            .await
            .map_err(|e| {
                error!("Failed to update target entity relation count: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;

        Ok(relation_id)
    }

    /// 获取实体的相关实体
    pub async fn get_related_entities(
        entity_id: &str,
        relation_type: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<(Entity, Relation)>, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(10);

        // 分别查询实体和关系，避免字段冲突
        let relation_query = if let Some(rt) = relation_type {
            sqlx::query_as::<_, Relation>(
                r#"
                SELECT relation_id, source_entity_id, target_entity_id, relation_type, relation_name, 
                       r.description, properties, weight, confidence, 
                       r.created_at, r.updated_at, usage_count, success_count, r.status
                FROM relations r
                WHERE r.source_entity_id = $1 AND r.relation_type = $2 AND r.status = 'active'
                ORDER BY r.weight DESC, r.confidence DESC
                LIMIT $3
                "#,
            )
            .bind(entity_id)
            .bind(rt)
            .bind(limit)
        } else {
            sqlx::query_as::<_, Relation>(
                r#"
                SELECT relation_id, source_entity_id, target_entity_id, relation_type, relation_name, 
                       r.description, properties, weight, confidence, 
                       r.created_at, r.updated_at, usage_count, success_count, r.status
                FROM relations r
                WHERE r.source_entity_id = $1 AND r.status = 'active'
                ORDER BY r.weight DESC, r.confidence DESC
                LIMIT $2
                "#,
            )
            .bind(entity_id)
            .bind(limit)
        };

        let relations = relation_query.fetch_all(pool).await.map_err(|e| {
            error!("Failed to get relations: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        let mut result = Vec::new();
        for relation in relations {
            // 查询对应的目标实体
            if let Some(entity) = Self::get_entity_by_id(&relation.target_entity_id).await? {
                result.push((entity, relation));
            }
        }

        Ok(result)
    }

    /// 根据实体搜索相关的知识条目 (pool版本)
    pub async fn search_knowledge_by_entity(
        pool: &sqlx::PgPool,
        entity_name: &str,
        limit: Option<i32>,
    ) -> Result<Vec<Entity>, AppError> {
        let limit = limit.unwrap_or(10);

        // 搜索与该实体相关的知识条目
        // 通过关系表找到相关的 source/target 实体，然后搜索知识条目
        let rows = sqlx::query_as::<_, Entity>(
            r#"
            SELECT DISTINCT e.entity_id, e.entity_name, e.entity_type, e.description,
                   e.attributes, e.aliases, e.embedding_vector, e.confidence_score,
                   e.popularity_score, e.relation_count, e.mention_count, e.status
            FROM entities e
            LEFT JOIN relations r ON e.entity_id = r.source_entity_id OR e.entity_id = r.target_entity_id
            LEFT JOIN knowledge_entries ke ON ke.content LIKE '%' || e.entity_name || '%'
            WHERE e.entity_name LIKE '%' || $1 || '%'
               OR r.source_entity_id IN (SELECT entity_id FROM entities WHERE entity_name LIKE '%' || $1 || '%')
               OR r.target_entity_id IN (SELECT entity_id FROM entities WHERE entity_name LIKE '%' || $1 || '%')
            ORDER BY e.popularity_score DESC, e.mention_count DESC
            LIMIT $2
            "#,
        )
        .bind(entity_name)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to search knowledge by entity: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(rows)
    }

    /// 搜索包含指定实体的知识条目
    pub async fn search_entries_by_entity(
        pool: &sqlx::PgPool,
        entity_name: &str,
        top_k: i32,
    ) -> Result<Vec<Entity>, AppError> {
        // 搜索包含该实体名称的知识条目
        let rows = sqlx::query_as::<_, Entity>(
            r#"
            SELECT DISTINCT e.entity_id, e.entity_name, e.entity_type, e.description,
                   e.attributes, e.aliases, e.embedding_vector, e.confidence_score,
                   e.popularity_score, e.relation_count, e.mention_count, e.status
            FROM entities e
            WHERE e.entity_name LIKE '%' || $1 || '%'
               OR e.entity_name IN (
                   SELECT entity_name FROM entities
                   WHERE entity_id IN (
                       SELECT source_entity_id FROM relations WHERE target_entity_id IN (
                           SELECT entity_id FROM entities WHERE entity_name LIKE '%' || $1 || '%'
                       )
                       UNION
                       SELECT target_entity_id FROM relations WHERE source_entity_id IN (
                           SELECT entity_id FROM entities WHERE entity_name LIKE '%' || $1 || '%'
                       )
                   )
               )
            ORDER BY e.popularity_score DESC, e.mention_count DESC
            LIMIT $2
            "#,
        )
        .bind(entity_name)
        .bind(top_k)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to search entries by entity: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(rows)
    }

    /// 获取实体列表
    pub async fn list_entities(
        entity_type: Option<&str>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<EntityListResponse, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(20);
        let offset = offset.unwrap_or(0);

        let (entities, total): (Vec<Entity>, (i64,)) = if let Some(et) = entity_type {
            let entities = sqlx::query_as::<_, Entity>(
                r#"
                SELECT entity_id, entity_name, entity_type, description, attributes, aliases,
                       embedding_vector, embedding_model, embedding_dimension,
                       created_at::text as created_at, updated_at::text as updated_at,
                       confidence_score, popularity_score, relation_count, mention_count, status
                FROM entities
                WHERE status = 'active' AND entity_type = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(et)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                error!("Failed to list entities: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;

            let total = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM entities WHERE status = 'active' AND entity_type = $1",
            )
            .bind(et)
            .fetch_one(pool)
            .await
            .map_err(|e| {
                error!("Failed to count entities: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;
            (entities, total)
        } else {
            let entities = sqlx::query_as::<_, Entity>(
                r#"
                SELECT entity_id, entity_name, entity_type, description, attributes, aliases,
                       embedding_vector, embedding_model, embedding_dimension,
                       created_at::text as created_at, updated_at::text as updated_at,
                       confidence_score, popularity_score, relation_count, mention_count, status
                FROM entities
                WHERE status = 'active'
                ORDER BY created_at DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                error!("Failed to list entities: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;

            let total = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM entities WHERE status = 'active'",
            )
            .fetch_one(pool)
            .await
            .map_err(|e| {
                error!("Failed to count entities: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;
            (entities, total)
        };

        Ok(EntityListResponse {
            entities,
            total: total.0 as usize,
            limit,
            offset,
        })
    }

    // ============ Bi-temporal Tracking Methods ============

    /// 获取特定时间点的实体（时间旅行查询）
    pub async fn get_entity_at_time(
        entity_id: &str,
        at_timestamp: &str,
    ) -> Result<Option<Entity>, AppError> {
        let pool = pool();

        let entity = sqlx::query_as::<_, Entity>(
            r#"
            SELECT entity_id, entity_name, entity_type, description, attributes, aliases,
                   embedding_vector, embedding_model, embedding_dimension,
                   created_at::text as created_at, updated_at::text as updated_at,
                   confidence_score, popularity_score, relation_count, mention_count, status,
                   valid_from::text as valid_from,
                   valid_until::text as valid_until,
                   superseded_by
            FROM entities
            WHERE entity_id = $1
              AND valid_from <= $2
              AND (valid_until IS NULL OR valid_until > $2)
            "#,
        )
        .bind(entity_id)
        .bind(at_timestamp)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to get entity at time: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(entity)
    }

    /// 获取实体的版本历史
    pub async fn get_entity_history(
        entity_id: &str,
    ) -> Result<Vec<Entity>, AppError> {
        let pool = pool();

        let entities = sqlx::query_as::<_, Entity>(
            r#"
            SELECT entity_id, entity_name, entity_type, description, attributes, aliases,
                   embedding_vector, embedding_model, embedding_dimension,
                   created_at::text as created_at, updated_at::text as updated_at,
                   confidence_score, popularity_score, relation_count, mention_count, status,
                   valid_from::text as valid_from,
                   valid_until::text as valid_until,
                   superseded_by
            FROM entities
            WHERE entity_id = $1
            ORDER BY version DESC, valid_from DESC
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get entity history: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(entities)
    }

    /// 更新实体时创建新版本（保留历史）
    pub async fn supersede_entity(
        entity_id: &str,
        new_name: &str,
        new_type: &str,
        new_description: Option<&str>,
    ) -> Result<String, AppError> {
        let pool = pool();
        let new_entity_id = Ulid::new().to_string();

        // 获取当前实体
        let current = Self::get_entity_by_id(entity_id).await?;
        if current.is_none() {
            return Err(AppError::NotFound(format!("Entity {} not found", entity_id)));
        }
        let current = current.unwrap();

        // 将当前实体标记为被替换
        sqlx::query(
            r#"
            UPDATE entities
            SET valid_until = CURRENT_TIMESTAMP,
                superseded_by = $1,
                status = 'deprecated'
            WHERE entity_id = $2
            "#,
        )
        .bind(&new_entity_id)
        .bind(entity_id)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to supersede entity: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        // 创建新版本实体
        sqlx::query(
            r#"
            INSERT INTO entities (
                entity_id, entity_name, entity_type, description, attributes, aliases,
                embedding_vector, embedding_model, embedding_dimension,
                confidence_score, popularity_score, relation_count, mention_count,
                status, version, valid_from
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'active', 1, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(&new_entity_id)
        .bind(new_name)
        .bind(new_type)
        .bind(new_description)
        .bind(&current.attributes)
        .bind(&current.aliases)
        .bind(&current.embedding_vector)
        .bind(&current.embedding_model)
        .bind(current.embedding_dimension)
        .bind(current.confidence_score)
        .bind(current.popularity_score)
        .bind(current.relation_count)
        .bind(current.mention_count)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create new entity version: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Superseded entity {} with new version {}", entity_id, new_entity_id);
        Ok(new_entity_id)
    }
}
