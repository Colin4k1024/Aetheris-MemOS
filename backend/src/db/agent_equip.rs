//! Agent Equipment Repository — tenant-scoped CRUD for the `agent_equipment`
//! binding table (#89). Every query runs inside a `begin_tenant_tx` so the
//! `aetheris.tenant_id` GUC is set; the RLS policy retrofitted in
//! `20260824000001_rls_agent_equipment.sql` fail-closes any path that forgets
//! the application-layer scope.

use crate::db::tenant_scope::begin_tenant_tx;
use crate::error::AppError;
use crate::models::agent_equip::{AgentEquipment, CreateEquipmentRequest, UpdateEquipmentRequest};
use crate::tenant::TenantId;
use sqlx::PgPool;
use ulid::Ulid;

pub struct AgentEquipmentRepository {
    pool: PgPool,
}

impl AgentEquipmentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Bind an asset to an agent (tenant-scoped).
    pub async fn create(
        &self,
        tenant_id: &TenantId,
        agent_id: &str,
        req: CreateEquipmentRequest,
    ) -> Result<String, AppError> {
        let id = Ulid::new().to_string();
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        sqlx::query(
            r#"
            INSERT INTO agent_equipment
                (id, tenant_id, agent_id, asset_type, asset_id, binding_type, visibility, condition, priority)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&id)
        .bind(tenant_id.as_str())
        .bind(agent_id)
        .bind(req.asset_type.as_str())
        .bind(&req.asset_id)
        .bind(req.binding_type.as_str())
        .bind(req.visibility.as_str())
        .bind(req.condition)
        .bind(req.priority)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to insert agent_equipment: {e}")))?;
        tx.commit()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to commit agent_equipment: {e}")))?;
        Ok(id)
    }

    /// List all equipment bound to an agent (tenant-scoped).
    pub async fn list_by_agent(
        &self,
        tenant_id: &TenantId,
        agent_id: &str,
    ) -> Result<Vec<AgentEquipment>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let rows = sqlx::query_as::<_, AgentEquipment>(
            r#"
            SELECT id, tenant_id, agent_id, asset_type, asset_id, binding_type,
                   visibility, condition, priority, created_at::text
            FROM agent_equipment
            WHERE tenant_id = $1 AND agent_id = $2
            ORDER BY priority DESC, created_at ASC
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(agent_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list agent_equipment: {e}")))?;
        tx.commit().await.ok();
        Ok(rows)
    }

    /// Get a single equipment binding by id (tenant-scoped).
    pub async fn get(
        &self,
        tenant_id: &TenantId,
        id: &str,
    ) -> Result<Option<AgentEquipment>, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let row = sqlx::query_as::<_, AgentEquipment>(
            r#"
            SELECT id, tenant_id, agent_id, asset_type, asset_id, binding_type,
                   visibility, condition, priority, created_at::text
            FROM agent_equipment
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get agent_equipment: {e}")))?;
        tx.commit().await.ok();
        Ok(row)
    }

    /// Partial update: only the provided fields are changed (COALESCE keeps
    /// the rest). Note: this cannot set a field back to NULL — distinguishing
    /// "not provided" from "clear" needs a sentinel (follow-up).
    pub async fn update(
        &self,
        tenant_id: &TenantId,
        id: &str,
        req: UpdateEquipmentRequest,
    ) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let result = sqlx::query(
            r#"
            UPDATE agent_equipment
            SET binding_type = COALESCE($3, binding_type),
                visibility   = COALESCE($4, visibility),
                condition    = COALESCE($5, condition),
                priority     = COALESCE($6, priority)
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(id)
        .bind(req.binding_type.map(|b| b.as_str()))
        .bind(req.visibility.map(|v| v.as_str()))
        .bind(req.condition)
        .bind(req.priority)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to update agent_equipment: {e}")))?;
        let affected = result.rows_affected();
        tx.commit()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to commit agent_equipment: {e}")))?;
        Ok(affected > 0)
    }

    /// Delete an equipment binding (tenant-scoped). Returns false if the row
    /// did not exist (or belonged to another tenant — indistinguishable by
    /// design, to avoid leaking existence).
    pub async fn delete(&self, tenant_id: &TenantId, id: &str) -> Result<bool, AppError> {
        let mut tx = begin_tenant_tx(&self.pool, tenant_id).await?;
        let result =
            sqlx::query("DELETE FROM agent_equipment WHERE tenant_id = $1 AND id = $2")
                .bind(tenant_id.as_str())
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to delete agent_equipment: {e}")))?;
        let affected = result.rows_affected();
        tx.commit()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to commit agent_equipment: {e}")))?;
        Ok(affected > 0)
    }
}
