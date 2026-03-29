//! Redis adapter for Short-Term Memory (STM) cache.
//!
//! This adapter provides a Redis-backed implementation of STM session storage
//! with automatic TTL-based expiry for temporary session data.

use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::tenant::TenantId;
use crate::AppError;

/// Redis connection manager for STM sessions.
static REDIS_STM_POOL: std::sync::OnceLock<ConnectionManager> = std::sync::OnceLock::new();

/// Redis STM configuration.
#[derive(Debug, Clone)]
pub struct RedisStmConfig {
    pub url: String,
    pub pool_size: usize,
    pub default_ttl_seconds: u64,
}

impl Default for RedisStmConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
            default_ttl_seconds: 3600, // 1 hour default TTL
        }
    }
}

/// Initialize Redis connection pool for STM.
pub async fn init_redis_stm(config: &RedisStmConfig) -> Result<(), AppError> {
    info!("Connecting to Redis for STM: {}", config.url);

    let client = redis::Client::open(config.url.as_str()).map_err(|e| {
        error!("Redis connection failed: {}", e);
        AppError::Internal(format!("Redis connection failed: {}", e))
    })?;

    let manager = ConnectionManager::new(client).await.map_err(|e| {
        error!("Redis connection manager creation failed: {}", e);
        AppError::Internal(format!("Redis connection manager creation failed: {}", e))
    })?;

    REDIS_STM_POOL
        .set(manager)
        .map_err(|_| AppError::Internal("Redis STM pool already initialized".to_string()))?;

    info!("Redis STM connection pool initialized");
    Ok(())
}

/// Get the Redis connection for STM operations.
fn get_redis_conn() -> Result<&'static ConnectionManager, AppError> {
    REDIS_STM_POOL.get().ok_or_else(|| {
        AppError::Internal("Redis STM not initialized. Call init_redis_stm() first.".to_string())
    })
}

/// Build a tenant-scoped session key.
///
/// Pattern: `stm:{tenant_id}:session:{session_id}`
fn build_session_key(tenant_id: &TenantId, session_id: &str) -> String {
    format!("stm:{}:session:{}", tenant_id.prefix(), session_id)
}

/// Build a tenant-scoped messages key.
///
/// Pattern: `stm:{tenant_id}:messages:{session_id}`
fn build_messages_key(tenant_id: &TenantId, session_id: &str) -> String {
    format!("stm:{}:messages:{}", tenant_id.prefix(), session_id)
}

/// Session metadata stored in Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisSession {
    pub session_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub session_type: String,
    pub max_context_length: i32,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
    pub status: String,
    pub priority: i32,
}

/// Message stored in Redis session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisMessage {
    pub message_id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub token_count: Option<i32>,
    pub importance_score: Option<f32>,
}

/// Redis STM adapter for session and message operations.
pub struct RedisStmAdapter;

impl RedisStmAdapter {
    /// Create a new session with TTL-based expiry.
    ///
    /// # Arguments
    /// * `tenant_id` - Tenant identifier for multi-tenant isolation
    /// * `user_id` - User identifier
    /// * `agent_id` - Agent identifier
    /// * `session_type` - Type of session
    /// * `max_context_length` - Maximum context length
    /// * `retention_hours` - Session retention time in hours (used for TTL)
    ///
    /// # Returns
    /// * `Ok(String)` - The created session ID
    pub async fn create_session(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        session_type: &str,
        max_context_length: i32,
        retention_hours: i32,
    ) -> Result<String, AppError> {
        let conn = get_redis_conn()?;
        let session_id = ulid::Ulid::new().to_string();

        let now = chrono::Utc::now();
        let expires_at = now + chrono::Duration::hours(retention_hours as i64);

        let session = RedisSession {
            session_id: session_id.clone(),
            user_id: user_id.to_string(),
            agent_id: agent_id.to_string(),
            session_type: session_type.to_string(),
            max_context_length,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
            expires_at: expires_at.to_rfc3339(),
            status: "active".to_string(),
            priority: 5,
        };

        let session_key = build_session_key(tenant_id, &session_id);
        let ttl_seconds = (retention_hours as u64) * 3600;

        let mut conn = conn.clone();
        let serialized: String = serde_json::to_string(&session).map_err(|e| {
            error!("Failed to serialize session: {}", e);
            AppError::Serialization(format!("Failed to serialize session: {}", e))
        })?;

        let _: () = redis::cmd("SETEX")
            .arg(&session_key)
            .arg(ttl_seconds)
            .arg(&serialized)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                error!("Failed to create session in Redis: {}", e);
                AppError::Internal(format!("Redis error: {}", e))
            })?;

        // Initialize empty messages list with same TTL
        let messages_key = build_messages_key(tenant_id, &session_id);
        let _: () = redis::cmd("SETEX")
            .arg(&messages_key)
            .arg(ttl_seconds)
            .arg("[]")
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                error!("Failed to initialize messages list in Redis: {}", e);
                AppError::Internal(format!("Redis error: {}", e))
            })?;

        info!(
            "Created Redis STM session: {} for tenant: {} (TTL: {}s)",
            session_id, tenant_id, ttl_seconds
        );

        Ok(session_id)
    }

    /// Get session metadata from Redis.
    ///
    /// # Arguments
    /// * `tenant_id` - Tenant identifier
    /// * `session_id` - Session identifier
    ///
    /// # Returns
    /// * `Ok(Option<RedisSession>)` - Session if found
    pub async fn get_session(
        tenant_id: &TenantId,
        session_id: &str,
    ) -> Result<Option<RedisSession>, AppError> {
        let conn = get_redis_conn()?;
        let session_key = build_session_key(tenant_id, session_id);

        let mut conn = conn.clone();
        let result: Option<String> = conn.get(&session_key).await.map_err(|e| {
            error!("Failed to get session from Redis: {}", e);
            AppError::Internal(format!("Redis error: {}", e))
        })?;

        match result {
            Some(data) => {
                let session: RedisSession = serde_json::from_str(&data).map_err(|e| {
                    error!("Failed to deserialize session: {}", e);
                    AppError::Deserialization(format!("Failed to deserialize session: {}", e))
                })?;

                // Verify tenant ownership
                let prefix = tenant_id.prefix();
                let belongs_to_tenant =
                    session.user_id.starts_with(&prefix) || session.user_id == tenant_id.as_str();
                if !belongs_to_tenant {
                    warn!(
                        "Cross-tenant access attempt: tenant {} accessing session {}",
                        tenant_id, session_id
                    );
                    crate::services::multi_tenant::record_isolation_violation(
                        tenant_id.as_str(),
                        session_id,
                        "redis_stm_session_cross_tenant_access",
                    );
                    return Ok(None);
                }

                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    /// Append a message to a session.
    ///
    /// # Arguments
    /// * `tenant_id` - Tenant identifier
    /// * `session_id` - Session identifier
    /// * `role` - Message role (e.g., "user", "assistant")
    /// * `content` - Message content
    /// * `token_count` - Optional token count
    /// * `importance_score` - Optional importance score
    ///
    /// # Returns
    /// * `Ok(String)` - The created message ID
    pub async fn append_message(
        tenant_id: &TenantId,
        session_id: &str,
        role: &str,
        content: &str,
        token_count: Option<i32>,
        importance_score: Option<f64>,
    ) -> Result<String, AppError> {
        // First verify the session belongs to this tenant
        let session = Self::get_session(tenant_id, session_id).await?;
        if session.is_none() {
            return Err(AppError::NotFound("Session not found".to_string()));
        }

        let conn = get_redis_conn()?;
        let messages_key = build_messages_key(tenant_id, session_id);

        let message_id = ulid::Ulid::new().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        let message = RedisMessage {
            message_id: message_id.clone(),
            session_id: session_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            created_at: now,
            token_count,
            importance_score: importance_score.map(|s| s as f32),
        };

        let mut conn = conn.clone();

        // Get existing messages
        let existing: Option<String> = conn.get(&messages_key).await.map_err(|e| {
            error!("Failed to get messages from Redis: {}", e);
            AppError::Internal(format!("Redis error: {}", e))
        })?;

        let mut messages: Vec<RedisMessage> = match existing {
            Some(data) => serde_json::from_str(&data).unwrap_or_default(),
            None => vec![],
        };

        messages.push(message);

        let serialized: String = serde_json::to_string(&messages).map_err(|e| {
            error!("Failed to serialize messages: {}", e);
            AppError::Serialization(format!("Failed to serialize messages: {}", e))
        })?;

        // Get TTL from session to apply to messages
        let ttl: i64 = conn
            .ttl(&build_session_key(tenant_id, session_id))
            .await
            .unwrap_or(3600);

        if ttl > 0 {
            let _: () = redis::cmd("SETEX")
                .arg(&messages_key)
                .arg(ttl)
                .arg(&serialized)
                .query_async(&mut conn)
                .await
                .map_err(|e| {
                    error!("Failed to append message in Redis: {}", e);
                    AppError::Internal(format!("Redis error: {}", e))
                })?;
        } else {
            // Session expired, cannot append
            return Err(AppError::NotFound("Session expired".to_string()));
        }

        // Update session's updated_at timestamp
        let session_key = build_session_key(tenant_id, session_id);
        if let Ok(Some(data)) = {
            let mut c = conn.clone();
            c.get::<_, Option<String>>(&session_key).await
        } {
            if let Ok(mut session) = serde_json::from_str::<RedisSession>(&data) {
                session.updated_at = chrono::Utc::now().to_rfc3339();
                if let Ok(serialized) = serde_json::to_string(&session) {
                    let _: Option<String> = conn.set_ex(&session_key, serialized, ttl as u64).await;
                }
            }
        }

        info!(
            "Appended message {} to session {} for tenant: {}",
            message_id, session_id, tenant_id
        );

        Ok(message_id)
    }

    /// Get all messages from a session.
    ///
    /// # Arguments
    /// * `tenant_id` - Tenant identifier
    /// * `session_id` - Session identifier
    /// * `limit` - Optional limit on number of messages to return
    ///
    /// # Returns
    /// * `Ok(Vec<RedisMessage>)` - List of messages
    pub async fn get_session_messages(
        tenant_id: &TenantId,
        session_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<RedisMessage>, AppError> {
        // First verify the session belongs to this tenant
        let session = Self::get_session(tenant_id, session_id).await?;
        if session.is_none() {
            return Ok(vec![]);
        }

        let conn = get_redis_conn()?;
        let messages_key = build_messages_key(tenant_id, session_id);

        let mut conn = conn.clone();
        let result: Option<String> = conn.get(&messages_key).await.map_err(|e| {
            error!("Failed to get messages from Redis: {}", e);
            AppError::Internal(format!("Redis error: {}", e))
        })?;

        let messages: Vec<RedisMessage> = match result {
            Some(data) => serde_json::from_str(&data).unwrap_or_default(),
            None => vec![],
        };

        let limit = limit.unwrap_or(100) as usize;
        let messages = messages.into_iter().take(limit).collect();

        Ok(messages)
    }

    /// Delete a session and its messages.
    ///
    /// # Arguments
    /// * `tenant_id` - Tenant identifier
    /// * `session_id` - Session identifier
    ///
    /// # Returns
    /// * `Ok(bool)` - True if session was deleted
    pub async fn delete_session(tenant_id: &TenantId, session_id: &str) -> Result<bool, AppError> {
        // First verify the session belongs to this tenant
        let session = Self::get_session(tenant_id, session_id).await?;
        if session.is_none() {
            return Ok(false);
        }

        let conn = get_redis_conn()?;
        let session_key = build_session_key(tenant_id, session_id);
        let messages_key = build_messages_key(tenant_id, session_id);

        let mut conn = conn.clone();

        // Delete both keys
        let deleted: i64 = redis::cmd("DEL")
            .arg(&session_key)
            .arg(&messages_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                error!("Failed to delete session from Redis: {}", e);
                AppError::Internal(format!("Redis error: {}", e))
            })?;

        info!(
            "Deleted Redis STM session: {} for tenant: {} (keys deleted: {})",
            session_id, tenant_id, deleted
        );

        Ok(deleted > 0)
    }

    /// Check if Redis STM is initialized and available.
    pub fn is_available() -> bool {
        REDIS_STM_POOL.get().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_session_key() {
        let tenant_id = TenantId::from_string("test-tenant");
        let session_id = "session123";

        let key = build_session_key(&tenant_id, session_id);
        assert_eq!(key, "stm:t:test-tenant:session:session123");
    }

    #[test]
    fn test_build_messages_key() {
        let tenant_id = TenantId::from_string("test-tenant");
        let session_id = "session123";

        let key = build_messages_key(&tenant_id, session_id);
        assert_eq!(key, "stm:t:test-tenant:messages:session123");
    }

    #[test]
    fn test_redis_session_serialization() {
        let session = RedisSession {
            session_id: "test-id".to_string(),
            user_id: "user-1".to_string(),
            agent_id: "agent-1".to_string(),
            session_type: "chat".to_string(),
            max_context_length: 4096,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            expires_at: "2024-01-02T00:00:00Z".to_string(),
            status: "active".to_string(),
            priority: 5,
        };

        let serialized = serde_json::to_string(&session).unwrap();
        let deserialized: RedisSession = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.session_id, session.session_id);
        assert_eq!(deserialized.user_id, session.user_id);
    }

    #[test]
    fn test_redis_message_serialization() {
        let message = RedisMessage {
            message_id: "msg-1".to_string(),
            session_id: "session-1".to_string(),
            role: "user".to_string(),
            content: "Hello world".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            token_count: Some(100),
            importance_score: Some(0.8),
        };

        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: RedisMessage = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.message_id, message.message_id);
        assert_eq!(deserialized.content, message.content);
        assert_eq!(deserialized.token_count, Some(100));
    }
}
