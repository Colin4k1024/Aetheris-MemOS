use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::{error, info};
use ulid::Ulid;

use crate::db::pool;
use crate::db::tenant_scope::begin_tenant_tx;
use crate::tenant::TenantId;
use crate::AppError;

/// Feature-gated Redis STM adapter import.
#[cfg(feature = "redis-stm")]
use crate::db::adapters::redis_stm::RedisStmAdapter;

/// 短期记忆会话仓库
pub struct STMRepository;

/// 会话信息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct SessionListResponse {
    pub sessions: Vec<Session>,
    pub total: usize,
    pub limit: i32,
    pub offset: i32,
}

/// 会话消息
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
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
    /// 创建新会话（租户隔离）
    ///
    /// When `redis-stm` feature is enabled, delegates to Redis adapter.
    /// Otherwise uses PostgreSQL directly.
    pub async fn create_session(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        session_type: &str,
        max_context_length: i32,
        retention_hours: i32,
    ) -> Result<String, AppError> {
        #[cfg(feature = "redis-stm")]
        {
            if RedisStmAdapter::is_available() {
                return RedisStmAdapter::create_session(
                    tenant_id,
                    user_id,
                    agent_id,
                    session_type,
                    max_context_length,
                    retention_hours,
                )
                .await;
            }
        }

        let session_id = Ulid::new().to_string();
        let pool = pool();

        // 计算过期时间（用于日志，实际过期时间在SQL中计算）
        let _expires_at = format!("datetime('now', '+{} hours')", retention_hours);

        // 构建租户限定的source_id用于后续跨租户隔离验证（过渡期与物理 tenant_id 列双写）
        let source_id = format!("{}:{}", tenant_id.prefix(), user_id);

        // RLS: open a tenant-scoped transaction so the write is enforced at the DB
        // layer (WITH CHECK on the physical tenant_id column), not just via the
        // user_id prefix.
        let mut tx = begin_tenant_tx(pool, tenant_id).await?;

        sqlx::query(
            r#"
            INSERT INTO context_sessions (
                session_id, user_id, agent_id, session_type,
                max_context_length, expires_at, status, priority, tenant_id
            ) VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP + ($6::text || ' hours')::interval, 'active', 5, $7)
            "#,
        )
        .bind(&session_id)
        .bind(source_id)
        .bind(agent_id)
        .bind(session_type)
        .bind(max_context_length)
        .bind(retention_hours)
        .bind(tenant_id.as_str())
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to create session: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        tx.commit().await.map_err(|e| {
            error!("Failed to commit create_session transaction: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!(
            "Created new session: {} for tenant: {}",
            session_id, tenant_id
        );
        Ok(session_id)
    }

    /// 根据session_id和tenant_id获取会话（租户隔离）
    pub async fn get_session(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        session_id: &str,
    ) -> Result<Option<Session>, AppError> {
        // RLS: tenant-scoped tx so the read is enforced at the DB layer.
        let mut tx = begin_tenant_tx(pool, tenant_id).await?;
        let session = sqlx::query_as::<_, Session>(
            r#"
            SELECT session_id, user_id, agent_id, created_at::text, updated_at::text, expires_at::text,
                   session_type, context_length, max_context_length, status, priority
            FROM context_sessions
            WHERE session_id = $1 AND status = 'active'
            "#,
        )
        .bind(session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to get session: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;
        tx.commit().await.ok();

        // 验证会话属于该租户（过渡期：RLS 之外的应用层兜底）
        if let Some(ref s) = session {
            let prefix = tenant_id.prefix();
            // 检查user_id是否以租户前缀开头（用于新数据格式）
            // 或者user_id等于tenant_id（用于MVP格式，每个user是自己的tenant）
            let belongs_to_tenant =
                s.user_id.starts_with(&prefix) || s.user_id == tenant_id.as_str();
            if !belongs_to_tenant {
                // 记录隔离违规
                crate::services::multi_tenant::record_isolation_violation(
                    tenant_id.as_str(),
                    session_id,
                    "stm_session_cross_tenant_access",
                );
                return Ok(None);
            }
        }

        Ok(session)
    }

    /// 添加消息到会话（租户隔离）
    ///
    /// When `redis-stm` feature is enabled, delegates to Redis adapter.
    /// Otherwise uses PostgreSQL directly.
    pub async fn add_message(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        session_id: &str,
        role: &str,
        content: &str,
        token_count: Option<i32>,
        importance_score: Option<f64>,
    ) -> Result<String, AppError> {
        #[cfg(feature = "redis-stm")]
        {
            if RedisStmAdapter::is_available() {
                return RedisStmAdapter::append_message(
                    tenant_id,
                    session_id,
                    role,
                    content,
                    token_count,
                    importance_score,
                )
                .await;
            }
        }

        // RLS: run the session verification read AND both writes inside a single
        // tenant-scoped transaction so the GUC is set for every statement and the
        // message insert's WITH CHECK (physical tenant_id) is enforced.
        let mut tx = begin_tenant_tx(pool, tenant_id).await?;

        // 验证session属于该租户
        let session = sqlx::query_as::<_, Session>(
            r#"
            SELECT session_id, user_id, agent_id, created_at::text, updated_at::text, expires_at::text,
                   session_type, context_length, max_context_length, status, priority
            FROM context_sessions
            WHERE session_id = $1 AND status = 'active'
            "#,
        )
        .bind(session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to get session for message: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        if let Some(ref s) = session {
            let prefix = tenant_id.prefix();
            let belongs_to_tenant =
                s.user_id.starts_with(&prefix) || s.user_id == tenant_id.as_str();
            if !belongs_to_tenant {
                crate::services::multi_tenant::record_isolation_violation(
                    tenant_id.as_str(),
                    session_id,
                    "stm_message_cross_tenant_access",
                );
                return Err(AppError::Forbidden(
                    "Session belongs to different tenant".to_string(),
                ));
            }
        } else {
            return Err(AppError::NotFound("Session not found".to_string()));
        }

        let message_id = Ulid::new().to_string();

        // Insert the message and bump the session context_length atomically: a failure
        // between the two writes must not leave the message stored with a stale count.
        sqlx::query(
            r#"
            INSERT INTO session_messages (
                message_id, session_id, role, content, token_count, importance_score, tenant_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&message_id)
        .bind(session_id)
        .bind(role)
        .bind(content)
        .bind(token_count)
        .bind(importance_score)
        .bind(tenant_id.as_str())
        .execute(&mut *tx)
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
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to update session context length: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        tx.commit().await.map_err(|e| {
            error!("Failed to commit add_message transaction: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!(
            "Added message to session: {} for tenant: {}",
            session_id, tenant_id
        );
        Ok(message_id)
    }

    /// 获取会话消息（租户隔离）
    ///
    /// When `redis-stm` feature is enabled, delegates to Redis adapter.
    /// Otherwise uses PostgreSQL directly.
    pub async fn get_session_messages(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        session_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<SessionMessage>, AppError> {
        #[cfg(feature = "redis-stm")]
        {
            if RedisStmAdapter::is_available() {
                return RedisStmAdapter::get_session_messages(tenant_id, session_id, limit)
                    .await
                    .map(|msgs| {
                        msgs.into_iter()
                            .map(|m| SessionMessage {
                                message_id: m.message_id,
                                session_id: m.session_id,
                                role: m.role,
                                content: m.content,
                                created_at: m.created_at,
                                token_count: m.token_count,
                                importance_score: m.importance_score,
                            })
                            .collect()
                    });
            }
        }

        // RLS: session verification read + messages fetch inside one tenant-scoped tx.
        let mut tx = begin_tenant_tx(pool, tenant_id).await?;

        // 验证session属于该租户
        let session = sqlx::query_as::<_, Session>(
            r#"
            SELECT session_id, user_id, agent_id, created_at::text, updated_at::text, expires_at::text,
                   session_type, context_length, max_context_length, status, priority
            FROM context_sessions
            WHERE session_id = $1 AND status = 'active'
            "#,
        )
        .bind(session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to get session: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        if let Some(ref s) = session {
            let prefix = tenant_id.prefix();
            let belongs_to_tenant =
                s.user_id.starts_with(&prefix) || s.user_id == tenant_id.as_str();
            if !belongs_to_tenant {
                crate::services::multi_tenant::record_isolation_violation(
                    tenant_id.as_str(),
                    session_id,
                    "stm_messages_cross_tenant_access",
                );
                return Ok(vec![]);
            }
        } else {
            return Ok(vec![]);
        }

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
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to get session messages: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;
        tx.commit().await.ok();

        Ok(messages)
    }

    /// 获取最近会话（租户隔离）
    pub async fn get_recent_sessions(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<Session>, AppError> {
        let limit = limit.unwrap_or(10);

        // 构建租户限定的user_id前缀用于过滤
        let prefix = tenant_id.prefix();
        let tenant_user_pattern = format!("{}:%", prefix);

        let mut tx = begin_tenant_tx(pool, tenant_id).await?;
        let sessions = sqlx::query_as::<_, Session>(
            r#"
            SELECT session_id, user_id, agent_id, created_at::text, updated_at::text, expires_at::text,
                   session_type, context_length, max_context_length, status, priority
            FROM context_sessions
            WHERE (user_id = $1 OR user_id LIKE $2) AND agent_id = $3 AND status = 'active'
            ORDER BY updated_at DESC
            LIMIT $4
            "#,
        )
        .bind(user_id)
        .bind(tenant_user_pattern)
        .bind(agent_id)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to get recent sessions: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;
        tx.commit().await.ok();

        Ok(sessions)
    }

    /// 会话列表响应（租户隔离）
    pub async fn list_sessions(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        user_id: Option<&str>,
        status: Option<&str>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<SessionListResponse, AppError> {
        let limit = limit.unwrap_or(20);
        let offset = offset.unwrap_or(0);

        // 构建租户前缀模式用于过滤
        let prefix = tenant_id.prefix();
        let tenant_pattern = format!("{}%", prefix);
        // Clone for use in second query
        let tenant_pattern_clone = tenant_pattern.clone();

        // RLS: all branch queries (fetch + count) run inside one tenant-scoped tx.
        let mut tx = begin_tenant_tx(pool, tenant_id).await?;

        let (sessions, total): (Vec<Session>, (i64,)) = match (user_id, status) {
            (Some(uid), Some(s)) => {
                let sessions = sqlx::query_as::<_, Session>(
                    r#"
                    SELECT session_id, user_id, agent_id, created_at::text, updated_at::text, expires_at::text,
                           session_type, context_length, max_context_length, status, priority
                    FROM context_sessions
                    WHERE (user_id = $1 OR user_id LIKE $2) AND status = $3
                    ORDER BY created_at DESC
                    LIMIT $4 OFFSET $5
                    "#,
                )
                .bind(uid)
                .bind(tenant_pattern)
                .bind(s)
                .bind(limit)
                .bind(offset)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Failed to list sessions: {}", e);
                    AppError::Internal(format!("Database error: {}", e))
                })?;

                let total = sqlx::query_as::<_, (i64,)>(
                    "SELECT COUNT(*) FROM context_sessions WHERE (user_id = $1 OR user_id LIKE $2) AND status = $3",
                )
                .bind(uid)
                .bind(tenant_pattern_clone)
                .bind(s)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Failed to count sessions: {}", e);
                    AppError::Internal(format!("Database error: {}", e))
                })?;
                (sessions, total)
            }
            (Some(uid), None) => {
                let sessions = sqlx::query_as::<_, Session>(
                    r#"
                    SELECT session_id, user_id, agent_id, created_at::text, updated_at::text, expires_at::text,
                           session_type, context_length, max_context_length, status, priority
                    FROM context_sessions
                    WHERE user_id = $1 OR user_id LIKE $2
                    ORDER BY created_at DESC
                    LIMIT $3 OFFSET $4
                    "#,
                )
                .bind(uid)
                .bind(tenant_pattern)
                .bind(limit)
                .bind(offset)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Failed to list sessions: {}", e);
                    AppError::Internal(format!("Database error: {}", e))
                })?;

                let total = sqlx::query_as::<_, (i64,)>(
                    "SELECT COUNT(*) FROM context_sessions WHERE user_id = $1 OR user_id LIKE $2",
                )
                .bind(uid)
                .bind(tenant_pattern_clone)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Failed to count sessions: {}", e);
                    AppError::Internal(format!("Database error: {}", e))
                })?;
                (sessions, total)
            }
            (None, Some(s)) => {
                // 没有指定user_id时，只返回属于该租户的会话
                let sessions = sqlx::query_as::<_, Session>(
                    r#"
                    SELECT session_id, user_id, agent_id, created_at::text, updated_at::text, expires_at::text,
                           session_type, context_length, max_context_length, status, priority
                    FROM context_sessions
                    WHERE user_id LIKE $1 AND status = $2
                    ORDER BY created_at DESC
                    LIMIT $3 OFFSET $4
                    "#,
                )
                .bind(tenant_pattern)
                .bind(s)
                .bind(limit)
                .bind(offset)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Failed to list sessions: {}", e);
                    AppError::Internal(format!("Database error: {}", e))
                })?;

                let total = sqlx::query_as::<_, (i64,)>(
                    "SELECT COUNT(*) FROM context_sessions WHERE user_id LIKE $1 AND status = $2",
                )
                .bind(tenant_pattern_clone)
                .bind(s)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Failed to count sessions: {}", e);
                    AppError::Internal(format!("Database error: {}", e))
                })?;
                (sessions, total)
            }
            (None, None) => {
                // 没有指定任何过滤条件时，只返回属于该租户的会话
                let sessions = sqlx::query_as::<_, Session>(
                    r#"
                    SELECT session_id, user_id, agent_id, created_at::text, updated_at::text, expires_at::text,
                           session_type, context_length, max_context_length, status, priority
                    FROM context_sessions
                    WHERE user_id LIKE $1
                    ORDER BY created_at DESC
                    LIMIT $2 OFFSET $3
                    "#,
                )
                .bind(tenant_pattern)
                .bind(limit)
                .bind(offset)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Failed to list sessions: {}", e);
                    AppError::Internal(format!("Database error: {}", e))
                })?;

                let total = sqlx::query_as::<_, (i64,)>(
                    "SELECT COUNT(*) FROM context_sessions WHERE user_id LIKE $1",
                )
                .bind(tenant_pattern_clone)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Failed to count sessions: {}", e);
                    AppError::Internal(format!("Database error: {}", e))
                })?;
                (sessions, total)
            }
        };
        tx.commit().await.ok();

        Ok(SessionListResponse {
            sessions,
            total: total.0 as usize,
            limit,
            offset,
        })
    }

    /// 获取该租户下所有活跃的 user_id 列表（租户隔离）
    ///
    /// SIGNATURE CHANGE (RLS): previously a GLOBAL cross-tenant scan with no
    /// tenant filter. Under RLS with no GUC that fail-closes to zero rows, so it
    /// now takes `&TenantId` and runs inside begin_tenant_tx — the policy scopes
    /// the DISTINCT to the caller's tenant. Both callers (reflection + transfer
    /// daemons) already own a tenant_id. This also closes a latent bleed where the
    /// per-tenant reflection cycle iterated every tenant's users.
    pub async fn get_active_user_ids(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
    ) -> Result<Vec<String>, AppError> {
        let mut tx = begin_tenant_tx(pool, tenant_id).await?;
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT user_id FROM context_sessions WHERE status = 'active'")
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Failed to get active user IDs: {}", e);
                    AppError::Internal(format!("Database error: {}", e))
                })?;
        tx.commit().await.ok();

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// 获取指定用户的活跃 agent_id 列表（租户隔离）
    pub async fn get_active_agent_ids(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        user_id: &str,
    ) -> Result<Vec<String>, AppError> {
        let prefix = tenant_id.prefix();
        let tenant_pattern = format!("{}%", prefix);

        let mut tx = begin_tenant_tx(pool, tenant_id).await?;
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT agent_id FROM context_sessions WHERE (user_id = $1 OR user_id LIKE $2) AND status = 'active'",
        )
        .bind(user_id)
        .bind(tenant_pattern)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to get active agent IDs: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;
        tx.commit().await.ok();

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// 删除会话（租户隔离）
    ///
    /// When `redis-stm` feature is enabled and Redis is available, delegates to Redis adapter.
    /// Otherwise uses PostgreSQL directly.
    pub async fn delete_session(
        pool: &sqlx::PgPool,
        tenant_id: &TenantId,
        session_id: &str,
    ) -> Result<bool, AppError> {
        #[cfg(feature = "redis-stm")]
        {
            if RedisStmAdapter::is_available() {
                return RedisStmAdapter::delete_session(tenant_id, session_id).await;
            }
        }

        // RLS: verification read + both deletes inside one tenant-scoped tx.
        let mut tx = begin_tenant_tx(pool, tenant_id).await?;

        // Verify session belongs to tenant via PostgreSQL
        let session = sqlx::query_as::<_, Session>(
            r#"
            SELECT session_id, user_id, agent_id, created_at::text, updated_at::text, expires_at::text,
                   session_type, context_length, max_context_length, status, priority
            FROM context_sessions
            WHERE session_id = $1 AND status = 'active'
            "#,
        )
        .bind(session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to get session for deletion: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        if let Some(ref s) = session {
            let prefix = tenant_id.prefix();
            let belongs_to_tenant =
                s.user_id.starts_with(&prefix) || s.user_id == tenant_id.as_str();
            if !belongs_to_tenant {
                crate::services::multi_tenant::record_isolation_violation(
                    tenant_id.as_str(),
                    session_id,
                    "stm_delete_cross_tenant_access",
                );
                return Ok(false);
            }
        } else {
            return Ok(false);
        }

        // Delete messages and the session row atomically so a failure between the two
        // can't leave orphaned messages behind a removed session.
        // Delete messages first
        sqlx::query("DELETE FROM session_messages WHERE session_id = $1")
            .bind(session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to delete session messages: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;

        // Delete session
        let result = sqlx::query("DELETE FROM context_sessions WHERE session_id = $1")
            .bind(session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to delete session: {}", e);
                AppError::Internal(format!("Database error: {}", e))
            })?;

        tx.commit().await.map_err(|e| {
            error!("Failed to commit delete_session transaction: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Deleted session: {} for tenant: {}", session_id, tenant_id);
        Ok(result.rows_affected() > 0)
    }
}

/// Count live STM sessions across **all tenants** for the operational inventory
/// gauge (`memory_stm_sessions_active`).
///
/// Deliberately cross-tenant and NOT routed through `begin_tenant_tx` — see
/// [`crate::db::ltm::count_active_entries`] for the shared RLS caveat (returns 0
/// under a hardened NOBYPASSRLS role with no tenant GUC; returns the real count
/// under today's default role).
///
/// "Active" is `status = 'active' AND expires_at > NOW()`: sessions expire by
/// their `expires_at` timestamp, and there is no sweep that flips the status
/// column, so `status = 'active'` alone would keep counting sessions whose TTL
/// has already lapsed.
pub async fn count_active_sessions(pool: &sqlx::PgPool) -> Result<i64, AppError> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM context_sessions WHERE status = 'active' AND expires_at > NOW()",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        error!("count_active_sessions failed: {}", e);
        AppError::Internal(format!("Database error: {}", e))
    })
}
