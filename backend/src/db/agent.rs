//! Agent Repository - Database operations for agent identity and self-model

use crate::models::agent::*;
use crate::error::AppError;
use sqlx::{PgPool, Row};
use ulid::Ulid;

// ============================================================================
// Agent Identity Repository
// ============================================================================

pub struct AgentRepository {
    pool: PgPool,
}

impl AgentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new agent identity
    pub async fn create(&self, input: CreateAgentIdentity) -> Result<AgentIdentity, AppError> {
        let agent_id = input.agent_id.unwrap_or_else(|| Ulid::new().to_string());
        let now = chrono::Utc::now().to_rfc3339();
        let agent_type = input.agent_type.unwrap_or_else(|| "general".to_string());
        let version = input.version.unwrap_or_else(|| "1.0.0".to_string());

        let result = sqlx::query(
            r#"
            INSERT INTO agent_identities
                (agent_id, agent_name, agent_type, version, capabilities, description,
                 personality_traits, system_prompt, core_directives, memory_config,
                 created_at, updated_at, status, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING *
            "#
        )
        .bind(&agent_id)
        .bind(&input.agent_name)
        .bind(&agent_type)
        .bind(&version)
        .bind(&input.capabilities.unwrap_or(serde_json::json!({})))
        .bind(&input.description)
        .bind(&input.personality_traits)
        .bind(&input.system_prompt)
        .bind(&input.core_directives.unwrap_or(serde_json::json!([])))
        .bind(&input.memory_config.unwrap_or(serde_json::json!({})))
        .bind(&now)
        .bind(&now)
        .bind("active")
        .bind(&input.metadata.unwrap_or(serde_json::json!({})))
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

        Ok(AgentIdentity {
            agent_id: result.get("agent_id"),
            agent_name: result.get("agent_name"),
            agent_type: result.get("agent_type"),
            version: result.get("version"),
            capabilities: result.get("capabilities"),
            description: result.get("description"),
            personality_traits: result.get("personality_traits"),
            system_prompt: result.get("system_prompt"),
            core_directives: result.get("core_directives"),
            memory_config: result.get("memory_config"),
            created_at: result.get("created_at"),
            updated_at: result.get("updated_at"),
            status: result.get("status"),
            metadata: result.get("metadata"),
        })
    }

    /// Get agent by ID
    pub async fn get_by_id(&self, agent_id: &str) -> Result<Option<AgentIdentity>, AppError> {
        let result = sqlx::query("SELECT * FROM agent_identities WHERE agent_id = $1")
            .bind(agent_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;

        match result {
            Some(row) => Ok(Some(AgentIdentity {
                agent_id: row.get("agent_id"),
                agent_name: row.get("agent_name"),
                agent_type: row.get("agent_type"),
                version: row.get("version"),
                capabilities: row.get("capabilities"),
                description: row.get("description"),
                personality_traits: row.get("personality_traits"),
                system_prompt: row.get("system_prompt"),
                core_directives: row.get("core_directives"),
                memory_config: row.get("memory_config"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                status: row.get("status"),
                metadata: row.get("metadata"),
            })),
            None => Ok(None),
        }
    }

    /// List all agents with pagination
    pub async fn list(&self, limit: i64, offset: i64) -> Result<AgentListResponse, AppError> {
        let agents: Vec<AgentIdentity> = sqlx::query(
            "SELECT * FROM agent_identities ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .into_iter()
        .map(|row| AgentIdentity {
            agent_id: row.get("agent_id"),
            agent_name: row.get("agent_name"),
            agent_type: row.get("agent_type"),
            version: row.get("version"),
            capabilities: row.get("capabilities"),
            description: row.get("description"),
            personality_traits: row.get("personality_traits"),
            system_prompt: row.get("system_prompt"),
            core_directives: row.get("core_directives"),
            memory_config: row.get("memory_config"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            status: row.get("status"),
            metadata: row.get("metadata"),
        })
        .collect();

        let total: i64 = sqlx::query("SELECT COUNT(*) FROM agent_identities")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?
            .get("count");

        Ok(AgentListResponse { agents, total })
    }

    /// Update agent identity
    pub async fn update(
        &self,
        agent_id: &str,
        input: UpdateAgentIdentity,
    ) -> Result<AgentIdentity, AppError> {
        let now = chrono::Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE agent_identities
            SET agent_name = COALESCE($2, agent_name),
                capabilities = COALESCE($3, capabilities),
                description = COALESCE($4, description),
                personality_traits = COALESCE($5, personality_traits),
                system_prompt = COALESCE($6, system_prompt),
                core_directives = COALESCE($7, core_directives),
                memory_config = COALESCE($8, memory_config),
                status = COALESCE($9, status),
                metadata = COALESCE($10, metadata),
                updated_at = $11
            WHERE agent_id = $1
            RETURNING *
            "#
        )
        .bind(agent_id)
        .bind(&input.agent_name)
        .bind(&input.capabilities)
        .bind(&input.description)
        .bind(&input.personality_traits)
        .bind(&input.system_prompt)
        .bind(&input.core_directives)
        .bind(&input.memory_config)
        .bind(&input.status)
        .bind(&input.metadata)
        .bind(&now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

        Ok(AgentIdentity {
            agent_id: result.get("agent_id"),
            agent_name: result.get("agent_name"),
            agent_type: result.get("agent_type"),
            version: result.get("version"),
            capabilities: result.get("capabilities"),
            description: result.get("description"),
            personality_traits: result.get("personality_traits"),
            system_prompt: result.get("system_prompt"),
            core_directives: result.get("core_directives"),
            memory_config: result.get("memory_config"),
            created_at: result.get("created_at"),
            updated_at: result.get("updated_at"),
            status: result.get("status"),
            metadata: result.get("metadata"),
        })
    }

    /// Delete agent
    pub async fn delete(&self, agent_id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM agent_identities WHERE agent_id = $1")
            .bind(agent_id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        Ok(())
    }
}

// ============================================================================
// Agent Capability Repository
// ============================================================================

pub struct AgentCapabilityRepository {
    pool: PgPool,
}

impl AgentCapabilityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new capability
    pub async fn create(&self, agent_id: &str, input: CreateAgentCapability) -> Result<AgentCapability, AppError> {
        let capability_id = input.capability_id.unwrap_or_else(|| Ulid::new().to_string());
        let now = chrono::Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            INSERT INTO agent_capabilities
                (capability_id, agent_id, capability_name, capability_type, description,
                 implementation_type, implementation_ref, success_rate, avg_latency_ms,
                 times_invoked, max_tokens, timeout_ms, enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING *
            "#
        )
        .bind(&capability_id)
        .bind(agent_id)
        .bind(&input.capability_name)
        .bind(&input.capability_type)
        .bind(&input.description)
        .bind(&input.implementation_type)
        .bind(&input.implementation_ref)
        .bind(0.0)
        .bind::<Option<i32>>(None)
        .bind(0)
        .bind::<Option<i32>>(None)
        .bind::<Option<i32>>(None)
        .bind(true)
        .bind(&now)
        .bind(&now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

        Ok(AgentCapability {
            capability_id: result.get("capability_id"),
            agent_id: result.get("agent_id"),
            capability_name: result.get("capability_name"),
            capability_type: result.get("capability_type"),
            description: result.get("description"),
            implementation_type: result.get("implementation_type"),
            implementation_ref: result.get("implementation_ref"),
            success_rate: result.get("success_rate"),
            avg_latency_ms: result.get("avg_latency_ms"),
            times_invoked: result.get("times_invoked"),
            max_tokens: result.get("max_tokens"),
            timeout_ms: result.get("timeout_ms"),
            enabled: result.get("enabled"),
            created_at: result.get("created_at"),
            updated_at: result.get("updated_at"),
        })
    }

    /// List capabilities for an agent
    pub async fn list_by_agent(&self, agent_id: &str) -> Result<Vec<AgentCapability>, AppError> {
        let rows = sqlx::query("SELECT * FROM agent_capabilities WHERE agent_id = $1")
            .bind(agent_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;

        Ok(rows.into_iter().map(|row| AgentCapability {
            capability_id: row.get("capability_id"),
            agent_id: row.get("agent_id"),
            capability_name: row.get("capability_name"),
            capability_type: row.get("capability_type"),
            description: row.get("description"),
            implementation_type: row.get("implementation_type"),
            implementation_ref: row.get("implementation_ref"),
            success_rate: row.get("success_rate"),
            avg_latency_ms: row.get("avg_latency_ms"),
            times_invoked: row.get("times_invoked"),
            max_tokens: row.get("max_tokens"),
            timeout_ms: row.get("timeout_ms"),
            enabled: row.get("enabled"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }).collect())
    }

    /// Update capability
    pub async fn update(
        &self,
        capability_id: &str,
        input: UpdateAgentCapability,
    ) -> Result<AgentCapability, AppError> {
        let now = chrono::Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE agent_capabilities
            SET capability_name = COALESCE($2, capability_name),
                description = COALESCE($3, description),
                implementation_type = COALESCE($4, implementation_type),
                implementation_ref = COALESCE($5, implementation_ref),
                success_rate = COALESCE($6, success_rate),
                avg_latency_ms = COALESCE($7, avg_latency_ms),
                times_invoked = COALESCE($8, times_invoked),
                max_tokens = COALESCE($9, max_tokens),
                timeout_ms = COALESCE($10, timeout_ms),
                enabled = COALESCE($11, enabled),
                updated_at = $12
            WHERE capability_id = $1
            RETURNING *
            "#
        )
        .bind(capability_id)
        .bind(&input.capability_name)
        .bind(&input.description)
        .bind(&input.implementation_type)
        .bind(&input.implementation_ref)
        .bind(&input.success_rate)
        .bind(&input.avg_latency_ms)
        .bind(&input.times_invoked)
        .bind(&input.max_tokens)
        .bind(&input.timeout_ms)
        .bind(&input.enabled)
        .bind(&now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

        Ok(AgentCapability {
            capability_id: result.get("capability_id"),
            agent_id: result.get("agent_id"),
            capability_name: result.get("capability_name"),
            capability_type: result.get("capability_type"),
            description: result.get("description"),
            implementation_type: result.get("implementation_type"),
            implementation_ref: result.get("implementation_ref"),
            success_rate: result.get("success_rate"),
            avg_latency_ms: result.get("avg_latency_ms"),
            times_invoked: result.get("times_invoked"),
            max_tokens: result.get("max_tokens"),
            timeout_ms: result.get("timeout_ms"),
            enabled: result.get("enabled"),
            created_at: result.get("created_at"),
            updated_at: result.get("updated_at"),
        })
    }

    /// Delete capability
    pub async fn delete(&self, capability_id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM agent_capabilities WHERE capability_id = $1")
            .bind(capability_id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        Ok(())
    }
}

// ============================================================================
// Agent Episode Repository
// ============================================================================

pub struct AgentEpisodeRepository {
    pool: PgPool,
}

impl AgentEpisodeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new episode
    pub async fn create(&self, agent_id: &str, input: CreateAgentEpisode) -> Result<AgentEpisode, AppError> {
        let episode_id = input.episode_id.unwrap_or_else(|| Ulid::new().to_string());
        let now = chrono::Utc::now().to_rfc3339();
        let start_time = input.start_time.unwrap_or_else(|| now.clone());

        let result = sqlx::query(
            r#"
            INSERT INTO agent_episodes
                (episode_id, agent_id, episode_type, start_time, end_time, situation,
                 actions_taken, outcome, outcome_score, success, what_went_well,
                 what_could_improve, lessons_learned, related_episode_ids,
                 relevant_knowledge_ids, reflection_level, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            RETURNING *
            "#
        )
        .bind(&episode_id)
        .bind(agent_id)
        .bind(&input.episode_type)
        .bind(&start_time)
        .bind(&input.end_time)
        .bind(&input.situation)
        .bind(&input.actions_taken.unwrap_or(serde_json::json!([])))
        .bind(&input.outcome)
        .bind(&input.outcome_score)
        .bind(&input.success)
        .bind(&input.what_went_well)
        .bind(&input.what_could_improve)
        .bind(&input.lessons_learned)
        .bind(&input.related_episode_ids.unwrap_or_default())
        .bind(&input.relevant_knowledge_ids.unwrap_or_default())
        .bind(&input.reflection_level.unwrap_or(0))
        .bind(&now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

        Ok(AgentEpisode {
            episode_id: result.get("episode_id"),
            agent_id: result.get("agent_id"),
            episode_type: result.get("episode_type"),
            start_time: result.get("start_time"),
            end_time: result.get("end_time"),
            situation: result.get("situation"),
            actions_taken: result.get("actions_taken"),
            outcome: result.get("outcome"),
            outcome_score: result.get("outcome_score"),
            success: result.get("success"),
            what_went_well: result.get("what_went_well"),
            what_could_improve: result.get("what_could_improve"),
            lessons_learned: result.get("lessons_learned"),
            related_episode_ids: result.get("related_episode_ids"),
            relevant_knowledge_ids: result.get("relevant_knowledge_ids"),
            reflection_level: result.get("reflection_level"),
            created_at: result.get("created_at"),
        })
    }

    /// List episodes for an agent with pagination
    pub async fn list_by_agent(
        &self,
        agent_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<EpisodeListResponse, AppError> {
        let rows = sqlx::query(
            "SELECT * FROM agent_episodes WHERE agent_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
        )
        .bind(agent_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

        let episodes: Vec<AgentEpisode> = rows.into_iter().map(|row| AgentEpisode {
            episode_id: row.get("episode_id"),
            agent_id: row.get("agent_id"),
            episode_type: row.get("episode_type"),
            start_time: row.get("start_time"),
            end_time: row.get("end_time"),
            situation: row.get("situation"),
            actions_taken: row.get("actions_taken"),
            outcome: row.get("outcome"),
            outcome_score: row.get("outcome_score"),
            success: row.get("success"),
            what_went_well: row.get("what_went_well"),
            what_could_improve: row.get("what_could_improve"),
            lessons_learned: row.get("lessons_learned"),
            related_episode_ids: row.get("related_episode_ids"),
            relevant_knowledge_ids: row.get("relevant_knowledge_ids"),
            reflection_level: row.get("reflection_level"),
            created_at: row.get("created_at"),
        }).collect();

        let total: i64 = sqlx::query("SELECT COUNT(*) FROM agent_episodes WHERE agent_id = $1")
            .bind(agent_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?
            .get("count");

        Ok(EpisodeListResponse { episodes, total })
    }

    /// Update episode
    pub async fn update(
        &self,
        episode_id: &str,
        input: UpdateAgentEpisode,
    ) -> Result<AgentEpisode, AppError> {
        let result = sqlx::query(
            r#"
            UPDATE agent_episodes
            SET end_time = COALESCE($2, end_time),
                outcome = COALESCE($3, outcome),
                outcome_score = COALESCE($4, outcome_score),
                success = COALESCE($5, success),
                what_went_well = COALESCE($6, what_went_well),
                what_could_improve = COALESCE($7, what_could_improve),
                lessons_learned = COALESCE($8, lessons_learned),
                related_episode_ids = COALESCE($9, related_episode_ids),
                relevant_knowledge_ids = COALESCE($10, relevant_knowledge_ids),
                reflection_level = COALESCE($11, reflection_level)
            WHERE episode_id = $1
            RETURNING *
            "#
        )
        .bind(episode_id)
        .bind(&input.end_time)
        .bind(&input.outcome)
        .bind(&input.outcome_score)
        .bind(&input.success)
        .bind(&input.what_went_well)
        .bind(&input.what_could_improve)
        .bind(&input.lessons_learned)
        .bind(&input.related_episode_ids)
        .bind(&input.relevant_knowledge_ids)
        .bind(&input.reflection_level)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

        Ok(AgentEpisode {
            episode_id: result.get("episode_id"),
            agent_id: result.get("agent_id"),
            episode_type: result.get("episode_type"),
            start_time: result.get("start_time"),
            end_time: result.get("end_time"),
            situation: result.get("situation"),
            actions_taken: result.get("actions_taken"),
            outcome: result.get("outcome"),
            outcome_score: result.get("outcome_score"),
            success: result.get("success"),
            what_went_well: result.get("what_went_well"),
            what_could_improve: result.get("what_could_improve"),
            lessons_learned: result.get("lessons_learned"),
            related_episode_ids: result.get("related_episode_ids"),
            relevant_knowledge_ids: result.get("relevant_knowledge_ids"),
            reflection_level: result.get("reflection_level"),
            created_at: result.get("created_at"),
        })
    }
}

// ============================================================================
// Agent Self-Model Repository
// ============================================================================

pub struct AgentSelfModelRepository {
    pool: PgPool,
}

impl AgentSelfModelRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create or update self-model
    pub async fn upsert(&self, agent_id: &str, input: CreateAgentSelfModel) -> Result<AgentSelfModel, AppError> {
        let model_id = input.model_id.unwrap_or_else(|| Ulid::new().to_string());
        let now = chrono::Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            INSERT INTO agent_self_models
                (model_id, agent_id, identity_beliefs, strengths, weaknesses, learned_skills,
                 preferences, relationships, computed_traits, confidence_score, consistency_score,
                 created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (agent_id) DO UPDATE SET
                identity_beliefs = COALESCE($3, agent_self_models.identity_beliefs),
                strengths = COALESCE($4, agent_self_models.strengths),
                weaknesses = COALESCE($5, agent_self_models.weaknesses),
                learned_skills = COALESCE($6, agent_self_models.learned_skills),
                preferences = COALESCE($7, agent_self_models.preferences),
                relationships = COALESCE($8, agent_self_models.relationships),
                confidence_score = COALESCE($10, agent_self_models.confidence_score),
                consistency_score = COALESCE($11, agent_self_models.consistency_score),
                updated_at = $13
            RETURNING *
            "#
        )
        .bind(&model_id)
        .bind(agent_id)
        .bind(&input.identity_beliefs.unwrap_or(serde_json::json!([])))
        .bind(&input.strengths.unwrap_or(serde_json::json!([])))
        .bind(&input.weaknesses.unwrap_or(serde_json::json!([])))
        .bind(&input.learned_skills.unwrap_or(serde_json::json!([])))
        .bind(&input.preferences.unwrap_or(serde_json::json!({})))
        .bind(&input.relationships.unwrap_or(serde_json::json!([])))
        .bind(serde_json::json!({}))
        .bind(0.5)
        .bind(1.0)
        .bind(&now)
        .bind(&now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

        Ok(AgentSelfModel {
            model_id: result.get("model_id"),
            agent_id: result.get("agent_id"),
            identity_beliefs: result.get("identity_beliefs"),
            strengths: result.get("strengths"),
            weaknesses: result.get("weaknesses"),
            learned_skills: result.get("learned_skills"),
            preferences: result.get("preferences"),
            relationships: result.get("relationships"),
            computed_traits: result.get("computed_traits"),
            confidence_score: result.get("confidence_score"),
            consistency_score: result.get("consistency_score"),
            created_at: result.get("created_at"),
            updated_at: result.get("updated_at"),
        })
    }

    /// Get self-model by agent ID
    pub async fn get_by_agent(&self, agent_id: &str) -> Result<Option<AgentSelfModel>, AppError> {
        let result = sqlx::query("SELECT * FROM agent_self_models WHERE agent_id = $1")
            .bind(agent_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;

        match result {
            Some(row) => Ok(Some(AgentSelfModel {
                model_id: row.get("model_id"),
                agent_id: row.get("agent_id"),
                identity_beliefs: row.get("identity_beliefs"),
                strengths: row.get("strengths"),
                weaknesses: row.get("weaknesses"),
                learned_skills: row.get("learned_skills"),
                preferences: row.get("preferences"),
                relationships: row.get("relationships"),
                computed_traits: row.get("computed_traits"),
                confidence_score: row.get("confidence_score"),
                consistency_score: row.get("consistency_score"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    /// Update self-model
    pub async fn update(
        &self,
        agent_id: &str,
        input: UpdateAgentSelfModel,
    ) -> Result<AgentSelfModel, AppError> {
        let now = chrono::Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE agent_self_models
            SET identity_beliefs = COALESCE($2, identity_beliefs),
                strengths = COALESCE($3, strengths),
                weaknesses = COALESCE($4, weaknesses),
                learned_skills = COALESCE($5, learned_skills),
                preferences = COALESCE($6, preferences),
                relationships = COALESCE($7, relationships),
                computed_traits = COALESCE($8, computed_traits),
                confidence_score = COALESCE($9, confidence_score),
                consistency_score = COALESCE($10, consistency_score),
                updated_at = $11
            WHERE agent_id = $1
            RETURNING *
            "#
        )
        .bind(agent_id)
        .bind(&input.identity_beliefs)
        .bind(&input.strengths)
        .bind(&input.weaknesses)
        .bind(&input.learned_skills)
        .bind(&input.preferences)
        .bind(&input.relationships)
        .bind(&input.computed_traits)
        .bind(&input.confidence_score)
        .bind(&input.consistency_score)
        .bind(&now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

        Ok(AgentSelfModel {
            model_id: result.get("model_id"),
            agent_id: result.get("agent_id"),
            identity_beliefs: result.get("identity_beliefs"),
            strengths: result.get("strengths"),
            weaknesses: result.get("weaknesses"),
            learned_skills: result.get("learned_skills"),
            preferences: result.get("preferences"),
            relationships: result.get("relationships"),
            computed_traits: result.get("computed_traits"),
            confidence_score: result.get("confidence_score"),
            consistency_score: result.get("consistency_score"),
            created_at: result.get("created_at"),
            updated_at: result.get("updated_at"),
        })
    }
}

// ============================================================================
// Agent Behavior Profile Repository
// ============================================================================

pub struct AgentBehaviorProfileRepository {
    pool: PgPool,
}

impl AgentBehaviorProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new behavior profile
    pub async fn create(&self, agent_id: &str, input: CreateAgentBehaviorProfile) -> Result<AgentBehaviorProfile, AppError> {
        let profile_id = input.profile_id.unwrap_or_else(|| Ulid::new().to_string());
        let now = chrono::Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            INSERT INTO agent_behavior_profiles
                (profile_id, agent_id, behavior_type, pattern_description, pattern_embedding,
                 times_applied, success_rate, avg_outcome_score, effective_contexts, confidence,
                 status, created_at, last_used_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING *
            "#
        )
        .bind(&profile_id)
        .bind(agent_id)
        .bind(&input.behavior_type)
        .bind(&input.pattern_description)
        .bind(&input.pattern_embedding.map(|v| serde_json::json!(v)))
        .bind(0)
        .bind(0.0)
        .bind(0.0)
        .bind(&input.effective_contexts.unwrap_or(serde_json::json!([])))
        .bind(0.5)
        .bind("active")
        .bind(&now)
        .bind::<Option<String>>(None)
        .bind(&now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

        Ok(AgentBehaviorProfile {
            profile_id: result.get("profile_id"),
            agent_id: result.get("agent_id"),
            behavior_type: result.get("behavior_type"),
            pattern_description: result.get("pattern_description"),
            pattern_embedding: result.get::<serde_json::Value, _>("pattern_embedding")
                .as_array()
                .map(|arr| arr.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect::<Vec<f32>>()),
            times_applied: result.get("times_applied"),
            success_rate: result.get("success_rate"),
            avg_outcome_score: result.get("avg_outcome_score"),
            effective_contexts: result.get("effective_contexts"),
            confidence: result.get("confidence"),
            status: result.get("status"),
            created_at: result.get("created_at"),
            last_used_at: result.get("last_used_at"),
            updated_at: result.get("updated_at"),
        })
    }

    /// List behavior profiles for an agent
    pub async fn list_by_agent(&self, agent_id: &str) -> Result<Vec<AgentBehaviorProfile>, AppError> {
        let rows = sqlx::query("SELECT * FROM agent_behavior_profiles WHERE agent_id = $1")
            .bind(agent_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;

        Ok(rows.into_iter().map(|row| {
            AgentBehaviorProfile {
                profile_id: row.get("profile_id"),
                agent_id: row.get("agent_id"),
                behavior_type: row.get("behavior_type"),
                pattern_description: row.get("pattern_description"),
                pattern_embedding: row.get::<serde_json::Value, _>("pattern_embedding")
                    .as_array()
                    .map(|arr| arr.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect::<Vec<f32>>()),
                times_applied: row.get("times_applied"),
                success_rate: row.get("success_rate"),
                avg_outcome_score: row.get("avg_outcome_score"),
                effective_contexts: row.get("effective_contexts"),
                confidence: row.get("confidence"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                last_used_at: row.get("last_used_at"),
                updated_at: row.get("updated_at"),
            }
        }).collect())
    }
}
