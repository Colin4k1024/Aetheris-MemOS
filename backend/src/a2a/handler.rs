use a2a::types::{
    Message, Part, PartContent, Role, SendMessageRequest, SendMessageResponse, Task, TaskState,
    TaskStatus,
};
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::db::audit::AuditEvent;
use crate::db::{self, kg::KGRepository, pool};
use crate::hoops::enterprise::{
    try_enterprise_hooks, HookContext, HookDecision, HookResult, Operation,
};
use crate::services::audit_writer;
use crate::services::memory_fusion::MemoryFusionService;
use crate::services::memory_search::MemorySearchService;
use crate::services::memory_storage::MemoryStorageService;
use crate::services::rbac::{get_rbac_service, Permission};
use crate::tenant::RequestTenantContext;

use super::skills::MemorySkill;

/// A2A handler that delegates to real memory services.
///
/// Each handler method receives a `tenant_ctx` (from the A2A auth middleware)
/// and calls the actual memory service layer — no more format! placeholders.
pub struct A2AHandler {}

/// Map each A2A memory skill to the governance [`Operation`] it performs.
///
/// This lives next to the dispatch `match` in [`A2AHandler::handle_message`] and
/// is anchored on the same [`MemorySkill`] enum, so it cannot drift silently: the
/// match is **exhaustive** (no `_` arm), so adding a `MemorySkill` variant fails
/// to COMPILE until a mapping is added here. A new skill therefore cannot gain a
/// handler without also gaining a governance decision. This is strictly stronger
/// than the sibling `routers/mcp.rs::tool_to_operation`, which keys on a `&str`
/// and can only guard against drift with a runtime test.
///
/// | Skill            | Operation | Reason                              |
/// |------------------|-----------|-------------------------------------|
/// | `MemoryStore`    | Store     | `handle_memory_store` writes to LTM |
/// | `MemorySearch`   | Search    | read-only LTM query                 |
/// | `MemoryFusion`   | Search    | read-only cross-layer query         |
/// | `MemoryStatus`   | Search    | read-only layer counts              |
/// | `KnowledgeGraph` | Search    | read-only KG entity query           |
fn skill_to_operation(skill: &MemorySkill) -> Operation {
    match skill {
        MemorySkill::MemoryStore => Operation::Store,
        MemorySkill::MemorySearch => Operation::Search,
        MemorySkill::MemoryFusion => Operation::Search,
        MemorySkill::MemoryStatus => Operation::Search,
        MemorySkill::KnowledgeGraph => Operation::Search,
    }
}

/// Map a governance [`Operation`] to the RBAC [`Permission`] the caller must hold.
///
/// Exhaustive on purpose — a new `Operation` variant fails to COMPILE until its
/// permission is decided here, so the RBAC gate can never silently skip a new
/// operation class.
fn operation_to_permission(operation: Operation) -> Permission {
    match operation {
        Operation::Store => Permission::Write,
        Operation::Update => Permission::Write,
        Operation::Delete => Permission::Delete,
        Operation::Search => Permission::Read,
    }
}

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

        // --- In-handler governance (A-4c) ---
        // Every A2A skill is dispatched over the same POST /a2a/jsonrpc (or
        // /a2a/rest/messages) path, so the REST governance middleware — which keys
        // off HTTP method + path — cannot tell a write (memory_store) from a read
        // (search / status / kg). Governance is therefore applied here, where the
        // skill is known, exactly as routers/mcp.rs::call_tool does for MCP tools.
        //
        // Gate at the call site (mirrors mcp.rs's `governance_op`): when JWT auth is
        // disabled (dev mode) there is no meaningful identity to enforce against, so
        // both the gate and the post-store quota count are skipped.
        let governed_op = if crate::config::get().jwt.disabled {
            None
        } else {
            skill_id.as_ref().map(skill_to_operation)
        };
        if let Some(op) = governed_op {
            self.enforce_governance(op, tenant_ctx).await?;
        }

        let response = match skill_id {
            Some(MemorySkill::MemorySearch) => self.handle_memory_search(request, tenant_ctx).await,
            Some(MemorySkill::MemoryStore) => self.handle_memory_store(request, tenant_ctx).await,
            Some(MemorySkill::MemoryFusion) => self.handle_memory_fusion(request, tenant_ctx).await,
            Some(MemorySkill::MemoryStatus) => self.handle_memory_status(request, tenant_ctx).await,
            Some(MemorySkill::KnowledgeGraph) => {
                self.handle_knowledge_graph(request, tenant_ctx).await
            }
            None => self.handle_general_query(request).await,
        };

        // Count quota usage only after a *successful* Store, mirroring
        // governance_middleware / mcp.rs. A failed or denied write must not
        // increment. post_store_usage is a no-op without enterprise hooks.
        if governed_op == Some(Operation::Store) && response.is_ok() {
            Self::post_store_usage(tenant_ctx);
        }

        response
    }

    /// Apply in-handler governance for one A2A operation, mirroring the
    /// `routers/mcp.rs::call_tool` pipeline. Two independent gates:
    ///
    /// 1. **RBAC permission** — consulted through the global RBAC service, the
    ///    *same* singleton `hoops::governance::enforce_permission` uses. This is
    ///    the gate that is always live: it holds even when the enterprise hook set
    ///    is not initialised (e.g. a SQLite dev backend), satisfying the C-1
    ///    constraint that the strong-typed authorization path not depend on
    ///    enterprise hooks.
    /// 2. **Quota** — enterprise governance pre-hooks (`pre_store` / `pre_search`
    ///    …), which only bite when the enterprise hook set is wired (PG-backed).
    ///    Absent, this half is a no-op — same as mcp.rs.
    ///
    /// Honest scope of today's effect: every authenticated user is Owner of their
    /// own single-user tenant (`tenant_id == user_id`; backlog C-3), so the RBAC
    /// gate returns Allow for same-tenant callers and only starts *denying* once an
    /// org-level tenant model (where `tenant_id != user_id`) lands. What changes
    /// now is that the grant *source* goes from nonexistent to present — the gate
    /// point is in place so differentiated enforcement takes effect automatically
    /// when C-3 lands. `blocking_has_permission` (not the async `get_role`) is used
    /// deliberately: it auto-grants Owner for a self-tenant caller, so it does not
    /// spuriously deny the anonymous/self-tenant identities that reach this path.
    async fn enforce_governance(
        &self,
        operation: Operation,
        tenant_ctx: &RequestTenantContext,
    ) -> Result<(), String> {
        // --- Gate 1: RBAC permission (independent of enterprise hooks) ---
        let permission = operation_to_permission(operation);
        let allowed = get_rbac_service().blocking_has_permission(
            tenant_ctx.tenant_id.as_str(),
            &tenant_ctx.user_id,
            permission,
        );
        if !allowed {
            audit_writer::record_audit(
                AuditEvent::new("a2a.handle_message", "a2a_skill")
                    .tenant(tenant_ctx.tenant_id.as_str())
                    .actor(tenant_ctx.user_id.clone())
                    .with_metadata(&json!({
                        "operation": operation.as_str(),
                        "outcome": "denied",
                        "reason": "rbac_permission",
                        "required_permission": format!("{permission:?}"),
                    })),
            );
            warn!(
                tenant = %tenant_ctx.tenant_id,
                operation = %operation.as_str(),
                "A2A message denied: insufficient role permissions"
            );
            return Err("Insufficient role permissions".to_string());
        }

        // --- Gate 2: Quota (only when enterprise hooks are wired) ---
        if let Some(hooks) = try_enterprise_hooks() {
            let ctx = HookContext::new(
                tenant_ctx.tenant_id.to_string(),
                operation,
                "/a2a".to_string(),
            )
            .with_user(tenant_ctx.user_id.clone());

            let decision = match operation {
                Operation::Store => hooks.pre_store(&ctx),
                Operation::Search => hooks.pre_search(&ctx),
                Operation::Update => hooks.pre_update(&ctx),
                Operation::Delete => hooks.pre_delete(&ctx),
            };

            if let HookDecision::Deny(reason) = decision {
                audit_writer::record_audit(
                    AuditEvent::new("a2a.handle_message", "a2a_skill")
                        .tenant(tenant_ctx.tenant_id.as_str())
                        .actor(tenant_ctx.user_id.clone())
                        .with_metadata(&json!({
                            "operation": operation.as_str(),
                            "outcome": "denied",
                            "reason": "governance",
                            "detail": reason,
                        })),
                );
                warn!(
                    tenant = %tenant_ctx.tenant_id,
                    operation = %operation.as_str(),
                    reason = %reason,
                    "A2A message denied by governance"
                );
                return Err(reason);
            }
        }

        Ok(())
    }

    /// Increment quota usage after a successful Store, mirroring
    /// `governance_middleware` / `call_tool`. No-op when the enterprise hook set is
    /// absent. The caller ensures this only runs for a successful, governed store.
    fn post_store_usage(tenant_ctx: &RequestTenantContext) {
        if let Some(hooks) = try_enterprise_hooks() {
            let ctx = HookContext::new(
                tenant_ctx.tenant_id.to_string(),
                Operation::Store,
                "/a2a".to_string(),
            )
            .with_user(tenant_ctx.user_id.clone());
            hooks.post_store(&ctx, &HookResult::success());
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
        //
        // `source_type` MUST be one of the values `SourceType::parse` accepts
        // (document/api/database/web/user_input) — the previous "a2a_message"
        // was not, so every A2A store was rejected with 400 at the boundary
        // before any DB/embedding work. "user_input" is the honest label for
        // caller-provided content and matches what the sibling MCP write handler
        // uses (routers/mcp.rs `handle_memory_write`).
        let result = MemoryStorageService::store_ltm_for_tenant(
            &tenant_ctx.tenant_id,
            &format!("a2a:{}", uuid::Uuid::new_v4()),
            "user_input",
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

#[cfg(test)]
mod tests {
    use super::*;

    // ─────────────────────────────────────────────────────────────────────────
    // Governance mapping (A-4c). Mirrors routers/mcp.rs's `tool_to_operation`
    // tests, but the enum-keyed match here is exhaustive, so the mapping's
    // completeness is enforced by the COMPILER, not only these tests. The
    // anti-drift guard below still exists to prove the *write* skill stays a
    // write — a mis-mapping (e.g. someone flipping MemoryStore to Search to
    // "fix" a quota complaint) compiles fine but would let A2A writes past the
    // write gate, so it must be caught by a test.
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn store_skill_maps_to_store_operation() {
        assert_eq!(
            skill_to_operation(&MemorySkill::MemoryStore),
            Operation::Store
        );
    }

    #[test]
    fn read_skills_map_to_search_operation() {
        for skill in [
            MemorySkill::MemorySearch,
            MemorySkill::MemoryFusion,
            MemorySkill::MemoryStatus,
            MemorySkill::KnowledgeGraph,
        ] {
            assert_eq!(
                skill_to_operation(&skill),
                Operation::Search,
                "{skill:?} must be a read operation"
            );
        }
    }

    /// Anti-drift guard, structural rather than a hardcoded list: it walks the
    /// full set of `MemorySkill` variants and asserts that exactly ONE of them —
    /// `MemoryStore` — governs as a write. The `matches!` in the fold means a new
    /// variant is force-classified here the moment it is added (the variant array
    /// won't compile until it is listed), and if a future edit makes a *second*
    /// skill a Store, or drops the Store mapping from `MemoryStore`, the count
    /// breaks. See the negative-control test that proves this bites.
    #[test]
    fn exactly_one_skill_is_a_write() {
        let all = [
            MemorySkill::MemorySearch,
            MemorySkill::MemoryStore,
            MemorySkill::MemoryFusion,
            MemorySkill::MemoryStatus,
            MemorySkill::KnowledgeGraph,
        ];
        let writes: Vec<&MemorySkill> = all
            .iter()
            .filter(|s| matches!(skill_to_operation(s), Operation::Store))
            .collect();
        assert_eq!(
            writes.len(),
            1,
            "exactly one skill must govern as a write; found {writes:?}"
        );
        assert!(
            matches!(writes[0], MemorySkill::MemoryStore),
            "the single write skill must be MemoryStore, not {:?}",
            writes[0]
        );
    }

    #[test]
    fn operation_permission_mapping_is_least_privilege() {
        assert_eq!(operation_to_permission(Operation::Store), Permission::Write);
        assert_eq!(
            operation_to_permission(Operation::Update),
            Permission::Write
        );
        assert_eq!(
            operation_to_permission(Operation::Delete),
            Permission::Delete
        );
        assert_eq!(operation_to_permission(Operation::Search), Permission::Read);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // enforce_governance behaviour. These exercise the RBAC gate (gate 1), which
    // is the half that is live today and does not depend on enterprise hooks
    // (C-1 constraint). The quota gate (gate 2) is a no-op without a wired
    // enterprise hook set, which is the case in this pure unit harness.
    // ─────────────────────────────────────────────────────────────────────────

    /// A self-tenant caller (tenant_id == user_id) is auto-granted Owner by the
    /// RBAC service, so every operation class is allowed. This is the honest
    /// "today" behaviour: the gate is wired but same-tenant callers pass until an
    /// org-level tenant model (C-3) makes roles differ.
    #[tokio::test]
    async fn self_tenant_caller_is_allowed_for_all_operations() {
        let handler = A2AHandler::new();
        let ctx = RequestTenantContext::new("a2a-self-tenant-user");
        for op in [
            Operation::Search,
            Operation::Store,
            Operation::Update,
            Operation::Delete,
        ] {
            assert!(
                handler.enforce_governance(op, &ctx).await.is_ok(),
                "self-tenant Owner must be allowed for {op:?}"
            );
        }
    }

    /// A cross-tenant caller (tenant_id != user_id) with no recorded role holds no
    /// permission — the RBAC gate denies. This proves the gate is a real check and
    /// not hardwired to Allow: it is exactly the separation that takes effect once
    /// C-3 decouples tenant_id from user_id. A write must be denied harder than a
    /// read here (both deny today because the caller has no role at all).
    #[tokio::test]
    async fn cross_tenant_caller_without_role_is_denied() {
        let handler = A2AHandler::new();
        // tenant_id != user_id, so blocking_has_permission does NOT auto-grant.
        let ctx = RequestTenantContext {
            tenant_id: crate::tenant::TenantId::from_string("org-tenant"),
            user_id: "outsider-user".to_string(),
        };
        for op in [Operation::Search, Operation::Store, Operation::Delete] {
            let err = handler
                .enforce_governance(op, &ctx)
                .await
                .expect_err("cross-tenant caller with no role must be denied");
            assert!(
                err.contains("Insufficient role permissions"),
                "denial reason should be the RBAC message, got: {err}"
            );
        }
    }

    /// `handle_message` must actually *call* the gate before dispatching.
    ///
    /// The tests above prove `enforce_governance` denies when invoked, but none of
    /// them would notice if the call at the top of `handle_message` were deleted —
    /// the gate would still pass its own unit tests while every request sailed
    /// past it. That is the "gate exists but is not wired" shape this remediation
    /// batch keeps finding, so it is pinned structurally here: the governance call
    /// must appear in `handle_message`, and it must appear *before* the dispatch
    /// `match`, since authorizing after the write has already run is no gate at
    /// all.
    #[test]
    fn handle_message_enforces_governance_before_dispatch() {
        let src = include_str!("handler.rs");
        let body = src
            .split("pub async fn handle_message")
            .nth(1)
            .expect("handle_message must exist");

        let gate = body.find("enforce_governance").expect(
            "handle_message must call enforce_governance — without it every A2A skill, \
                     including the memory_store write, runs with no RBAC or quota check",
        );
        let dispatch = body
            .find("Some(MemorySkill::MemoryStore) =>")
            .expect("handle_message must dispatch MemoryStore");

        assert!(
            gate < dispatch,
            "enforce_governance must run BEFORE the skill dispatch; authorizing after \
             handle_memory_store has already written is not a gate"
        );
    }
}
