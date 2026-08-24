//! Skill Repository — tenant-scoped CRUD for the `skills` table (#90 first
//! increment). Every query runs inside a `begin_tenant_tx` so the
//! `aetheris.tenant_id` GUC is set; the RLS policy retrofitted in
//! `20260824000002_rls_skills.sql` fail-closes any path that forgets the
//! application-layer scope. The JSONB columns (trigger_conditions /
//! execution_steps / validation_rules / source_session_ids) are bound as
//! `serde_json::Value` (sqlx `json` feature).

use crate::db::tenant_scope::begin_tenant_tx;
use crate::error::AppError;
use crate::models::skill::{CreateSkillRequest, Skill, UpdateSkillRequest};
use crate::tenant::TenantId;
use sqlx::PgPool;
use ulid::Ulid;

pub struct SkillRepository {
    pool: PgPool,
}

impl SkillRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        tenant_id: &TenantId,
        req: CreateSkillRequest,
    ) -> Result<String, AppError> {
        let id = Ulid::new().to_string();
        let triggers = serde_json::to_value(&req.trigger_conditions).unwrap_or_default();
        let steps = serde_json::to_value(&req.execution_steps).unwrap_or_default();
        let rules = serde_json::to_value(&req.validation_rules).unwrap_or_default();
        let sources = serde_json::json!([]);

        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        sqlx::query(
            r#"
            INSERT INTO skills
                (id, tenant_id, name, description, version, trigger_conditions,
                 execution_steps, validation_rules, source_session_ids,
                 owner_agent_id, visibility, status)
            VALUES ($1, $2, $3, $4, 1, $5, $6, $7, $8, $9, $10, 'draft')
            "#,
        )
        .bind(&id)
        .bind(tenant_id.as_str())
        .bind(&req.name)
        .bind(&req.description)
        .bind(&triggers)
        .bind(&steps)
        .bind(&rules)
        .bind(&sources)
        .bind(&req.owner_agent_id)
        .bind(req.visibility.as_str())
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to insert skill: {e}")))?;
        tx.commit()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to commit skill: {e}")))?;
        Ok(id)
    }

    pub async fn list(
        &self,
        tenant_id: &TenantId,
        owner_agent_id: Option<&str>,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Skill>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        // Filter dynamically: tenant_id is always bound; agent/status optional.
        let rows = sqlx::query_as::<_, Skill>(
            r#"
            SELECT id, tenant_id, name, description, version, trigger_conditions,
                   execution_steps, validation_rules, source_session_ids,
                   owner_agent_id, visibility, status, embedding_model,
                   embedding_dimension, created_at::text, updated_at::text
            FROM skills
            WHERE tenant_id = $1
              AND ($2 IS NULL OR owner_agent_id = $2)
              AND ($3 IS NULL OR status = $3)
            ORDER BY updated_at DESC
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(owner_agent_id)
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list skills: {e}")))?;
        tx.commit().await.ok();
        Ok(rows)
    }

    pub async fn get(&self, tenant_id: &TenantId, id: &str) -> Result<Option<Skill>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let row = sqlx::query_as::<_, Skill>(
            r#"
            SELECT id, tenant_id, name, description, version, trigger_conditions,
                   execution_steps, validation_rules, source_session_ids,
                   owner_agent_id, visibility, status, embedding_model,
                   embedding_dimension, created_at::text, updated_at::text
            FROM skills
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get skill: {e}")))?;
        tx.commit().await.ok();
        Ok(row)
    }

    /// Partial update of a skill's mutable metadata. `status` transitions
    /// (draft→active→deprecated) go through here. COALESCE keeps unspecified
    /// fields. Bumping `version` (publishing a new revision) is a follow-up.
    pub async fn update(
        &self,
        tenant_id: &TenantId,
        id: &str,
        req: UpdateSkillRequest,
    ) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let result = sqlx::query(
            r#"
            UPDATE skills
            SET description = COALESCE($3, description),
                visibility  = COALESCE($4, visibility),
                status      = COALESCE($5, status),
                updated_at  = NOW()
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(id)
        .bind(req.description)
        .bind(req.visibility.map(|v| v.as_str()))
        .bind(req.status.map(|s| s.as_str()))
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to update skill: {e}")))?;
        let affected = result.rows_affected();
        tx.commit()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to commit skill: {e}")))?;
        Ok(affected > 0)
    }

    pub async fn delete(&self, tenant_id: &TenantId, id: &str) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let result = sqlx::query("DELETE FROM skills WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id.as_str())
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to delete skill: {e}")))?;
        let affected = result.rows_affected();
        tx.commit()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to commit skill: {e}")))?;
        Ok(affected > 0)
    }
}
