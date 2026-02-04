use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::{error, info};
use ulid::Ulid;

use crate::db::pool;
use crate::AppError;

/// 短期记忆会话仓库
pub struct STMRepository;

/// 会话信息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
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

/// 会话消息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, salvo::oapi::ToSchema)]
pub struct SessionMessage {
    pub message_id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub token_count: Option<i32>,
    pub importance_score: Option<f64>,
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

        // 计算过期时间
        let expires_at = format!(
            "datetime('now', '+{} hours')",
            retention_hours
        );

        sqlx::query(
            r#"
            INSERT INTO context_sessions (
                session_id, user_id, agent_id, session_type,
                max_context_length, expires_at, status, priority
            ) VALUES (?, ?, ?, ?, ?, datetime('now', '+' || ? || ' hours'), 'active', 5)
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
            ) VALUES (?, ?, ?, ?, ?, ?)
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
            SET context_length = context_length + ?,
                updated_at = datetime('now')
            WHERE session_id = ?
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
            SELECT message_id, session_id, role, content, created_at, token_count, importance_score
            FROM session_messages
            WHERE session_id = ?
            ORDER BY created_at ASC
            LIMIT ?
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
            SELECT session_id, user_id, agent_id, created_at, updated_at, expires_at,
                   session_type, context_length, max_context_length, status, priority
            FROM context_sessions
            WHERE user_id = ? AND agent_id = ? AND status = 'active'
            ORDER BY updated_at DESC
            LIMIT ?
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
}

