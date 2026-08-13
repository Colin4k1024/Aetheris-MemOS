use anyhow::Result;
use sqlx::{Pool, Sqlite};

use super::types::*;

pub struct SkillMatcher;

impl SkillMatcher {
    pub async fn find_matching_skills(
        pool: &Pool<Sqlite>,
        request: &SkillMatchRequest,
    ) -> Result<Vec<SkillMatchResult>> {
        // Simple keyword matching — future: vector similarity on skill descriptions
        let pattern = format!("%{}%", request.query);
        let limit = request.limit.unwrap_or(5) as i64;

        let rows: Vec<(String, String, String)> = sqlx::query_as(
            r#"SELECT id, name, description FROM skills
               WHERE tenant_id = ? AND status = 'active'
               AND (name LIKE ? OR description LIKE ? OR tags LIKE ?)
               ORDER BY usage_count DESC
               LIMIT ?"#,
        )
        .bind(&request.tenant_id)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        // TODO: full skill row parsing and scoring
        Ok(vec![])
    }
}
