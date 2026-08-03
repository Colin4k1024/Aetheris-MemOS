use a2a::types::{
    Message, Part, PartContent, Role, SendMessageRequest, SendMessageResponse, Task, TaskState,
    TaskStatus,
};
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::db::{self, kg::KGRepository, pool};
use crate::services::memory_fusion::MemoryFusionService;
use crate::services::memory_search::MemorySearchService;
use crate::services::memory_storage::MemoryStorageService;
use crate::tenant::RequestTenantContext;

use super::skills::MemorySkill;

/// A2A handler that delegates to real memory services.
///
/// Each handler method receives a `tenant_ctx` (from the A2A auth middleware)
/// and calls the actual memory service layer — no more format! placeholders.
pub struct A2AHandler {}

impl A2AHandler {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn handle_message(
        &self,
        request: SendMessageRequest,
        tenant_ctx: &RequestTenantContext,
    ) -> Result<SendMessageResponse, String> {
        let message = &request.message;
        let skill_id = self.detect_skill(message);

        info!(
            tenant = %tenant_ctx.tenant_id,
            skill = ?skill_id,
            "A2A message received"
        );

        match skill_id {
            Some(MemorySkill::MemorySearch) => self.handle_memory_search(request, tenant_ctx).await,
            Some(MemorySkill::MemoryStore) => self.handle_memory_store(request, tenant_ctx).await,
            Some(MemorySkill::MemoryFusion) => self.handle_memory_fusion(request, tenant_ctx).await,
            Some(MemorySkill::MemoryStatus) => self.handle_memory_status(request, tenant_ctx).await,
            Some(MemorySkill::KnowledgeGraph) => {
                self.handle_knowledge_graph(request, tenant_ctx).await
            }
            None => self.handle_general_query(request).await,
        }
    }

    fn detect_skill(&self, message: &Message) -> Option<MemorySkill> {
        let text = self.extract_text(message).to_lowercase();

        if text.contains("search") || text.contains("find") || text.contains("query") {
            Some(MemorySkill::MemorySearch)
        } else if text.contains("store") || text.contains("remember") || text.contains("save") {
            Some(MemorySkill::MemoryStore)
        } else if text.contains("fusion")
            || text.contains("all layers")
            || text.contains("comprehensive")
        {
            Some(MemorySkill::MemoryFusion)
        } else if text.contains("status") || text.contains("health") || text.contains("stats") {
            Some(MemorySkill::MemoryStatus)
        } else if text.contains("knowledge") || text.contains("entity") || text.contains("graph") {
            Some(MemorySkill::KnowledgeGraph)
        } else {
            Some(MemorySkill::MemorySearch)
        }
    }

    fn extract_text(&self, message: &Message) -> String {
        message
            .parts
            .iter()
            .filter_map(|part| {
                if let PartContent::Text(text) = &part.content {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    async fn handle_memory_search(
        &self,
        request: SendMessageRequest,
        tenant_ctx: &RequestTenantContext,
    ) -> Result<SendMessageResponse, String> {
        let text = self.extract_text(&request.message);

        // MemorySearchService is a unit struct with static methods.
        // search_ltm_for_tenant: (tenant_id, query, top_k, enable_rerank, min_score)
        let results = MemorySearchService::search_ltm_for_tenant(
            &tenant_ctx.tenant_id,
            &text,
            10,
            None,
            None,
        )
        .await
        .map_err(|e| format!("Memory search failed: {e}"))?;

        let result_count = results.len();
        let response_text = if results.is_empty() {
            format!("No memories found for query: '{}'", text)
        } else {
            let summaries: Vec<String> = results
                .iter()
                .take(5)
                .map(|r| {
                    let content = r.content.chars().take(100).collect::<String>();
                    format!("- {} (score: {:.2})", content, r.score)
                })
                .collect();
            format!(
                "Found {} memories for '{}':\n{}",
                result_count,
                text,
                summaries.join("\n")
            )
        };

        self.create_response(
            request,
            response_text,
            json!({
                "skill": "memory_search",
                "result_count": result_count,
                "query": text
            }),
        )
    }

    async fn handle_memory_store(
        &self,
        request: SendMessageRequest,
        tenant_ctx: &RequestTenantContext,
    ) -> Result<SendMessageResponse, String> {
        let text = self.extract_text(&request.message);

        // MemoryStorageService::store_ltm_for_tenant: (tenant_id, source_id, source_type, content, title)
        let result = MemoryStorageService::store_ltm_for_tenant(
            &tenant_ctx.tenant_id,
            &format!("a2a:{}", uuid::Uuid::new_v4()),
            "a2a_message",
            &text,
            None,
        )
        .await
        .map_err(|e| format!("Memory store failed: {e}"))?;

        let response_text = format!(
            "Memory stored successfully. Entry ID: {}, Status: {}",
            result.entry_id, result.index_status
        );

        self.create_response(
            request,
            response_text,
            json!({
                "skill": "memory_store",
                "entry_id": result.entry_id,
                "index_status": result.index_status
            }),
        )
    }

    async fn handle_memory_fusion(
        &self,
        request: SendMessageRequest,
        tenant_ctx: &RequestTenantContext,
    ) -> Result<SendMessageResponse, String> {
        let text = self.extract_text(&request.message);

        // MemoryFusionService::query: (query, tenant_id, limit)
        let fusion_result = MemoryFusionService::query(&text, &tenant_ctx.tenant_id, Some(20))
            .await
            .map_err(|e| format!("Memory fusion failed: {e}"))?;

        let response_text = format!(
            "Memory fusion completed for '{}': STM={}, LTM={}, KG={}, MM={}, Merged={}",
            text,
            fusion_result.layer_results.stm.len(),
            fusion_result.layer_results.ltm.len(),
            fusion_result.layer_results.kg.len(),
            fusion_result.layer_results.mm.len(),
            fusion_result.merged_results.len()
        );

        self.create_response(
            request,
            response_text,
            json!({
                "skill": "memory_fusion",
                "stm_count": fusion_result.layer_results.stm.len(),
                "ltm_count": fusion_result.layer_results.ltm.len(),
                "kg_count": fusion_result.layer_results.kg.len(),
                "mm_count": fusion_result.layer_results.mm.len(),
                "merged_count": fusion_result.merged_results.len()
            }),
        )
    }

    async fn handle_memory_status(
        &self,
        request: SendMessageRequest,
        tenant_ctx: &RequestTenantContext,
    ) -> Result<SendMessageResponse, String> {
        let pg_pool = pool();

        // Get real counts from each memory layer
        let stm_count = db::stm::STMRepository::get_active_user_ids(pg_pool, &tenant_ctx.tenant_id)
            .await
            .map(|ids| ids.len())
            .unwrap_or(0);

        // LTM: use list_entities equivalent via search with empty query or direct count
        // Since LTMRepository has no count_by_tenant, use a search with limit 1 to verify connectivity
        let ltm_accessible =
            MemorySearchService::search_ltm_for_tenant(&tenant_ctx.tenant_id, "", 1, None, None)
                .await
                .map(|r| !r.is_empty() || true) // connectivity check
                .unwrap_or(false);

        // KG: list_entities takes (pool, tenant_id, entity_type, limit, offset)
        let kg_result = KGRepository::list_entities(
            pg_pool,
            &tenant_ctx.tenant_id,
            None, // all entity types
            Some(1),
            Some(0),
        )
        .await;
        let kg_entities = kg_result.map(|r| r.total).unwrap_or(0);

        let overall_healthy = ltm_accessible;

        let response_text = format!(
            "Memory system status: STM active users={}, KG entities={}, Overall={}",
            stm_count,
            kg_entities,
            if overall_healthy {
                "healthy"
            } else {
                "degraded"
            }
        );

        self.create_response(
            request,
            response_text,
            json!({
                "skill": "memory_status",
                "stm_active_users": stm_count,
                "kg_entities": kg_entities,
                "overall_healthy": overall_healthy
            }),
        )
    }

    async fn handle_knowledge_graph(
        &self,
        request: SendMessageRequest,
        tenant_ctx: &RequestTenantContext,
    ) -> Result<SendMessageResponse, String> {
        let text = self.extract_text(&request.message);

        // Use search_entries_by_entity_for_tenant (actual method name)
        let entities = KGRepository::search_entries_by_entity_for_tenant(
            pool(),
            &tenant_ctx.tenant_id,
            &text,
            10,
        )
        .await
        .map_err(|e| format!("Knowledge graph query failed: {e}"))?;

        let entity_count = entities.len();
        let response_text = if entities.is_empty() {
            format!("No entities found for query: '{}'", text)
        } else {
            let summaries: Vec<String> = entities
                .iter()
                .take(5)
                .map(|e| format!("- {} ({})", e.entity_name, e.entity_type))
                .collect();
            format!(
                "Found {} entities for '{}':\n{}",
                entity_count,
                text,
                summaries.join("\n")
            )
        };

        self.create_response(
            request,
            response_text,
            json!({
                "skill": "knowledge_graph",
                "entity_count": entity_count,
                "query": text
            }),
        )
    }

    async fn handle_general_query(
        &self,
        request: SendMessageRequest,
    ) -> Result<SendMessageResponse, String> {
        let text = self.extract_text(&request.message);
        let response_text = format!(
            "Received your message: '{}'. How can I help you with memory operations?",
            text
        );

        self.create_response(
            request,
            response_text,
            json!({
                "skill": "general"
            }),
        )
    }

    fn create_response(
        &self,
        request: SendMessageRequest,
        text: String,
        metadata: Value,
    ) -> Result<SendMessageResponse, String> {
        let message_id = uuid::Uuid::new_v4().to_string();
        let context_id = request
            .message
            .context_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let response_message = Message {
            message_id,
            context_id: Some(context_id.clone()),
            task_id: None,
            role: Role::Agent,
            parts: vec![Part::text(text)],
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        };

        let task_id = uuid::Uuid::new_v4().to_string();
        let task = Task {
            id: task_id,
            context_id,
            status: TaskStatus {
                state: TaskState::Completed,
                message: Some(response_message.clone()),
                timestamp: Some(chrono::Utc::now()),
            },
            artifacts: Some(vec![]),
            history: Some(vec![request.message, response_message]),
            metadata: None,
        };

        Ok(SendMessageResponse::Task(task))
    }
}
