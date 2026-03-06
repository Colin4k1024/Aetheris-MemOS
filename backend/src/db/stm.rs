use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::{error, info};
use ulid::Ulid;

use crate::db::pool;
use crate::AppError;

/// 短期记忆会话仓库
pub struct STMRepository;

/// 会话信息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, salvo::oapi::ToSchema)]
pub struct Session {
    pub session_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
    pub session_type: String,
    pub context_length: i32,
    pub max_context_length: i32,
    pub status: String,
    pub priority: i32,
}

/// 会话列表响应
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, salvo::oapi::ToSchema)]
pub struct SessionListResponse {
    pub sessions: Vec<Session>,
    pub total: usize,
    pub limit: i32,
    pub offset: i32,
}

/// 会话消息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, salvo::oapi::ToSchema)]
pub struct SessionMessage {
    pub message_id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub token_count: Option<i32>,
    pub importance_score: Option<f32>,
}

impl STMRepository {
    /// 创建新会话
    pub async fn create_session(
        user_id: &str,
        agent_id: &str,
        session_type: &str,
        max_context_length: i32,
        retention_hours: i32,
    ) -> Result<String, AppError> {
        let session_id = Ulid::new().to_string();
        let pool = pool();

        // 计算过期时间（用于日志，实际过期时间在SQL中计算）
        let _expires_at = format!(
            "datetime('now', '+{} hours')",
            retention_hours
        );

        sqlx::query(
            r#"
            INSERT INTO context_sessions (
                session_id, user_id, agent_id, session_type,
                max_context_length, expires_at, status, priority
            ) VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP + ($6::text || ' hours')::interval, 'active', 5)
            "#,
        )
        .bind(&session_id)
        .bind(user_id)
        .bind(agent_id)
        .bind(session_type)
        .bind(max_context_length)
        .bind(retention_hours)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create session: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Created new session: {}", session_id);
        Ok(session_id)
    }

    /// 添加消息到会话
    pub async fn add_message(
        session_id: &str,
        role: &str,
        content: &str,
        token_count: Option<i32>,
        importance_score: Option<f64>,
    ) -> Result<String, AppError> {
        let message_id = Ulid::new().to_string();
        let pool = pool();

        sqlx::query(
            r#"
            INSERT INTO session_messages (
                message_id, session_id, role, content, token_count, importance_score
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&message_id)
        .bind(session_id)
        .bind(role)
        .bind(content)
        .bind(token_count)
        .bind(importance_score)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to add message: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        // 更新会话的上下文长度
        sqlx::query(
            r#"
            UPDATE context_sessions
            SET context_length = context_length + $1,
                updated_at = CURRENT_TIMESTAMP
            WHERE session_id = $2
            "#,
        )
        .bind(token_count.unwrap_or(0))
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to update session context length: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Added message to session: {}", session_id);
        Ok(message_id)
    }

    /// 获取会话消息
    pub async fn get_session_messages(
        session_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<SessionMessage>, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(100);

        let messages = sqlx::query_as::<_, SessionMessage>(
            r#"
            SELECT message_id, session_id, role, content, created_at::text, token_count, importance_score
            FROM session_messages
            WHERE session_id = $1
            ORDER BY created_at ASC
            LIMIT $2
            "#,
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get session messages: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(messages)
    }

    /// 获取最近会话
    pub async fn get_recent_sessions(
        user_id: &str,
        agent_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<Session>, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(10);

        let sessions = sqlx::query_as::<_, Session>(
            r#"
            SELECT session_id, user_id, agent_id, created_at::text, updated_at::text, expires_at::text,
                   session_type, context_length, max_context_length, status, priority
            FROM context_sessions
            WHERE user_id = $1 AND agent_id = $2 AND status = 'active'
            ORDER BY updated_at DESC
            LIMIT $3
            "#,
        )
        .bind(user_id)
        .bind(agent_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get recent sessions: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(sessions)
    }

    /// 会话列表响应
    pub async fn list_sessions(
        user_id: Option<&str>,
        status: Option<&str>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<SessionListResponse, AppError> {
        let pool = pool();
        let limit = limit.unwrap_or(20);
        let offset = offset.unwrap_or(0);

        let mut query = String::from(
            "SELECT session_id, user_id, agent_id, created_at::text, updated_at::text, expires_at::text,
             session_type, context_length, max_context_length, status, priority
             FROM context_sessions WHERE 1=1"
        );

        let mut count_query = String::from("SELECT COUNT(*) FROM context_sessions WHERE 1=1");

        if let Some(uid) = user_id {
            query.push_str(&format!(" AND user_id = '{}'", uid));
            count_query.push_str(&format!(" AND user_id = '{}'", uid));
        }

        if let Some(s) = status {
            query.push_str(&format!(" AND status = '{}'", s));
            count_query.push_str(&format!(" AND status = '{}'", s));
        }

        query.push_str(&format!(" ORDER BY created_at DESC LIMIT {} OFFSET {}", limit, offset));

        let sessions: Vec<Session> = sqlx::query_as(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                error!("Failed to list sessions: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;

        let total: (i64,) = sqlx::query_as(&count_query)
            .fetch_one(pool)
            .await
            .map_err(|e| {
                error!("Failed to count sessions: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;

        Ok(SessionListResponse {
            sessions,
            total: total.0 as usize,
            limit,
            offset,
        })
    }

    /// 获取所有活跃的 user_id 列表
    pub async fn get_active_user_ids() -> Result<Vec<String>, AppError> {
        let pool = pool();

        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT user_id FROM context_sessions WHERE status = 'active'",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get active user IDs: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// 获取指定用户的活跃 agent_id 列表
    pub async fn get_active_agent_ids(user_id: &str) -> Result<Vec<String>, AppError> {
        let pool = pool();

        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT agent_id FROM context_sessions WHERE user_id = $1 AND status = 'active'",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get active agent IDs: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}

