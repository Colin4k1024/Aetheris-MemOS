use a2a::types::{
    Message, Part, PartContent, Role, SendMessageRequest, SendMessageResponse, Task, TaskState,
    TaskStatus,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::{
    services::{
        memory_fusion::MemoryFusionService, memory_search::MemorySearchService,
        memory_storage::MemoryStorageService,
    },
    tenant::TenantId,
};

use super::skills::MemorySkill;

#[derive(Clone)]
struct StoredTask {
    task: Task,
    tenant_id: TenantId,
}

pub struct A2AHandler {
    tasks: RwLock<HashMap<String, StoredTask>>,
}

impl A2AHandler {
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_task(&self, tenant_id: &TenantId, task_id: &str) -> Option<Task> {
        self.tasks
            .read()
            .await
            .get(task_id)
            .filter(|stored| stored.tenant_id == *tenant_id)
            .map(|stored| stored.task.clone())
    }

    pub async fn list_tasks(&self, tenant_id: &TenantId) -> Vec<Task> {
        self.tasks
            .read()
            .await
            .values()
            .filter(|stored| stored.tenant_id == *tenant_id)
            .map(|stored| stored.task.clone())
            .collect()
    }

    pub async fn handle_message(
        &self,
        request: SendMessageRequest,
        tenant_id: TenantId,
    ) -> Result<SendMessageResponse, String> {
        self.handle_message_for_task(request, tenant_id, uuid::Uuid::new_v4().to_string())
            .await
    }

    pub async fn handle_message_for_task(
        &self,
        request: SendMessageRequest,
        tenant_id: TenantId,
        task_id: String,
    ) -> Result<SendMessageResponse, String> {
        let failed_request = request.clone();
        let message = &request.message;
        let skill_id = self.detect_skill(message);

        let response = match skill_id {
            Some(MemorySkill::MemorySearch) => {
                self.handle_memory_search(request, &tenant_id, task_id.clone())
                    .await
            }
            Some(MemorySkill::MemoryStore) => {
                self.handle_memory_store(request, &tenant_id, task_id.clone())
                    .await
            }
            Some(MemorySkill::MemoryFusion) => {
                self.handle_memory_fusion(request, &tenant_id, task_id.clone())
                    .await
            }
            Some(MemorySkill::KnowledgeGraph) => {
                self.handle_knowledge_graph(request, &tenant_id, task_id.clone())
                    .await
            }
            None => self.handle_general_query(request).await,
        };

        if let Err(error) = &response {
            self.store_failed_task(failed_request, &tenant_id, error, task_id)
                .await;
        }

        response
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
            None
        } else if text.contains("knowledge") || text.contains("entity") || text.contains("graph") {
            Some(MemorySkill::KnowledgeGraph)
        } else {
            None
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
        tenant_id: &TenantId,
        task_id: String,
    ) -> Result<SendMessageResponse, String> {
        self.ensure_memory_services_ready()?;
        let text = self.extract_text(&request.message);
        let result = MemoryFusionService::query(&text, tenant_id, Some(10))
            .await
            .map_err(|error| format!("Memory search failed: {error}"))?;

        // #128: same recall core as every other transport. The A2A caller may
        // pass the authenticated subject via request metadata `user_id`.
        let mut payload = json!({
            "skill": "memory_search",
            "result": result
        });
        let a2a_user = request
            .metadata
            .as_ref()
            .and_then(|m| m.get("user_id"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        if let Some(user) = a2a_user {
            if let Ok(Some(wm)) = crate::services::recall::core::belief_working_memory(
                tenant_id,
                Some(&user),
                &text,
                None,
                None,
                None,
                &[],
            )
            .await
            {
                payload["beliefs"] = serde_json::to_value(&wm.items).unwrap_or_default();
                payload["workingMemory"] = serde_json::Value::String(wm.text.clone());
            }
        }
        self.create_response(
            task_id,
            request,
            tenant_id,
            "Memory search completed".to_string(),
            payload,
        )
        .await
    }

    async fn handle_memory_store(
        &self,
        request: SendMessageRequest,
        tenant_id: &TenantId,
        task_id: String,
    ) -> Result<SendMessageResponse, String> {
        self.ensure_memory_services_ready()?;
        let text = self.extract_text(&request.message);
        let (session_id, message_id) = MemoryStorageService::store_stm_for_tenant(
            tenant_id,
            "a2a",
            "a2a",
            "conversation",
            "user",
            &text,
            4_000,
            24,
            request.message.context_id.as_deref(),
        )
        .await
        .map_err(|error| format!("Memory store failed: {error}"))?;

        self.create_response(
            task_id,
            request,
            tenant_id,
            "Memory stored in stm layer".to_string(),
            json!({
                "skill": "memory_store",
                "layer": "stm",
                "sessionId": session_id,
                "messageId": message_id,
            }),
        )
        .await
    }

    async fn handle_memory_fusion(
        &self,
        request: SendMessageRequest,
        tenant_id: &TenantId,
        task_id: String,
    ) -> Result<SendMessageResponse, String> {
        self.ensure_memory_services_ready()?;
        let text = self.extract_text(&request.message);
        let result = MemoryFusionService::query(&text, tenant_id, Some(20))
            .await
            .map_err(|error| format!("Memory fusion failed: {error}"))?;
        self.create_response(
            task_id,
            request,
            tenant_id,
            "Memory fusion query completed".to_string(),
            json!({
                "skill": "memory_fusion",
                "result": result
            }),
        )
        .await
    }

    async fn handle_knowledge_graph(
        &self,
        request: SendMessageRequest,
        tenant_id: &TenantId,
        task_id: String,
    ) -> Result<SendMessageResponse, String> {
        self.ensure_memory_services_ready()?;
        let text = self.extract_text(&request.message);
        let results = MemorySearchService::search_by_entity_for_tenant(tenant_id, &text, Some(10))
            .await
            .map_err(|error| format!("Knowledge graph search failed: {error}"))?;
        self.create_response(
            task_id,
            request,
            tenant_id,
            "Knowledge graph query completed".to_string(),
            json!({
                "skill": "knowledge_graph",
                "results": results
            }),
        )
        .await
    }

    async fn handle_general_query(
        &self,
        _request: SendMessageRequest,
    ) -> Result<SendMessageResponse, String> {
        Err("Unsupported A2A operation. Use one of the advertised memory skills.".to_string())
    }

    async fn create_response(
        &self,
        task_id: String,
        request: SendMessageRequest,
        tenant_id: &TenantId,
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
            task_id: Some(task_id.clone()),
            role: Role::Agent,
            parts: vec![Part::text(text)],
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        };

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
            metadata: Some(HashMap::from([("result".to_string(), metadata)])),
        };

        self.tasks.write().await.insert(
            task.id.clone(),
            StoredTask {
                task: task.clone(),
                tenant_id: tenant_id.clone(),
            },
        );
        Ok(SendMessageResponse::Task(task))
    }

    async fn store_failed_task(
        &self,
        request: SendMessageRequest,
        tenant_id: &TenantId,
        error: &str,
        task_id: String,
    ) {
        let context_id = request
            .message
            .context_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let response_message = Message {
            message_id: uuid::Uuid::new_v4().to_string(),
            context_id: Some(context_id.clone()),
            task_id: Some(task_id.clone()),
            role: Role::Agent,
            parts: vec![Part::text(error.to_string())],
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        };
        let task = Task {
            id: task_id.clone(),
            context_id,
            status: TaskStatus {
                state: TaskState::Failed,
                message: Some(response_message.clone()),
                timestamp: Some(chrono::Utc::now()),
            },
            artifacts: Some(vec![]),
            history: Some(vec![request.message, response_message]),
            metadata: None,
        };

        self.tasks.write().await.insert(
            task_id,
            StoredTask {
                task,
                tenant_id: tenant_id.clone(),
            },
        );
    }

    fn ensure_memory_services_ready(&self) -> Result<(), String> {
        if crate::db::DATABASE_POOL.get().is_none() {
            return Err("Memory services are not initialized".to_string());
        }
        Ok(())
    }
}
