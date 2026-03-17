//! Agent Service - Business logic for agent identity and self-model management

use crate::db::agent::{
    AgentBehaviorProfileRepository, AgentCapabilityRepository, AgentEpisodeRepository,
    AgentRepository, AgentSelfModelRepository,
};
use crate::error::AppError;
use crate::models::agent::*;
use sqlx::PgPool;

pub struct AgentService {
    identity_repo: AgentRepository,
    capability_repo: AgentCapabilityRepository,
    episode_repo: AgentEpisodeRepository,
    self_model_repo: AgentSelfModelRepository,
    behavior_profile_repo: AgentBehaviorProfileRepository,
}

impl AgentService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            identity_repo: AgentRepository::new(pool.clone()),
            capability_repo: AgentCapabilityRepository::new(pool.clone()),
            episode_repo: AgentEpisodeRepository::new(pool.clone()),
            self_model_repo: AgentSelfModelRepository::new(pool.clone()),
            behavior_profile_repo: AgentBehaviorProfileRepository::new(pool),
        }
    }

    // =========================================================================
    // Identity Operations
    // =========================================================================

    /// Create a new agent with default self-model
    pub async fn create_agent(
        &self,
        input: CreateAgentIdentity,
    ) -> Result<AgentIdentity, AppError> {
        let identity = self.identity_repo.create(input).await?;

        // Create default self-model for the agent
        let _ = self
            .self_model_repo
            .upsert(&identity.agent_id, CreateAgentSelfModel::default())
            .await;

        Ok(identity)
    }

    /// Get agent by ID
    pub async fn get_agent(&self, agent_id: &str) -> Result<Option<AgentIdentity>, AppError> {
        self.identity_repo.get_by_id(agent_id).await
    }

    /// List all agents
    pub async fn list_agents(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<AgentListResponse, AppError> {
        self.identity_repo.list(limit, offset).await
    }

    /// Update agent identity
    pub async fn update_agent(
        &self,
        agent_id: &str,
        input: UpdateAgentIdentity,
    ) -> Result<AgentIdentity, AppError> {
        self.identity_repo.update(agent_id, input).await
    }

    /// Delete agent
    pub async fn delete_agent(&self, agent_id: &str) -> Result<(), AppError> {
        self.identity_repo.delete(agent_id).await
    }

    // =========================================================================
    // Capability Operations
    // =========================================================================

    /// Add capability to agent
    pub async fn add_capability(
        &self,
        agent_id: &str,
        input: CreateAgentCapability,
    ) -> Result<AgentCapability, AppError> {
        self.capability_repo.create(agent_id, input).await
    }

    /// List agent capabilities
    pub async fn list_capabilities(
        &self,
        agent_id: &str,
    ) -> Result<Vec<AgentCapability>, AppError> {
        self.capability_repo.list_by_agent(agent_id).await
    }

    /// Update capability
    pub async fn update_capability(
        &self,
        capability_id: &str,
        input: UpdateAgentCapability,
    ) -> Result<AgentCapability, AppError> {
        self.capability_repo.update(capability_id, input).await
    }

    /// Delete capability
    pub async fn delete_capability(&self, capability_id: &str) -> Result<(), AppError> {
        self.capability_repo.delete(capability_id).await
    }

    // =========================================================================
    // Episode Operations
    // =========================================================================

    /// Record an episode (experience)
    pub async fn record_episode(
        &self,
        agent_id: &str,
        input: CreateAgentEpisode,
    ) -> Result<AgentEpisode, AppError> {
        self.episode_repo.create(agent_id, input).await
    }

    /// List agent episodes
    pub async fn list_episodes(
        &self,
        agent_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<EpisodeListResponse, AppError> {
        self.episode_repo
            .list_by_agent(agent_id, limit, offset)
            .await
    }

    /// Update episode (e.g., add reflection)
    pub async fn update_episode(
        &self,
        episode_id: &str,
        input: UpdateAgentEpisode,
    ) -> Result<AgentEpisode, AppError> {
        self.episode_repo.update(episode_id, input).await
    }

    // =========================================================================
    // Self-Model Operations
    // =========================================================================

    /// Get agent self-model
    pub async fn get_self_model(&self, agent_id: &str) -> Result<Option<AgentSelfModel>, AppError> {
        self.self_model_repo.get_by_agent(agent_id).await
    }

    /// Update agent self-model
    pub async fn update_self_model(
        &self,
        agent_id: &str,
        input: UpdateAgentSelfModel,
    ) -> Result<AgentSelfModel, AppError> {
        self.self_model_repo.update(agent_id, input).await
    }

    // =========================================================================
    // Behavior Profile Operations
    // =========================================================================

    /// Record a behavior pattern
    pub async fn record_behavior(
        &self,
        agent_id: &str,
        input: CreateAgentBehaviorProfile,
    ) -> Result<AgentBehaviorProfile, AppError> {
        self.behavior_profile_repo.create(agent_id, input).await
    }

    /// List behavior profiles
    pub async fn list_behaviors(
        &self,
        agent_id: &str,
    ) -> Result<Vec<AgentBehaviorProfile>, AppError> {
        self.behavior_profile_repo.list_by_agent(agent_id).await
    }

    // =========================================================================
    // Composite Operations
    // =========================================================================

    /// Get complete agent info with self-model
    pub async fn get_agent_with_self_model(
        &self,
        agent_id: &str,
    ) -> Result<Option<AgentWithSelfModel>, AppError> {
        let identity = match self.identity_repo.get_by_id(agent_id).await? {
            Some(id) => id,
            None => return Ok(None),
        };

        let self_model = self.self_model_repo.get_by_agent(agent_id).await?;
        let capabilities = self.capability_repo.list_by_agent(agent_id).await?;
        let behavior_profiles = self.behavior_profile_repo.list_by_agent(agent_id).await?;
        let recent_episodes = self.episode_repo.list_by_agent(agent_id, 10, 0).await?;

        Ok(Some(AgentWithSelfModel {
            identity,
            self_model,
            capabilities,
            behavior_profiles,
            recent_episodes: recent_episodes.episodes,
        }))
    }

    /// Trigger self-reflection (analyze recent episodes and update self-model)
    pub async fn trigger_reflection(&self, agent_id: &str) -> Result<AgentSelfModel, AppError> {
        // Get recent episodes for reflection
        let episodes = self.episode_repo.list_by_agent(agent_id, 50, 0).await?;

        // Simple reflection: analyze success rate and update self-model
        let total_episodes = episodes.episodes.len() as f64;
        if total_episodes == 0.0 {
            return self
                .self_model_repo
                .get_by_agent(agent_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Self-model not found".to_string()));
        }

        let successful_episodes = episodes
            .episodes
            .iter()
            .filter(|e| e.success.unwrap_or(false))
            .count() as f64;
        let success_rate = successful_episodes / total_episodes;

        // Get current self-model
        let current = self.self_model_repo.get_by_agent(agent_id).await?;

        // Update confidence score based on success rate
        let confidence_score = match current {
            Some(model) => (model.confidence_score + success_rate) / 2.0,
            None => success_rate,
        };

        // Update self-model
        self.self_model_repo
            .update(
                agent_id,
                UpdateAgentSelfModel {
                    identity_beliefs: None,
                    strengths: None,
                    weaknesses: None,
                    learned_skills: None,
                    preferences: None,
                    relationships: None,
                    computed_traits: None,
                    confidence_score: Some(confidence_score),
                    consistency_score: Some(1.0), // Could calculate based on episode consistency
                },
            )
            .await
    }
}

// Default implementations
impl Default for CreateAgentSelfModel {
    fn default() -> Self {
        Self {
            model_id: None,
            identity_beliefs: Some(serde_json::json!([])),
            strengths: Some(serde_json::json!([])),
            weaknesses: Some(serde_json::json!([])),
            learned_skills: Some(serde_json::json!([])),
            preferences: Some(serde_json::json!({})),
            relationships: Some(serde_json::json!([])),
        }
    }
}
