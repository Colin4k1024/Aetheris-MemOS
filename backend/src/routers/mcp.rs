//! MCP (Model Context Protocol) Router
//!
//! This module provides HTTP endpoints for MCP protocol communication.

use axum::{
    extract::{Extension, State},
    middleware,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::hoops::enterprise::{try_enterprise_hooks, HookContext, HookDecision, HookResult, Operation};
use crate::hoops::governance::governance_middleware;
use crate::hoops::jwt::auth_middleware;
use crate::mcp::capability;
use crate::mcp::signing::{
    verify_component, verify_unsigned, ComponentSignature, SigningError, TrustedKeyBundle,
};

use crate::db::audit::AuditEvent;
use crate::db::kg::KGRepository;
use crate::db::ltm::LTMRepository;
use crate::db::mm::MMRepository;
use crate::db::pool;
use crate::db::stm::STMRepository;
use crate::protocol::mcp::{
    get_memory_resources, get_memory_tools, Resource as McpResource, ResourceContent,
    ResourceContentResponse, ServerCapabilities, Tool as McpTool, ToolCallResponse,
    ToolsListResponse, TOOL_MEMORY_FORGET, TOOL_MEMORY_LIST, TOOL_MEMORY_RECALL,
    TOOL_MEMORY_SEARCH, TOOL_MEMORY_WRITE,
};
use crate::services::audit_writer;
use crate::services::{memory_search::MemorySearchService, memory_storage::MemoryStorageService};
use crate::tenant::{RequestTenantContext, TenantId};
use crate::{json_ok, AppError, JsonResult};

/// MCP Router State
#[derive(Clone)]
pub struct McpState {
    pub server_name: String,
    pub server_version: String,
    pub component_registry: Arc<McpComponentRegistry>,
}

impl Default for McpState {
    fn default() -> Self {
        Self {
            server_name: "adaptive-memory-system".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            component_registry: Arc::new(McpComponentRegistry::new()),
        }
    }
}

/// MCP Component Registry - holds verified tools and resources
#[derive(Debug)]
pub struct McpComponentRegistry {
    key_bundle: TrustedKeyBundle,
    verified_tools: RwLock<Vec<(McpTool, ComponentSignature)>>,
    verified_resources: RwLock<Vec<(McpResource, ComponentSignature)>>,
}

impl McpComponentRegistry {
    /// Create new registry with trusted key bundle from environment
    pub fn new() -> Self {
        Self {
            key_bundle: TrustedKeyBundle::load_from_env(),
            verified_tools: RwLock::new(Vec::new()),
            verified_resources: RwLock::new(Vec::new()),
        }
    }

    /// Register a tool with its signature - returns Ok if verified
    pub async fn register_tool(
        &self,
        tool: McpTool,
        signature: Option<ComponentSignature>,
    ) -> Result<(), String> {
        match signature {
            Some(sig) => {
                let artifact = serde_json::to_vec(&tool).map_err(|e| e.to_string())?;
                verify_component(&tool.name, &artifact, &sig, &self.key_bundle)
                    .map_err(|e| e.to_string())?;
                self.verified_tools.write().await.push((tool, sig));
                Ok(())
            }
            None => {
                // D-03: Unsigned components are rejected
                Err(verify_unsigned(&tool.name).unwrap_err().to_string())
            }
        }
    }

    /// Register a resource with its signature - returns Ok if verified
    pub async fn register_resource(
        &self,
        resource: McpResource,
        signature: Option<ComponentSignature>,
    ) -> Result<(), String> {
        match signature {
            Some(sig) => {
                let artifact = serde_json::to_vec(&resource).map_err(|e| e.to_string())?;
                verify_component(&resource.uri, &artifact, &sig, &self.key_bundle)
                    .map_err(|e| e.to_string())?;
                self.verified_resources.write().await.push((resource, sig));
                Ok(())
            }
            None => {
                // D-03: Unsigned components are rejected
                Err(verify_unsigned(&resource.uri).unwrap_err().to_string())
            }
        }
    }

    /// Get all verified tools
    pub async fn get_verified_tools(&self) -> Vec<McpTool> {
        self.verified_tools
            .read()
            .await
            .iter()
            .map(|(t, _)| t.clone())
            .collect()
    }

    /// Get all verified resources
    pub async fn get_verified_resources(&self) -> Vec<McpResource> {
        self.verified_resources
            .read()
            .await
            .iter()
            .map(|(r, _)| r.clone())
            .collect()
    }
}

/// Build MCP router with authentication
pub fn router() -> Router {
    let state = McpState::default();

    // Protected routes (require authentication)
    // Layer order: auth (outer, last) → governance (inner, first) → handler.
    // Request flow: auth authenticates first, then governance runs pre-hooks.
    let protected_router = Router::new()
        .route("/tools", get(list_tools))
        .route("/tools/call", post(call_tool))
        .route("/resources", get(list_resources))
        .route("/resources/read", post(read_resource))
        .route_layer(middleware::from_fn(governance_middleware))
        .layer(middleware::from_fn(auth_middleware));

    Router::new()
        .route("/initialize", post(initialize))
        .nest("/mcp", protected_router)
        .with_state(state)
}

/// Initialize MCP server
async fn initialize(State(state): State<McpState>) -> JsonResult<serde_json::Value> {
    json_ok(serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": ServerCapabilities::default(),
        "serverInfo": {
            "name": state.server_name,
            "version": state.server_version
        }
    }))
}

/// List available MCP tools - only returns tools with valid signatures
async fn list_tools(State(state): State<McpState>) -> JsonResult<ToolsListResponse> {
    let registry = &state.component_registry;
    let key_bundle = &registry.key_bundle;

    let all_tools = get_memory_tools();
    let mut verified_tools = Vec::new();

    // Try to load signatures from environment
    let signatures_json = std::env::var("MCP_TOOL_SIGNATURES").unwrap_or_default();
    let signatures: Vec<ComponentSignature> =
        serde_json::from_str(&signatures_json).unwrap_or_default();

    for tool in all_tools {
        // Find signature for this tool
        if let Some(sig) = signatures.iter().find(|s| s.component_id == tool.name) {
            let artifact = match serde_json::to_vec(&tool) {
                Ok(a) => a,
                Err(e) => {
                    warn!("MCP tool {} failed to serialize: {}", tool.name, e);
                    continue;
                }
            };

            // D-01: Verify signature on load
            if let Err(e) = verify_component(&tool.name, &artifact, sig, key_bundle) {
                // D-03: Reject unsigned/invalid components with structured log
                warn!(
                    component_id = %tool.name,
                    issuer = %sig.issuer,
                    error = %e,
                    "MCP tool rejected: signing verification failed"
                );
                continue;
            }

            verified_tools.push(tool);
        } else {
            // No signature found - reject unsigned component (D-03)
            warn!(
                component_id = %tool.name,
                "MCP tool rejected: no signature found"
            );
        }
    }

    json_ok(ToolsListResponse {
        tools: verified_tools,
    })
}

/// MCP tool call request
#[derive(Debug, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
}

/// Map each MCP tool to the governance [`Operation`] it truly performs.
///
/// This mapping lives here — next to the dispatch `match` — so it cannot drift
/// without being noticed. **A new tool added to the dispatch MUST also be added here.**
///
/// | Tool constant          | Operation | Reason                         |
/// |------------------------|-----------|--------------------------------|
/// | `TOOL_MEMORY_WRITE`    | Store     | Creates/updates memory entries |
/// | `TOOL_MEMORY_FORGET`   | Delete    | Deletes memory entries         |
/// | `TOOL_MEMORY_SEARCH`   | Search    | Read-only query                |
/// | `TOOL_MEMORY_RECALL`   | Search    | Read-only session recall       |
/// | `TOOL_MEMORY_LIST`     | Search    | Read-only listing              |
fn tool_to_operation(tool_name: &str) -> Option<Operation> {
    match tool_name {
        TOOL_MEMORY_WRITE => Some(Operation::Store),
        TOOL_MEMORY_FORGET => Some(Operation::Delete),
        TOOL_MEMORY_SEARCH | TOOL_MEMORY_RECALL | TOOL_MEMORY_LIST => Some(Operation::Search),
        _ => None,
    }
}

/// Call MCP tool
async fn call_tool(
    State(state): State<McpState>,
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(params): Json<ToolCallParams>,
) -> JsonResult<ToolCallResponse> {
    info!("MCP tool call: {}", params.name);

    // --- Step a: Verify tool call signature (D-01, D-03) ---
    // Re-verify the signature at invocation time using MCP_TOOL_SIGNATURES (same
    // source as list_tools). Unsigned tools are rejected via verify_unsigned.
    let signatures_json = std::env::var("MCP_TOOL_SIGNATURES").unwrap_or_default();
    let signatures: Vec<ComponentSignature> =
        serde_json::from_str(&signatures_json).unwrap_or_default();
    let key_bundle = &state.component_registry.key_bundle;

    let signing_result = match signatures.iter().find(|s| s.component_id == params.name) {
        Some(sig) => {
            let tool = get_memory_tools()
                .into_iter()
                .find(|t| t.name == params.name);
            match tool {
                Some(tool) => match serde_json::to_vec(&tool) {
                    Ok(artifact) => verify_component(&params.name, &artifact, sig, key_bundle),
                    Err(e) => Err(SigningError::VerificationFailed(
                        params.name.clone(),
                        e.to_string(),
                    )),
                },
                None => verify_unsigned(&params.name),
            }
        }
        None => verify_unsigned(&params.name),
    };

    if let Err(signing_err) = signing_result {
        warn!(
            component_id = %params.name,
            error = %signing_err,
            "MCP tool call rejected: signing verification failed"
        );
        audit_writer::record_audit(
            AuditEvent::new("mcp.call_tool", "mcp_tool")
                .tenant(tenant_ctx.tenant_id.as_str())
                .with_metadata(&serde_json::json!({
                    "tool": params.name,
                    "outcome": "denied",
                    "reason": "signing_verification_failed",
                })),
        );
        return Err(AppError::Forbidden(
            "Tool not authorized: signing verification failed".to_string(),
        ));
    }

    // --- Step b: Capability authorization (deny-by-default) ---
    // Plane A trusted first-party path: grant [Read, Write, Delete].
    // authorize() rejects unknown tools regardless of granted capabilities.
    let granted = [
        capability::MemoryCapability::Read,
        capability::MemoryCapability::Write,
        capability::MemoryCapability::Delete,
    ];
    if let Err(authz_err) = capability::authorize(&granted, &params.name) {
        warn!(
            component_id = %params.name,
            error = %authz_err,
            "MCP tool call rejected: capability authorization failed"
        );
        audit_writer::record_audit(
            AuditEvent::new("mcp.call_tool", "mcp_tool")
                .tenant(tenant_ctx.tenant_id.as_str())
                .with_metadata(&serde_json::json!({
                    "tool": params.name,
                    "outcome": "denied",
                    "reason": "capability_unauthorized",
                    "error": authz_err.to_string(),
                })),
        );
        return Err(AppError::Forbidden(format!(
            "Tool not authorized: {}",
            authz_err
        )));
    }

    // --- Step c: Governance (quota, RBAC) ---
    // MCP calls bypass the REST governance middleware because all tools share a
    // single POST /api/mcp/tools/call path, so classify() cannot distinguish them.
    // Instead, governance is applied here where the tool name is known.
    //
    // When JWT auth is disabled (dev mode), skip governance entirely — there is no
    // meaningful tenant/user identity to enforce against.
    let governance_op = if !crate::config::get().jwt.disabled {
        tool_to_operation(&params.name)
    } else {
        None
    };

    if let Some(op) = governance_op {
        if let Some(hooks) = try_enterprise_hooks() {
            let ctx = HookContext::new(
                tenant_ctx.tenant_id.to_string(),
                op,
                "/api/mcp/tools/call".to_string(),
            )
            .with_user(tenant_ctx.user_id.clone());

            let decision = match op {
                Operation::Store => hooks.pre_store(&ctx),
                Operation::Search => hooks.pre_search(&ctx),
                Operation::Delete => hooks.pre_delete(&ctx),
                Operation::Update => HookDecision::Allow, // unreachable for MCP tools
            };

            if let HookDecision::Deny(reason) = decision {
                warn!(
                    component_id = %params.name,
                    reason = %reason,
                    "MCP tool call denied by governance"
                );
                audit_writer::record_audit(
                    AuditEvent::new("mcp.call_tool", "mcp_tool")
                        .tenant(tenant_ctx.tenant_id.as_str())
                        .with_metadata(&serde_json::json!({
                            "tool": params.name,
                            "outcome": "denied",
                            "reason": "governance",
                            "detail": reason,
                        })),
                );
                return Err(AppError::Forbidden(reason));
            }
        }
    }

    let result = match params.name.as_str() {
        TOOL_MEMORY_WRITE => handle_memory_write(&tenant_ctx.tenant_id, params.arguments).await,
        TOOL_MEMORY_SEARCH => handle_memory_search(&tenant_ctx.tenant_id, params.arguments).await,
        TOOL_MEMORY_RECALL => handle_memory_recall(&tenant_ctx.tenant_id, params.arguments).await,
        TOOL_MEMORY_FORGET => handle_memory_forget(&tenant_ctx.tenant_id, params.arguments).await,
        TOOL_MEMORY_LIST => handle_memory_list(&tenant_ctx.tenant_id, params.arguments).await,
        _ => {
            // Unreachable: capability::authorize rejects unknown tools deny-by-default.
            audit_writer::record_audit(
                AuditEvent::new("mcp.call_tool", "mcp_tool")
                    .tenant(tenant_ctx.tenant_id.as_str())
                    .with_metadata(&serde_json::json!({
                        "tool": params.name,
                        "outcome": "denied",
                        "reason": "unknown_tool",
                    })),
            );
            return Err(AppError::BadRequest(format!(
                "Unknown tool: {}",
                params.name
            )));
        }
    };

    // --- Post-store hook for successful writes ---
    // Mirror governance_middleware: increment quota usage only after a successful
    // Store operation. Failed tool calls must NOT increment usage.
    if governance_op == Some(Operation::Store) && result.is_ok() {
        if let Some(hooks) = try_enterprise_hooks() {
            let ctx = HookContext::new(
                tenant_ctx.tenant_id.to_string(),
                Operation::Store,
                "/api/mcp/tools/call".to_string(),
            )
            .with_user(tenant_ctx.user_id.clone());
            hooks.post_store(&ctx, &HookResult::success());
        }
    }

    match result {
        Ok(response) => {
            audit_writer::record_audit(
                AuditEvent::new("mcp.call_tool", "mcp_tool")
                    .tenant(tenant_ctx.tenant_id.as_str())
                    .with_metadata(&serde_json::json!({
                        "tool": params.name,
                        "outcome": "success",
                    })),
            );
            json_ok(ToolCallResponse {
                content: response,
                is_error: Some(false),
            })
        }
        Err(e) => {
            error!("MCP tool error: {}", e);
            audit_writer::record_audit(
                AuditEvent::new("mcp.call_tool", "mcp_tool")
                    .tenant(tenant_ctx.tenant_id.as_str())
                    .with_metadata(&serde_json::json!({
                        "tool": params.name,
                        "outcome": "error",
                        "error": e.to_string(),
                    })),
            );
            Err(AppError::Internal(e.to_string()))
        }
    }
}

/// Handle memory_write tool
async fn handle_memory_write(
    tenant_id: &TenantId,
    arguments: Option<serde_json::Value>,
) -> Result<Vec<crate::protocol::mcp::ToolContent>, AppError> {
    let args = arguments.ok_or_else(|| AppError::BadRequest("Missing arguments".to_string()))?;

    let content = args["content"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("Missing 'content' parameter".to_string()))?
        .to_string();
    let layer = args["layer"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("Missing 'layer' parameter".to_string()))?
        .to_lowercase();

    let user_id = args["user_id"].as_str().unwrap_or("default").to_string();
    let session_id = args["session_id"].as_str().map(|s| s.to_string());
    let agent_id = args["agent_id"].as_str().map(|s| s.to_string());

    match layer.as_str() {
        "stm" => {
            let agent = agent_id.unwrap_or_else(|| "mcp_agent".to_string());

            let (session_id, message_id) = MemoryStorageService::store_stm_for_tenant(
                tenant_id,
                &user_id,
                &agent,
                "conversation",
                "user",
                &content,
                4000,
                24,
                session_id.as_deref(),
            )
            .await?;

            Ok(vec![crate::protocol::mcp::ToolContent::Text(
                serde_json::json!({
                    "success": true,
                    "layer": "stm",
                    "sessionId": session_id,
                    "messageId": message_id
                })
                .to_string(),
            )])
        }
        "ltm" => {
            let source_id = format!("mcp_{}", ulid::Ulid::new());
            let result = MemoryStorageService::store_ltm_for_tenant(
                tenant_id,
                &source_id,
                "user_input",
                &content,
                None,
            )
            .await?;

            Ok(vec![crate::protocol::mcp::ToolContent::Text(
                serde_json::json!({
                    "success": true,
                    "layer": "ltm",
                    "entryId": result.entry_id,
                    "indexStatus": result.index_status
                })
                .to_string(),
            )])
        }
        "kg" => {
            let entity_name = args["entity_name"]
                .as_str()
                .ok_or_else(|| {
                    AppError::BadRequest("Missing 'entity_name' parameter for KG".to_string())
                })?
                .to_string();
            let entity_type = args["entity_type"].as_str().unwrap_or("concept");
            let description = args["description"].as_str();

            let entity_id = KGRepository::create_entity(
                tenant_id,
                &entity_name,
                entity_type,
                description,
                None,
                None,
                None,
                None,
                1.0,
            )
            .await?;

            Ok(vec![crate::protocol::mcp::ToolContent::Text(
                serde_json::json!({
                    "success": true,
                    "layer": "kg",
                    "entityId": entity_id,
                    "entityName": entity_name,
                    "entityType": entity_type
                })
                .to_string(),
            )])
        }
        "mm" => {
            let modality_type = args["modality_type"].as_str().unwrap_or("text");
            let session_id = session_id.or(Some("mcp_session".to_string()));
            let source_id = format!("mcp_{}", ulid::Ulid::new());

            let entry_id = MMRepository::create_entry(
                session_id.as_deref(),
                &source_id,
                modality_type,
                "{}",
                Some(&content),
                None,
                None,
                None,
                tenant_id.as_str(),
            )
            .await?;

            Ok(vec![crate::protocol::mcp::ToolContent::Text(
                serde_json::json!({
                    "success": true,
                    "layer": "mm",
                    "entryId": entry_id,
                    "modalityType": modality_type
                })
                .to_string(),
            )])
        }
        _ => Err(AppError::BadRequest(format!("Invalid layer: {}", layer))),
    }
}

/// Handle memory_search tool
async fn handle_memory_search(
    tenant_id: &TenantId,
    arguments: Option<serde_json::Value>,
) -> Result<Vec<crate::protocol::mcp::ToolContent>, AppError> {
    let args = arguments.ok_or_else(|| AppError::BadRequest("Missing arguments".to_string()))?;

    let query = args["query"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("Missing 'query' parameter".to_string()))?
        .to_string();
    let layer = args["layer"].as_str().unwrap_or("ltm").to_lowercase();
    let limit = args["limit"].as_u64().unwrap_or(10) as i32;
    let user_id = args["user_id"].as_str();
    let session_id = args["session_id"].as_str();

    let results = match layer.as_str() {
        "stm" => {
            // Search in STM sessions - returns SessionMessage
            let user = user_id.unwrap_or("default");
            let agent = "mcp_agent";
            let messages = MemorySearchService::search_stm_for_tenant(
                tenant_id,
                user,
                agent,
                None,
                Some(limit),
            )
            .await?;
            // Convert to a unified format
            let results: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "id": m.message_id,
                        "score": 1.0,
                        "content": m.content
                    })
                })
                .collect();
            serde_json::json!({ "type": "stm", "results": results })
        }
        "ltm" => {
            // Search in LTM - returns SearchResult
            let search_results = MemorySearchService::search_ltm_for_tenant(
                tenant_id,
                &query,
                limit as usize,
                None,
                None,
            )
            .await?;
            let results_json: Vec<serde_json::Value> = search_results
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "id": m.entry_id,
                        "score": m.score,
                        "content": m.content
                    })
                })
                .collect();
            serde_json::json!({ "type": "ltm", "results": results_json })
        }
        "kg" => {
            // Search in knowledge graph entities
            let entities =
                KGRepository::list_entities(pool(), tenant_id, Some(&query), Some(limit), Some(0))
                    .await?;
            let results_json: Vec<serde_json::Value> = entities
                .entities
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "id": e.entity_id,
                        "name": e.entity_name,
                        "type": e.entity_type,
                        "score": e.confidence_score,
                        "description": e.description
                    })
                })
                .collect();
            serde_json::json!({ "type": "kg", "results": results_json })
        }
        "mm" => {
            // Search in multimodal memories by modality type
            let entries = MMRepository::get_entries_by_modality(
                "text",
                Some(limit),
                tenant_id.as_str(),
            )
            .await?;
            // Filter by query if possible
            let results_json: Vec<serde_json::Value> = entries
                .iter()
                .filter(|e| {
                    e.text_content
                        .as_ref()
                        .map_or(false, |t| t.contains(&query))
                })
                .map(|e| {
                    serde_json::json!({
                        "id": e.entry_id,
                        "score": e.quality_score,
                        "modalityType": e.modality_type,
                        "content": e.text_content
                    })
                })
                .collect();
            serde_json::json!({ "type": "mm", "results": results_json })
        }
        _ => {
            return Err(AppError::BadRequest(format!("Invalid layer: {}", layer)));
        }
    };

    let text = results.to_string();

    Ok(vec![crate::protocol::mcp::ToolContent::Text(text)])
}

/// Handle memory_recall tool
async fn handle_memory_recall(
    tenant_id: &TenantId,
    arguments: Option<serde_json::Value>,
) -> Result<Vec<crate::protocol::mcp::ToolContent>, AppError> {
    let args = arguments.ok_or_else(|| AppError::BadRequest("Missing arguments".to_string()))?;

    let session_id = args["session_id"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("Missing 'session_id' parameter".to_string()))?
        .to_string();
    let limit = args["limit"].as_u64().unwrap_or(10) as i32;

    // Recall memories from a session
    let messages =
        STMRepository::get_session_messages(pool(), tenant_id, &session_id, Some(limit)).await?;

    let text = serde_json::json!({
        "success": true,
        "sessionId": session_id,
        "count": messages.len(),
        "memories": messages.iter().map(|m| {
            serde_json::json!({
                "id": m.message_id,
                "content": m.content,
                "role": m.role,
                "createdAt": m.created_at
            })
        }).collect::<Vec<_>>()
    })
    .to_string();

    Ok(vec![crate::protocol::mcp::ToolContent::Text(text)])
}

/// Handle memory_forget tool
async fn handle_memory_forget(
    tenant_id: &TenantId,
    arguments: Option<serde_json::Value>,
) -> Result<Vec<crate::protocol::mcp::ToolContent>, AppError> {
    let args = arguments.ok_or_else(|| AppError::BadRequest("Missing arguments".to_string()))?;

    let _memory_id = args["memory_id"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("Missing 'memory_id' parameter".to_string()))?
        .to_string();
    let layer = args["layer"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("Missing 'layer' parameter".to_string()))?
        .to_lowercase();

    let response = crate::services::memory_contract::forget_memory(
        tenant_id,
        crate::services::memory_contract::MemoryForgetRequest {
            memory_id: _memory_id,
            layer,
        },
    )
    .await?;

    let text = serde_json::to_string(&response).map_err(|e| {
        AppError::Serialization(format!("Failed to serialize forget response: {e}"))
    })?;

    Ok(vec![crate::protocol::mcp::ToolContent::Text(text)])
}

/// Handle memory_list tool
async fn handle_memory_list(
    tenant_id: &TenantId,
    arguments: Option<serde_json::Value>,
) -> Result<Vec<crate::protocol::mcp::ToolContent>, AppError> {
    let args = arguments.ok_or_else(|| AppError::BadRequest("Missing arguments".to_string()))?;

    let layer = args["layer"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("Missing 'layer' parameter".to_string()))?
        .to_lowercase();
    let limit = args["limit"].as_u64().unwrap_or(20) as i32;
    let offset = args["offset"].as_u64().unwrap_or(0) as i32;
    let user_id = args["user_id"].as_str();

    let entries = match layer.as_str() {
        "stm" => {
            // List STM sessions
            let response = STMRepository::list_sessions(
                pool(),
                tenant_id,
                user_id,
                None,
                Some(limit),
                Some(offset),
            )
            .await?;

            serde_json::json!({
                "type": "stm_sessions",
                "count": response.sessions.len(),
                "sessions": response.sessions.iter().map(|s| {
                    serde_json::json!({
                        "sessionId": s.session_id,
                        "userId": s.user_id,
                        "agentId": s.agent_id,
                        "sessionType": s.session_type,
                        "status": s.status,
                        "createdAt": s.created_at
                    })
                }).collect::<Vec<_>>()
            })
            .to_string()
        }
        "ltm" => {
            // List LTM entries
            let response = LTMRepository::list_entries(
                pool(),
                tenant_id,
                None,
                None,
                Some(limit),
                Some(offset),
            )
            .await?;

            serde_json::json!({
                "type": "ltm_entries",
                "count": response.entries.len(),
                "entries": response.entries.iter().map(|e| {
                    serde_json::json!({
                        "id": e.entry_id,
                        "content": e.content,
                        "title": e.title,
                        "createdAt": e.created_at
                    })
                }).collect::<Vec<_>>()
            })
            .to_string()
        }
        "kg" => {
            // List KG entities
            let entity_type = args["entity_type"].as_str();
            let response = KGRepository::list_entities(
                pool(),
                tenant_id,
                entity_type,
                Some(limit),
                Some(offset),
            )
            .await?;

            serde_json::json!({
                "type": "kg_entities",
                "count": response.entities.len(),
                "entities": response.entities.iter().map(|e| {
                    serde_json::json!({
                        "id": e.entity_id,
                        "name": e.entity_name,
                        "type": e.entity_type,
                        "description": e.description,
                        "confidenceScore": e.confidence_score,
                        "createdAt": e.created_at
                    })
                }).collect::<Vec<_>>()
            })
            .to_string()
        }
        "mm" => {
            // List MM entries
            let modality_type = args["modality_type"].as_str();
            let response = MMRepository::list_entries(
                modality_type,
                Some(limit),
                Some(offset),
                tenant_id.as_str(),
            )
            .await?;

            serde_json::json!({
                "type": "mm_entries",
                "count": response.entries.len(),
                "entries": response.entries.iter().map(|e| {
                    serde_json::json!({
                        "id": e.entry_id,
                        "sessionId": e.session_id,
                        "modalityType": e.modality_type,
                        "title": e.title,
                        "qualityScore": e.quality_score,
                        "createdAt": e.created_at
                    })
                }).collect::<Vec<_>>()
            })
            .to_string()
        }
        _ => {
            return Err(AppError::BadRequest(format!("Invalid layer: {}", layer)));
        }
    };

    Ok(vec![crate::protocol::mcp::ToolContent::Text(entries)])
}

/// List available MCP resources - only returns resources with valid signatures
async fn list_resources(State(state): State<McpState>) -> JsonResult<serde_json::Value> {
    let registry = &state.component_registry;
    let key_bundle = &registry.key_bundle;

    let all_resources = get_memory_resources();
    let mut verified_resources = Vec::new();

    // Try to load signatures from environment
    let signatures_json = std::env::var("MCP_RESOURCE_SIGNATURES").unwrap_or_default();
    let signatures: Vec<ComponentSignature> =
        serde_json::from_str(&signatures_json).unwrap_or_default();

    for resource in all_resources {
        // Find signature for this resource
        if let Some(sig) = signatures.iter().find(|s| s.component_id == resource.uri) {
            let artifact = match serde_json::to_vec(&resource) {
                Ok(a) => a,
                Err(e) => {
                    warn!("MCP resource {} failed to serialize: {}", resource.uri, e);
                    continue;
                }
            };

            // D-01: Verify signature on load
            if let Err(e) = verify_component(&resource.uri, &artifact, sig, key_bundle) {
                // D-03: Reject unsigned/invalid components with structured log
                warn!(
                    component_id = %resource.uri,
                    issuer = %sig.issuer,
                    error = %e,
                    "MCP resource rejected: signing verification failed"
                );
                continue;
            }

            verified_resources.push(resource);
        } else {
            // No signature found - reject unsigned component (D-03)
            warn!(
                component_id = %resource.uri,
                "MCP resource rejected: no signature found"
            );
        }
    }

    json_ok(serde_json::json!({
        "resources": verified_resources
    }))
}

/// Read resource content request
#[derive(Debug, Deserialize)]
pub struct ResourceReadRequest {
    pub uri: String,
}

/// Read resource content
async fn read_resource(
    Extension(tenant_ctx): Extension<RequestTenantContext>,
    Json(req): Json<ResourceReadRequest>,
) -> JsonResult<ResourceContentResponse> {
    info!("MCP resource read: {}", req.uri);

    // Parse URI: memory://layer/id
    let parts: Vec<&str> = req
        .uri
        .strip_prefix("memory://")
        .unwrap_or(&req.uri)
        .split('/')
        .collect();

    if parts.is_empty() {
        return Err(AppError::BadRequest("Invalid resource URI".to_string()));
    }

    let layer = parts[0];
    let id = parts.get(1).map(|s| s.to_string());
    let tenant_id = &tenant_ctx.tenant_id;

    let content = match layer {
        "stm" => {
            if let Some(session_id) = id {
                let messages =
                    STMRepository::get_session_messages(pool(), tenant_id, &session_id, Some(50))
                        .await?;

                serde_json::json!({
                    "sessionId": session_id,
                    "messages": messages
                })
                .to_string()
            } else {
                // List all sessions
                let response =
                    STMRepository::list_sessions(pool(), tenant_id, None, None, Some(20), Some(0))
                        .await?;

                serde_json::json!({
                    "sessions": response.sessions
                })
                .to_string()
            }
        }
        "ltm" => {
            if let Some(entry_id) = id {
                let entry = LTMRepository::get_entry_by_id(pool(), tenant_id, &entry_id).await?;

                match entry {
                    Some(e) => serde_json::json!({
                        "entry": {
                            "id": e.entry_id,
                            "content": e.content,
                            "title": e.title,
                            "createdAt": e.created_at
                        }
                    })
                    .to_string(),
                    None => {
                        return Err(AppError::NotFound(format!("Entry {} not found", entry_id)));
                    }
                }
            } else {
                let response =
                    LTMRepository::list_entries(pool(), tenant_id, None, None, Some(20), Some(0))
                        .await?;

                serde_json::json!({
                    "entries": response.entries
                })
                .to_string()
            }
        }
        "kg" => {
            // Knowledge graph resources
            if let Some(entity_id) = id {
                let entity = KGRepository::get_entity_by_id(pool(), tenant_id, &entity_id).await?;

                match entity {
                    Some(e) => {
                        // Get related entities
                        let related = KGRepository::get_related_entities(
                            pool(),
                            tenant_id,
                            &entity_id,
                            None,
                            Some(5),
                        )
                        .await?;
                        let relations: Vec<serde_json::Value> = related
                            .iter()
                            .map(|(ent, rel)| {
                                serde_json::json!({
                                    "entityId": ent.entity_id,
                                    "entityName": ent.entity_name,
                                    "relationType": rel.relation_type,
                                    "weight": rel.weight
                                })
                            })
                            .collect();

                        serde_json::json!({
                            "entity": {
                                "id": e.entity_id,
                                "name": e.entity_name,
                                "type": e.entity_type,
                                "description": e.description,
                                "attributes": e.attributes,
                                "confidenceScore": e.confidence_score,
                                "popularityScore": e.popularity_score,
                                "relationCount": e.relation_count,
                                "createdAt": e.created_at
                            },
                            "relations": relations
                        })
                        .to_string()
                    }
                    None => {
                        return Err(AppError::NotFound(format!(
                            "Entity {} not found",
                            entity_id
                        )));
                    }
                }
            } else {
                // List all entities
                let response =
                    KGRepository::list_entities(pool(), tenant_id, None, Some(20), Some(0)).await?;

                serde_json::json!({
                    "entities": response.entities.iter().map(|e| {
                        serde_json::json!({
                            "id": e.entity_id,
                            "name": e.entity_name,
                            "type": e.entity_type,
                            "description": e.description,
                            "confidenceScore": e.confidence_score
                        })
                    }).collect::<Vec<_>>()
                })
                .to_string()
            }
        }
        "mm" => {
            // Multimodal resources
            if let Some(entry_id) = id {
                let entry =
                    MMRepository::get_entry_by_id(&entry_id, tenant_id.as_str()).await?;

                match entry {
                    Some(e) => {
                        // Get related entries
                        let related = MMRepository::get_related_entries(
                            &entry_id,
                            Some(5),
                            tenant_id.as_str(),
                        )
                        .await?;
                        let relations: Vec<serde_json::Value> = related
                            .iter()
                            .map(|(ent, rel)| {
                                serde_json::json!({
                                    "entryId": ent.entry_id,
                                    "modalityType": ent.modality_type,
                                    "relationType": rel.relation_type,
                                    "strength": rel.relation_strength
                                })
                            })
                            .collect();

                        serde_json::json!({
                            "entry": {
                                "id": e.entry_id,
                                "sessionId": e.session_id,
                                "sourceId": e.source_id,
                                "modalityType": e.modality_type,
                                "title": e.title,
                                "description": e.description,
                                "textContent": e.text_content,
                                "imageUrl": e.image_url,
                                "audioUrl": e.audio_url,
                                "videoUrl": e.video_url,
                                "qualityScore": e.quality_score,
                                "createdAt": e.created_at
                            },
                            "relations": relations
                        })
                        .to_string()
                    }
                    None => {
                        return Err(AppError::NotFound(format!("Entry {} not found", entry_id)));
                    }
                }
            } else {
                // List all entries
                let response =
                    MMRepository::list_entries(None, Some(20), Some(0), tenant_id.as_str())
                        .await?;

                serde_json::json!({
                    "entries": response.entries.iter().map(|e| {
                        serde_json::json!({
                            "id": e.entry_id,
                            "sessionId": e.session_id,
                            "modalityType": e.modality_type,
                            "title": e.title,
                            "qualityScore": e.quality_score,
                            "createdAt": e.created_at
                        })
                    }).collect::<Vec<_>>()
                })
                .to_string()
            }
        }
        _ => {
            return Err(AppError::NotFound(format!("Unknown layer: {}", layer)));
        }
    };

    json_ok(ResourceContentResponse {
        contents: vec![ResourceContent {
            uri: req.uri,
            mime_type: Some("application/json".to_string()),
            text: Some(content),
            data: None,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_write_maps_to_store() {
        assert_eq!(
            tool_to_operation(TOOL_MEMORY_WRITE),
            Some(Operation::Store)
        );
    }

    #[test]
    fn memory_forget_maps_to_delete() {
        assert_eq!(
            tool_to_operation(TOOL_MEMORY_FORGET),
            Some(Operation::Delete)
        );
    }

    #[test]
    fn memory_search_maps_to_search() {
        assert_eq!(
            tool_to_operation(TOOL_MEMORY_SEARCH),
            Some(Operation::Search)
        );
    }

    #[test]
    fn memory_recall_maps_to_search() {
        assert_eq!(
            tool_to_operation(TOOL_MEMORY_RECALL),
            Some(Operation::Search)
        );
    }

    #[test]
    fn memory_list_maps_to_search() {
        assert_eq!(
            tool_to_operation(TOOL_MEMORY_LIST),
            Some(Operation::Search)
        );
    }

    #[test]
    fn unknown_tool_returns_none() {
        assert_eq!(tool_to_operation("memory_unknown"), None);
        assert_eq!(tool_to_operation(""), None);
        assert_eq!(tool_to_operation("some_random_tool"), None);
    }

    #[test]
    fn all_known_tools_are_covered() {
        let known = [
            TOOL_MEMORY_WRITE,
            TOOL_MEMORY_SEARCH,
            TOOL_MEMORY_RECALL,
            TOOL_MEMORY_FORGET,
            TOOL_MEMORY_LIST,
        ];
        for &tool in &known {
            assert!(
                tool_to_operation(tool).is_some(),
                "known tool {tool} must map to an operation"
            );
        }
    }
}
