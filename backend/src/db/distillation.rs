use serde_json::json;
use tracing::{error, info};
use ulid::Ulid;

use crate::db::pool;
use crate::models::distillation::{DistillationJob, L1Atom, L2Scene, L3Persona};
use crate::tenant::TenantId;
use crate::AppError;

pub struct DistillationRepository;

impl DistillationRepository {
    // === L1 Atom operations ===

    pub async fn create_atom(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        atom_type: &str,
        scene_name: &str,
        content: &str,
        priority: f32,
        source_session_id: &str,
        source_message_ids: &[String],
        metadata: &serde_json::Value,
    ) -> Result<String, AppError> {
        let id = Ulid::new().to_string();
        let pool = pool();
        let msg_ids_json = serde_json::to_value(source_message_ids).unwrap_or(json!([]));

        sqlx::query(
            r#"
            INSERT INTO distillation_atoms (
                id, tenant_id, user_id, agent_id, atom_type, scene_name,
                content, priority, source_session_id, source_message_ids, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(&id)
        .bind(tenant_id.as_str())
        .bind(user_id)
        .bind(agent_id)
        .bind(atom_type)
        .bind(scene_name)
        .bind(content)
        .bind(priority)
        .bind(source_session_id)
        .bind(&msg_ids_json)
        .bind(metadata)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create atom: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!(
            "Created distillation atom: id={}, type={}, scene={}",
            id, atom_type, scene_name
        );
        Ok(id)
    }

    pub async fn list_atoms(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        atom_type: Option<&str>,
        scene_name: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<L1Atom>, AppError> {
        let pool = pool();

        // Build dynamic query based on filters
        let mut query = String::from(
            "SELECT * FROM distillation_atoms WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND is_active = TRUE",
        );
        let mut param_idx = 4;

        if atom_type.is_some() {
            query.push_str(&format!(" AND atom_type = ${}", param_idx));
            param_idx += 1;
        }
        if scene_name.is_some() {
            query.push_str(&format!(" AND scene_name = ${}", param_idx));
            param_idx += 1;
        }

        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, L1Atom>(&query)
            .bind(tenant_id.as_str())
            .bind(user_id)
            .bind(agent_id);

        if let Some(at) = atom_type {
            q = q.bind(at);
        }
        if let Some(sn) = scene_name {
            q = q.bind(sn);
        }

        q = q.bind(limit).bind(offset);

        q.fetch_all(pool).await.map_err(|e| {
            error!("Failed to list atoms: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })
    }

    pub async fn get_atom(id: &str, tenant_id: &TenantId) -> Result<Option<L1Atom>, AppError> {
        let pool = pool();
        sqlx::query_as::<_, L1Atom>(
            "SELECT * FROM distillation_atoms WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(tenant_id.as_str())
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to get atom: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })
    }

    pub async fn deactivate_atom(id: &str, superseded_by: &str) -> Result<(), AppError> {
        let pool = pool();
        sqlx::query(
            "UPDATE distillation_atoms SET is_active = FALSE, superseded_by = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(superseded_by)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to deactivate atom: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;
        Ok(())
    }

    pub async fn count_atoms_for_scene(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        scene_name: &str,
    ) -> Result<i64, AppError> {
        let pool = pool();
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM distillation_atoms WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND scene_name = $4 AND is_active = TRUE",
        )
        .bind(tenant_id.as_str())
        .bind(user_id)
        .bind(agent_id)
        .bind(scene_name)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Failed to count atoms: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;
        Ok(row.0)
    }

    pub async fn get_atoms_for_scene(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        scene_name: &str,
    ) -> Result<Vec<L1Atom>, AppError> {
        let pool = pool();
        sqlx::query_as::<_, L1Atom>(
            "SELECT * FROM distillation_atoms WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND scene_name = $4 AND is_active = TRUE ORDER BY priority DESC, created_at ASC",
        )
        .bind(tenant_id.as_str())
        .bind(user_id)
        .bind(agent_id)
        .bind(scene_name)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to get atoms for scene: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })
    }

    // === L2 Scene operations ===

    pub async fn upsert_scene(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        scene_name: &str,
        title: &str,
        content: &str,
        atom_ids: &[String],
        token_count: i32,
    ) -> Result<String, AppError> {
        let id = Ulid::new().to_string();
        let pool = pool();
        let atom_ids_json = serde_json::to_value(atom_ids).unwrap_or(json!([]));

        sqlx::query(
            r#"
            INSERT INTO distillation_scenes (id, tenant_id, user_id, agent_id, scene_name, title, content, atom_ids, token_count)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (tenant_id, user_id, agent_id, scene_name)
            DO UPDATE SET title = EXCLUDED.title, content = EXCLUDED.content,
                          atom_ids = EXCLUDED.atom_ids, token_count = EXCLUDED.token_count,
                          version = distillation_scenes.version + 1, updated_at = NOW()
            "#,
        )
        .bind(&id)
        .bind(tenant_id.as_str())
        .bind(user_id)
        .bind(agent_id)
        .bind(scene_name)
        .bind(title)
        .bind(content)
        .bind(&atom_ids_json)
        .bind(token_count)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to upsert scene: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(id)
    }

    pub async fn list_scenes(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
    ) -> Result<Vec<L2Scene>, AppError> {
        let pool = pool();
        sqlx::query_as::<_, L2Scene>(
            "SELECT * FROM distillation_scenes WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 ORDER BY updated_at DESC",
        )
        .bind(tenant_id.as_str())
        .bind(user_id)
        .bind(agent_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("Failed to list scenes: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })
    }

    pub async fn get_scene(id: &str, tenant_id: &TenantId) -> Result<Option<L2Scene>, AppError> {
        let pool = pool();
        sqlx::query_as::<_, L2Scene>(
            "SELECT * FROM distillation_scenes WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(tenant_id.as_str())
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to get scene: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })
    }

    // === L3 Persona operations ===

    pub async fn upsert_persona(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        profile_content: &str,
        scene_ids: &[String],
        token_count: i32,
    ) -> Result<String, AppError> {
        let id = Ulid::new().to_string();
        let pool = pool();
        let scene_ids_json = serde_json::to_value(scene_ids).unwrap_or(json!([]));

        sqlx::query(
            r#"
            INSERT INTO distillation_personas (id, tenant_id, user_id, agent_id, profile_content, scene_ids, token_count)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (tenant_id, user_id, agent_id)
            DO UPDATE SET profile_content = EXCLUDED.profile_content, scene_ids = EXCLUDED.scene_ids,
                          token_count = EXCLUDED.token_count, version = distillation_personas.version + 1,
                          updated_at = NOW()
            "#,
        )
        .bind(&id)
        .bind(tenant_id.as_str())
        .bind(user_id)
        .bind(agent_id)
        .bind(profile_content)
        .bind(&scene_ids_json)
        .bind(token_count)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to upsert persona: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(id)
    }

    pub async fn get_persona(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
    ) -> Result<Option<L3Persona>, AppError> {
        let pool = pool();
        sqlx::query_as::<_, L3Persona>(
            "SELECT * FROM distillation_personas WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3",
        )
        .bind(tenant_id.as_str())
        .bind(user_id)
        .bind(agent_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to get persona: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })
    }

    // === Job operations ===

    pub async fn create_job(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        session_id: &str,
        job_type: &str,
    ) -> Result<String, AppError> {
        let id = Ulid::new().to_string();
        let pool = pool();

        sqlx::query(
            r#"
            INSERT INTO distillation_jobs (id, tenant_id, user_id, agent_id, session_id, job_type)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&id)
        .bind(tenant_id.as_str())
        .bind(user_id)
        .bind(agent_id)
        .bind(session_id)
        .bind(job_type)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create job: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        info!("Created distillation job: id={}, type={}", id, job_type);
        Ok(id)
    }

    pub async fn claim_next_job(
        tenant_id: &TenantId,
    ) -> Result<Option<DistillationJob>, AppError> {
        let pool = pool();
        // Atomic claim: select + update in one query
        sqlx::query_as::<_, DistillationJob>(
            r#"
            UPDATE distillation_jobs SET status = 'running', started_at = NOW()
            WHERE id = (
                SELECT id FROM distillation_jobs
                WHERE tenant_id = $1 AND status = 'pending'
                ORDER BY created_at ASC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING *
            "#,
        )
        .bind(tenant_id.as_str())
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Failed to claim job: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })
    }

    pub async fn complete_job(id: &str, atoms_created: i32) -> Result<(), AppError> {
        let pool = pool();
        sqlx::query(
            "UPDATE distillation_jobs SET status = 'completed', atoms_created = $2, completed_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(atoms_created)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to complete job: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;
        Ok(())
    }

    pub async fn fail_job(id: &str, error_message: &str) -> Result<(), AppError> {
        let pool = pool();
        sqlx::query(
            "UPDATE distillation_jobs SET status = 'failed', error_message = $2, completed_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(error_message)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to mark job as failed: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;
        Ok(())
    }

    pub async fn list_jobs(
        tenant_id: &TenantId,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<DistillationJob>, AppError> {
        let pool = pool();
        let query = if let Some(s) = status {
            sqlx::query_as::<_, DistillationJob>(
                "SELECT * FROM distillation_jobs WHERE tenant_id = $1 AND status = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4",
            )
            .bind(tenant_id.as_str())
            .bind(s)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_as::<_, DistillationJob>(
                "SELECT * FROM distillation_jobs WHERE tenant_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
            )
            .bind(tenant_id.as_str())
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        };

        query.map_err(|e| {
            error!("Failed to list jobs: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })
    }
}
