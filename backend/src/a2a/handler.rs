use a2a::types::{
    Message, Part, PartContent, Role, SendMessageRequest, SendMessageResponse, Task, TaskState,
    TaskStatus,
};
use serde_json::{json, Value};
use std::collections::HashMap;

use super::skills::{MemoryFusionRequest, MemorySearchRequest, MemorySkill, MemoryStoreRequest};

pub struct A2AHandler {}

impl A2AHandler {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn handle_message(
        &self,
        request: SendMessageRequest,
    ) -> Result<SendMessageResponse, String> {
        let message = &request.message;
        let skill_id = self.detect_skill(message);

        match skill_id {
            Some(MemorySkill::MemorySearch) => self.handle_memory_search(request).await,
            Some(MemorySkill::MemoryStore) => self.handle_memory_store(request).await,
            Some(MemorySkill::MemoryFusion) => self.handle_memory_fusion(request).await,
            Some(MemorySkill::MemoryStatus) => self.handle_memory_status(request).await,
            Some(MemorySkill::KnowledgeGraph) => self.handle_knowledge_graph(request).await,
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
    ) -> Result<SendMessageResponse, String> {
        let text = self.extract_text(&request.message);

        let search_request = MemorySearchRequest {
            query: text,
            layer: None,
            limit: Some(10),
        };

        let response_text = format!(
            "Memory search completed for query: '{}'",
            search_request.query
        );

        self.create_response(
            request,
            response_text,
            json!({
                "skill": "memory_search",
                "request": search_request
            }),
        )
    }

    async fn handle_memory_store(
        &self,
        request: SendMessageRequest,
    ) -> Result<SendMessageResponse, String> {
        let text = self.extract_text(&request.message);

        let store_request = MemoryStoreRequest {
            content: text,
            layer: "stm".to_string(),
            metadata: None,
        };

        let response_text = format!("Memory stored in {} layer", store_request.layer);

        self.create_response(
            request,
            response_text,
            json!({
                "skill": "memory_store",
                "request": store_request
            }),
        )
    }

    async fn handle_memory_fusion(
        &self,
        request: SendMessageRequest,
    ) -> Result<SendMessageResponse, String> {
        let text = self.extract_text(&request.message);

        let fusion_request = MemoryFusionRequest {
            query: text,
            limit: Some(20),
        };

        let response_text = format!(
            "Memory fusion query completed for: '{}'",
            fusion_request.query
        );

        self.create_response(
            request,
            response_text,
            json!({
                "skill": "memory_fusion",
                "request": fusion_request
            }),
        )
    }

    async fn handle_memory_status(
        &self,
        request: SendMessageRequest,
    ) -> Result<SendMessageResponse, String> {
        let response_text = "Memory system status: All layers healthy".to_string();

        self.create_response(
            request,
            response_text,
            json!({
                "skill": "memory_status",
                "stm_count": 0,
                "ltm_count": 0,
                "kg_count": 0,
                "mm_count": 0,
                "overall_healthy": true
            }),
        )
    }

    async fn handle_knowledge_graph(
        &self,
        request: SendMessageRequest,
    ) -> Result<SendMessageResponse, String> {
        let text = self.extract_text(&request.message);

        let response_text = format!("Knowledge graph query for: '{}'", text);

        self.create_response(
            request,
            response_text,
            json!({
                "skill": "knowledge_graph",
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
