use anyhow::Result;
use chrono::Utc;
use sqlx::{Pool, Sqlite};
use ulid::Ulid;

fn now_str() -> String { Utc::now().to_rfc3339() }

use super::types::*;

pub struct SkillStore;

impl SkillStore {
    pub async fn create(
        pool: &Pool<Sqlite>,
        tenant_id: &str,
        user_id: &str,
        agent_id: Option<&str>,
        req: &SkillCreateRequest,
    ) -> Result<Skill> {
        let id = Ulid::new().to_string();
        let now = now_str();
        let visibility = req.visibility.unwrap_or(Visibility::Private);
        let trigger_json = serde_json::to_string(&req.trigger_conditions)?;
        let steps_json = serde_json::to_string(&req.execution_steps)?;
        let rules_json = serde_json::to_string(&req.validation_rules)?;
        let tags_json = serde_json::to_string(&req.tags)?;
        let aid = agent_id.unwrap_or("");

        sqlx::query(
            r#"INSERT INTO skills (id, name, description, version, status, trigger_conditions, execution_steps, validation_rules, owner_user_id, owner_agent_id, tenant_id, visibility, tags, usage_count, created_at, updated_at)
               VALUES (?, ?, ?, 1, 'draft', ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)"#,
        )
        .bind(&id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(&trigger_json)
        .bind(&steps_json)
        .bind(&rules_json)
        .bind(user_id)
        .bind(aid)
        .bind(tenant_id)
        .bind(visibility.as_str())
        .bind(&tags_json)
        .bind(&now_str())
        .bind(&now_str())
        .execute(pool)
        .await?;

        Ok(Skill {
            id,
            name: req.name.clone(),
            description: req.description.clone(),
            version: 1,
            status: SkillStatus::Draft,
            trigger_conditions: req.trigger_conditions.clone(),
            execution_steps: req.execution_steps.clone(),
            validation_rules: req.validation_rules.clone(),
            owner_user_id: user_id.to_string(),
            owner_agent_id: agent_id.map(|s| s.to_string()),
            tenant_id: tenant_id.to_string(),
            visibility,
            tags: req.tags.clone(),
            usage_count: 0,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn list_by_tenant(
        pool: &Pool<Sqlite>,
        tenant_id: &str,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Skill>> {
        // Simplified query — full implementation would parse rows
        let _rows: Vec<(String, String, String)> = if let Some(status) = status {
            sqlx::query_as(
                "SELECT id, name, description FROM skills WHERE tenant_id = ? AND status = ? ORDER BY updated_at DESC LIMIT ?",
            )
            .bind(tenant_id)
            .bind(status)
            .bind(limit)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, name, description FROM skills WHERE tenant_id = ? ORDER BY updated_at DESC LIMIT ?",
            )
            .bind(tenant_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        };

        // TODO: full row parsing
        Ok(vec![])
    }

    pub async fn activate(pool: &Pool<Sqlite>, skill_id: &str) -> Result<()> {
        sqlx::query("UPDATE skills SET status = 'active', updated_at = ? WHERE id = ?")
            .bind(&now_str())
            .bind(skill_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn archive(pool: &Pool<Sqlite>, skill_id: &str) -> Result<()> {
        sqlx::query("UPDATE skills SET status = 'archived', updated_at = ? WHERE id = ?")
            .bind(&now_str())
            .bind(skill_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn increment_usage(pool: &Pool<Sqlite>, skill_id: &str) -> Result<()> {
        sqlx::query("UPDATE skills SET usage_count = usage_count + 1 WHERE id = ?")
            .bind(skill_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
