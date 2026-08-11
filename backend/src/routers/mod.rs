use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

mod agent;
mod auth;
mod billing;
mod dashboard;
mod data_io;
mod demo;
mod distributed;
mod enterprise;
mod knowledge_graph;
mod mcp;
#[allow(dead_code)]
mod memory;
mod memory_pool;
mod memory_search;
mod memory_storage;
#[allow(dead_code)]
mod metrics;
#[allow(dead_code)]
mod multi_tenant_router;
#[allow(dead_code)]
mod multimodal;
#[allow(dead_code)]
mod planner;
mod probes;
mod procedural;
mod security;
#[allow(dead_code)]
mod snapshot;
#[allow(dead_code)]
mod tenant;
mod tracing;
mod user;
mod visualization;
mod workflows;

use std::sync::Arc;

use crate::layers::procedural_layer::ProceduralMemoryLayer;
use crate::{config, hoops, services::prometheus_exporter};

#[derive(RustEmbed)]
#[folder = "assets"]
struct Assets;

pub fn root() -> Router {
    let _ = &config::get().jwt;
    let auth_layer = middleware::from_fn(hoops::jwt::auth_middleware);
    let trusted_proxies = config::get().trusted_proxies.clone();
    let rate_limit_state = hoops::rate_limit_state(100, 60, trusted_proxies.clone());
    let memory_rate_limit =
        middleware::from_fn_with_state(rate_limit_state, hoops::rate_limit_middleware);
    let governance_layer = middleware::from_fn(hoops::governance::governance_middleware);

    let user_routes = Router::new()
        .route("/users", get(user::list_users).post(user::create_user))
        .route(
            "/users/{user_id}",
            put(user::update_user).delete(user::delete_user),
        )
        .route_layer(auth_layer.clone());

    let memory_config_routes = Router::new()
        .route(
            "/configs",
            get(memory::list_memory_configs).post(memory::create_memory_config),
        )
        .route(
            "/configs/{config_id}",
            get(memory::get_memory_config)
                .put(memory::update_memory_config)
                .delete(memory::delete_memory_config),
        )
        .route_layer(auth_layer.clone());

    let workflow_evidence_routes = Router::new()
        .route("/{id}/evidence", get(memory::get_workflow_evidence))
        .route_layer(auth_layer.clone());

    let memory_routes = Router::new()
        // Canonical adaptive endpoints
        .route("/adaptive/select", post(memory::select_memory_config))
        .route("/adaptive/status", get(memory::get_memory_status))
        // Backward-compatible aliases (to be deprecated in docs)
        .route(
            "/adaptive",
            post(memory::select_memory_config).get(memory::get_memory_status),
        )
        .route("/adaptive/trace", post(memory::select_memory_config_trace))
        .route("/traces", get(memory::get_decision_traces))
        .route("/explain", get(memory::explain_memory_selection))
        .route("/feedback", post(memory::record_memory_feedback))
        .route("/forget", post(memory::forget_memory))
        .route(
            "/analyzer/task-characteristics",
            post(memory::analyze_task_characteristics),
        )
        .route(
            "/analyzer/batch-characteristics",
            post(memory::batch_analyze_characteristics),
        )
        .route("/predictor/performance", post(memory::predict_performance))
        .route("/predictor/baselines", get(memory::get_baselines))
        .route("/monitor/resources", get(memory::get_resources))
        .route(
            "/monitor/cost-benefit",
            post(memory::calculate_cost_benefit),
        )
        .route("/monitor/optimize", post(memory::optimize))
        .route("/weights/adjust", post(memory::adjust_weights))
        .route("/weights/history", get(memory::get_weight_history))
        .route("/weights/status", get(memory::get_weight_status))
        .route("/health", get(memory::health_check))
        .route("/v1/health", get(memory::self_healing_health))
        .route("/config", get(memory::get_config))
        .route("/importance/{entry_id}", get(memory::get_importance))
        .route("/importance/batch", post(memory::batch_importance))
        .route("/fusion/status", get(memory::fusion_status))
        .route("/fusion/query", post(memory::fusion_query))
        .nest(
            "/storage",
            Router::new()
                .route("/sessions", get(memory_storage::list_sessions))
                .route("/stm", post(memory_storage::store_stm))
                .route(
                    "/stm/{session_id}",
                    get(memory_storage::get_session_messages),
                )
                .route("/ltm", post(memory_storage::store_ltm))
                .route("/transfer", post(memory_storage::transfer_stm_to_ltm))
                .route("/batch-ltm", post(memory_storage::batch_store_ltm))
                .route(
                    "/qdrant/backfill-tenant-metadata",
                    post(memory_storage::backfill_qdrant_tenant_metadata),
                )
                .route("/compress/session", post(memory_storage::compress_session))
                .route(
                    "/compress/messages",
                    post(memory_storage::compress_messages),
                ),
        )
        .nest(
            "/search",
            Router::new()
                .route("/stm", post(memory_search::search_stm))
                .route(
                    "/ltm",
                    get(memory_search::list_ltm_entries).post(memory_search::search_ltm),
                )
                .route("/ltm/{entry_id}", get(memory_search::get_ltm_entry))
                // Bi-temporal tracking endpoints
                .route("/ltm/{entry_id}/at", get(memory_search::get_ltm_at_time))
                .route(
                    "/ltm/{entry_id}/history",
                    get(memory_search::get_ltm_history),
                )
                .route("/ltm/time-travel", post(memory_search::search_ltm_at_time))
                .route(
                    "/kg/{entity_id}/at",
                    get(memory_search::get_kg_entity_at_time),
                )
                .route(
                    "/kg/{entity_id}/history",
                    get(memory_search::get_kg_entity_history),
                )
                .route("/hybrid", post(memory_search::hybrid_search))
                .route("/entity", post(memory_search::search_by_entity))
                .route("/triple", post(memory_search::triple_hybrid_search))
                .route("/scored", post(memory_search::scored_search)),
        )
        .merge(memory_config_routes)
        // Snapshot routes (Oris Integration)
        .nest(
            "/snapshot",
            Router::new()
                .route("/task", post(snapshot::create_task))
                .route("/task/{task_id}", get(snapshot::get_task))
                .route("/create", post(snapshot::create_snapshot))
                .route("/restore", post(snapshot::restore_snapshot))
                .route("/checkpoint", post(snapshot::create_checkpoint))
                .route("/rollback", post(snapshot::rollback_to_checkpoint))
                .route("/checkpoints/{task_id}", get(snapshot::list_checkpoints)),
        )
        // Memory pool routes (Multi-agent Collaborative)
        .nest(
            "/memory-pool",
            Router::new()
                .route("/register", post(memory_pool::register_agent))
                .route(
                    "/unregister/{agent_id}",
                    post(memory_pool::unregister_agent),
                )
                .route("/share/{owner_agent_id}", post(memory_pool::share_memory))
                .route(
                    "/revoke/{owner_agent_id}/{memory_id}",
                    post(memory_pool::revoke_memory),
                )
                .route(
                    "/visible/{agent_id}",
                    get(memory_pool::get_visible_memories),
                )
                .route("/correlations", post(memory_pool::add_correlation))
                .route(
                    "/correlations/{memory_id}",
                    get(memory_pool::get_correlations),
                )
                .route("/network", get(memory_pool::get_network_status))
                .route("/agents", get(memory_pool::list_agents)),
        )
        // Billing routes
        .nest(
            "/billing",
            Router::new()
                .route("/init", post(billing::init_tenant))
                .route("/usage", post(billing::get_usage))
                .route("/usage/{tenant_id}", get(billing::get_current_usage))
                .route("/quota/{tenant_id}", get(billing::get_quota_status))
                .route("/record", post(billing::record_usage)),
        )
        // P0 cleanup: enterprise cluster/shard routes removed from public API.
        // The in-memory fake cluster (services/enterprise.rs + routers/enterprise.rs)
        // is preserved for P2 re-implementation with real PG advisory-lock coordination
        // (see ADR-0006). Do NOT re-mount these routes until the implementation is real.
        //
        // Procedural memory routes
        .nest(
            "/procedural",
            Router::new()
                .route("/store", post(procedural::store_procedural))
                .route("/search", post(procedural::search_procedural))
                .with_state(Arc::new(ProceduralMemoryLayer::new())),
        )
        // GraphRAG hybrid search route
        .route("/search/graphrag", post(procedural::graphrag_hybrid_search))
        // Provider health route
        .route("/provider/health", get(procedural::provider_health))
        // Visualization routes (for Widget Studio)
        .nest(
            "/visualization",
            Router::new()
                .route("/timeline", get(visualization::get_timeline))
                .route("/graph", get(visualization::get_graph_visualization))
                .route("/heatmap", get(visualization::get_heatmap))
                .route("/dashboard", get(visualization::get_dashboard_stats))
                // Metrics routes
                .route("/metrics", get(metrics::get_metrics)),
        )
        .route_layer(governance_layer.clone())
        .route_layer(memory_rate_limit);

    let agent_routes = Router::new()
        // Agent Identity
        .route("/agents", post(agent::create_agent).get(agent::list_agents))
        .route(
            "/agents/{agent_id}",
            get(agent::get_agent)
                .put(agent::update_agent)
                .delete(agent::delete_agent),
        )
        // Self-Model
        .route(
            "/agents/{agent_id}/self-model",
            get(agent::get_self_model).put(agent::update_self_model),
        )
        .route(
            "/agents/{agent_id}/self-model/reflect",
            post(agent::trigger_reflection),
        )
        // Capabilities
        .route(
            "/agents/{agent_id}/capabilities",
            get(agent::list_capabilities).post(agent::add_capability),
        )
        .route(
            "/agents/{agent_id}/capabilities/{capability_id}",
            put(agent::update_capability).delete(agent::delete_capability),
        )
        // Episodes
        .route(
            "/agents/{agent_id}/episodes",
            get(agent::list_episodes).post(agent::record_episode),
        )
        .route(
            "/agents/{agent_id}/episodes/{episode_id}",
            put(agent::update_episode),
        )
        // Behavior Profiles
        .route(
            "/agents/{agent_id}/behaviors",
            get(agent::list_behaviors).post(agent::record_behavior),
        )
        // Complete agent info
        .route(
            "/agents/{agent_id}/complete",
            get(agent::get_agent_complete),
        )
        // Backlog C-1: agent identity/self-model CRUD is privileged (ManageAgents).
        // Gate it like /kg and /mm — governance is the inner layer; auth is applied
        // as the outer layer on protected_api_router (auth runs first, governance
        // second).
        .route_layer(governance_layer.clone());

    let protected_api_router = Router::new()
        .route("/currentUser", get(auth::get_current_user))
        .merge(user_routes)
        .nest("/v1", agent_routes)
        .nest("/v1/workflows", workflow_evidence_routes)
        .nest("/v1/memory", memory_routes)
        .nest(
            "/kg",
            Router::new()
                .route(
                    "/entities",
                    get(knowledge_graph::list_entities).post(knowledge_graph::create_entity),
                )
                .route(
                    "/entities/by-name/{name}",
                    get(knowledge_graph::get_entity_by_name),
                )
                .route(
                    "/entities/{entity_id}/related",
                    get(knowledge_graph::get_related_entities),
                )
                .route("/relations", post(knowledge_graph::create_relation))
                .route("/search", post(knowledge_graph::search_by_entity))
                .route_layer(governance_layer.clone()),
        )
        .nest(
            "/mm",
            Router::new()
                .route("/store", post(multimodal::store_mm))
                .route("/entry/{entry_id}", get(multimodal::get_mm))
                .route("/session/{session_id}", get(multimodal::get_session_mm))
                .route(
                    "/modality/{modality_type}",
                    get(multimodal::get_by_modality),
                )
                .route("/list", get(multimodal::list_mm))
                .route_layer(governance_layer.clone()),
        )
        .nest(
            "/tenants",
            Router::new()
                .route(
                    "/",
                    get(multi_tenant_router::list_tenants)
                        .post(multi_tenant_router::register_tenant),
                )
                .route(
                    "/{tenant_id}/search",
                    post(multi_tenant_router::tenant_search),
                )
                .route(
                    "/{tenant_id}/sessions",
                    get(multi_tenant_router::tenant_sessions),
                )
                .route("/access/check", post(multi_tenant_router::check_access))
                // Backlog C-1: tenant administration is privileged (ManageTenant /
                // DeleteTenant). Gate it like /kg and /mm.
                .route_layer(governance_layer.clone()),
        )
        .nest(
            "/v1/security",
            Router::new()
                .route("/prompt-probe/check", post(security::check_prompt_probe))
                .route(
                    "/prompt-probe/check-input",
                    post(security::check_prompt_probe_input),
                )
                .route(
                    "/prompt-probe/check-output",
                    post(security::check_prompt_probe_output),
                ),
        )
        // Workflow approval routes
        .nest(
            "/v1/workflows",
            Router::new()
                .route("/{workflow_id}/approve", post(workflows::approve_workflow))
                .route("/{workflow_id}/reject", post(workflows::reject_workflow)),
        )
        .nest(
            "/v1/approvals",
            Router::new().route("/{approval_id}/status", get(workflows::get_approval_status)),
        )
        // Distributed system routes (pool status, signals)
        .nest(
            "/v1/distributed",
            Router::new()
                .route("/pool/status", get(distributed::get_pool_status))
                .route("/pool/allocate", post(distributed::allocate_slots))
                .route("/pool/release", post(distributed::release_slots))
                .route("/signals/{workflow_id}", get(distributed::get_signals))
                .route("/signals/publish", post(distributed::publish_signal)),
        )
        // Planner sandbox routes (dry-run execution)
        .nest(
            "/v1/planner",
            planner::router(std::sync::Arc::new(planner::PlannerState::new())),
        )
        .route_layer(auth_layer);

    // Login endpoints get their own stricter rate limit to blunt credential
    // brute-forcing, independent of the memory-route limiter.
    let login_rate_limit = middleware::from_fn_with_state(
        hoops::rate_limit_state(10, 60, trusted_proxies.clone()),
        hoops::rate_limit_middleware,
    );
    let login_routes = Router::new()
        .route("/login", post(auth::post_login))
        .route(
            "/login/account",
            post(auth::post_login_with_token).get(auth::get_login_with_token),
        )
        .route_layer(login_rate_limit);

    // POST /register gets the same stricter rate limit as login to prevent
    // account-spam and user enumeration.
    let register_rate_limit = middleware::from_fn_with_state(
        hoops::rate_limit_state(10, 60, trusted_proxies),
        hoops::rate_limit_middleware,
    );
    let register_routes = Router::new()
        .route("/register", post(auth::register))
        .route_layer(register_rate_limit);

    let api_router = Router::new()
        .merge(login_routes)
        .merge(protected_api_router)
        .merge(mcp::router())
        // data import/export and time-travel tracing expose full memory contents
        // and workflow history — they must sit behind auth like the rest of /api.
        .merge(data_io::router().route_layer(middleware::from_fn(hoops::jwt::auth_middleware)))
        .nest(
            "/v1/tracing",
            tracing::router().route_layer(middleware::from_fn(hoops::jwt::auth_middleware)),
        );

    // A2A agent-interop protocol (opt-in: `--features a2a`; pulls the a2a-rs git deps).
    #[cfg(feature = "a2a")]
    let api_router = {
        let a2a_base_url = format!("http://{}", config::get().listen_addr.clone());
        let a2a_handler = Arc::new(crate::a2a::handler::A2AHandler::new());
        api_router.merge(crate::a2a::a2a_router(a2a_base_url, a2a_handler))
    };

    Router::new()
        .route("/", get(demo::hello))
        .route("/login", get(auth::login_page))
        .merge(register_routes)
        .route("/users", get(user::list_page))
        .nest("/api", api_router)
        .route("/api-doc/openapi.json", get(openapi_json))
        .route("/scalar", get(scalar_ui))
        .route("/scalar/", get(scalar_ui))
        .route("/favicon.ico", get(favicon))
        .route("/metrics", get(prometheus_exporter::metrics_handler))
        // Orchestrator probes: root path, unauthenticated (a kubelet cannot
        // present a JWT). /livez checks nothing external on purpose — see
        // routers/probes.rs for why probing dependencies there would turn a
        // dependency outage into a restart storm.
        .merge(probes::router())
        .nest_service("/assets", ServeDir::new("assets"))
        // Per-request metrics (memory_requests_total{endpoint,status} +
        // memory_request_duration_seconds). Applied at the outermost router so it
        // covers every route; the endpoint label is the resolved route template
        // (MatchedPath), which is available and fully-qualified at this layer —
        // verified in hoops::metrics_mw tests. Scrape/probe endpoints are skipped
        // inside the middleware.
        .layer(middleware::from_fn(hoops::track_request_metrics))
        .layer(TraceLayer::new_for_http())
        .fallback(not_found)
}

async fn favicon() -> impl IntoResponse {
    if let Some(file) = Assets::get("favicon.ico") {
        let mut response = Response::new(axum::body::Body::from(file.data.to_vec()));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("image/x-icon"),
        );
        response
    } else {
        (StatusCode::NOT_FOUND, "favicon not found").into_response()
    }
}

async fn openapi_json() -> impl IntoResponse {
    Json(serde_json::json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Aetheris MemOS MVP API",
            "version": "0.0.1",
            "description": "Stable MVP routes for agent-facing memory operations."
        },
        "paths": {
            "/api/v1/memory/health": { "get": { "summary": "Memory health check" } },
            "/api/v1/memory/adaptive/status": { "get": { "summary": "Get adaptive memory status" } },
            "/api/v1/memory/adaptive/select": { "post": {
                "summary": "Select memory configuration",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SelectMemoryRequest" } } } },
                "responses": { "200": { "description": "Memory configuration selected", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SelectMemoryResponse" } } } } }
            } },
            "/api/v1/memory/adaptive/trace": { "post": {
                "summary": "Select memory configuration with trace",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SelectMemoryRequest" } } } },
                "responses": { "200": { "description": "Decision trace", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/DecisionTrace" } } } } }
            } },
            "/api/v1/memory/traces": { "get": { "summary": "List decision traces" } },
            "/api/v1/memory/explain": { "get": { "summary": "Explain memory selection" } },
            "/api/v1/memory/feedback": { "post": {
                "summary": "Record retrieval feedback",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/MemoryFeedbackRequest" } } } },
                "responses": { "200": { "description": "Feedback recorded", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/MemoryFeedbackResponse" } } } } }
            } },
            "/api/v1/memory/forget": { "post": {
                "summary": "Forget a memory item",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/MemoryForgetRequest" } } } },
                "responses": { "200": { "description": "Forget result", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/MemoryForgetResponse" } } } } }
            } },
            "/api/v1/memory/storage/sessions": { "get": { "summary": "List STM sessions" } },
            "/api/v1/memory/storage/stm": { "post": {
                "summary": "Store STM message",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/StoreSTMRequest" } } } },
                "responses": { "200": { "description": "STM message stored", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/StoreSTMResponse" } } } } }
            } },
            "/api/v1/memory/storage/stm/{session_id}": { "get": { "summary": "Get STM session messages" } },
            "/api/v1/memory/storage/ltm": { "post": {
                "summary": "Store LTM entry",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/StoreLTMRequest" } } } },
                "responses": { "200": { "description": "LTM entry stored", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/StoreLTMResponse" } } } } }
            } },
            "/api/v1/memory/storage/transfer": { "post": {
                "summary": "Transfer STM to LTM",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TransferRequest" } } } },
                "responses": { "200": { "description": "Transfer result", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TransferResponse" } } } } }
            } },
            "/api/v1/memory/storage/batch-ltm": { "post": {
                "summary": "Batch store LTM entries",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/BatchStoreLTMRequest" } } } },
                "responses": { "200": { "description": "Batch LTM entries stored", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/BatchStoreLTMResponse" } } } } }
            } },
            "/api/v1/memory/storage/qdrant/backfill-tenant-metadata": { "post": {
                "summary": "Backfill Qdrant tenant metadata",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/BackfillQdrantTenantMetadataRequest" } } } },
                "responses": { "200": { "description": "Backfill report", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/QdrantTenantBackfillReport" } } } } }
            } },
            "/api/v1/memory/storage/compress/session": { "post": {
                "summary": "Compress messages from an STM session",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/CompressSessionRequest" } } } },
                "responses": { "200": { "description": "Compressed context", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/CompressionResult" } } } } }
            } },
            "/api/v1/memory/storage/compress/messages": { "post": {
                "summary": "Compress a provided message list",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/CompressMessagesRequest" } } } },
                "responses": { "200": { "description": "Compressed context", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/CompressionResult" } } } } }
            } },
            "/api/v1/memory/search/stm": { "post": {
                "summary": "Search STM",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SearchSTMRequest" } } } },
                "responses": { "200": { "description": "STM search results", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SearchSTMResponse" } } } } }
            } },
            "/api/v1/memory/search/ltm": {
                "get": { "summary": "List LTM entries" },
                "post": {
                    "summary": "Search LTM",
                    "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SearchLTMRequest" } } } },
                    "responses": { "200": { "description": "LTM search results", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SearchResponse" } } } } }
                }
            },
            "/api/v1/memory/search/ltm/{entry_id}": { "get": { "summary": "Get LTM entry" } },
            "/api/v1/memory/search/ltm/{entry_id}/at": { "get": {
                "summary": "Get an LTM entry as of a timestamp",
                "parameters": [
                    { "name": "entry_id", "in": "path", "required": true, "schema": { "type": "string" } },
                    { "name": "at", "in": "query", "required": true, "schema": { "type": "string", "format": "date-time" } },
                    { "name": "limit", "in": "query", "required": false, "schema": { "type": "integer" } }
                ],
                "responses": { "200": { "description": "LTM entry version", "content": { "application/json": { "schema": { "oneOf": [{ "$ref": "#/components/schemas/KnowledgeEntry" }, { "type": "null" }] } } } } }
            } },
            "/api/v1/memory/search/ltm/{entry_id}/history": { "get": {
                "summary": "List LTM entry history",
                "responses": { "200": { "description": "LTM entry versions", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/KnowledgeEntry" } } } } } }
            } },
            "/api/v1/memory/search/ltm/time-travel": { "post": {
                "summary": "Search LTM as of a timestamp",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TimeTravelQuery" } } } },
                "responses": { "200": { "description": "LTM entries at timestamp", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/KnowledgeEntry" } } } } } }
            } },
            "/api/v1/memory/search/kg/{entity_id}/at": { "get": {
                "summary": "Get a KG entity as of a timestamp",
                "parameters": [
                    { "name": "entity_id", "in": "path", "required": true, "schema": { "type": "string" } },
                    { "name": "at", "in": "query", "required": true, "schema": { "type": "string", "format": "date-time" } }
                ],
                "responses": { "200": { "description": "KG entity version", "content": { "application/json": { "schema": { "oneOf": [{ "$ref": "#/components/schemas/KGEntity" }, { "type": "null" }] } } } } }
            } },
            "/api/v1/memory/search/kg/{entity_id}/history": { "get": {
                "summary": "List KG entity history",
                "responses": { "200": { "description": "KG entity versions", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/KGEntity" } } } } } }
            } },
            "/api/v1/memory/search/hybrid": { "post": {
                "summary": "Hybrid memory search",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/HybridSearchRequest" } } } },
                "responses": { "200": { "description": "Hybrid search results", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SearchResponse" } } } } }
            } },
            "/api/v1/memory/search/triple": { "post": {
                "summary": "Triple hybrid memory search",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TripleHybridSearchRequest" } } } },
                "responses": { "200": { "description": "Triple hybrid search results", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SearchResponse" } } } } }
            } },
            "/api/v1/memory/search/scored": { "post": {
                "summary": "Confidence-scored memory search",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ScoredSearchRequest" } } } },
                "responses": { "200": { "description": "Scored search results", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SearchResponse" } } } } }
            } },
            "/api/v1/memory/search/entity": { "post": {
                "summary": "Search memory by entity",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SearchByEntityRequest" } } } },
                "responses": { "200": { "description": "Entity search results", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SearchResponse" } } } } }
            } },
            "/api/v1/memory/search/graphrag": { "post": { "summary": "GraphRAG hybrid search" } },
            "/api/v1/workflows/{id}/evidence": { "get": { "summary": "Get workflow evidence graph" } },
            "/api/kg/entities": {
                "get": { "summary": "List KG entities", "responses": { "200": { "description": "KG entities", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/EntityListResponse" } } } } } },
                "post": {
                    "summary": "Create KG entity",
                    "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/CreateEntityRequest" } } } },
                    "responses": { "200": { "description": "KG entity created", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/CreateEntityResponse" } } } } }
                }
            },
            "/api/kg/entities/by-name/{name}": { "get": {
                "summary": "Get KG entity by name",
                "responses": { "200": { "description": "KG entity", "content": { "application/json": { "schema": { "oneOf": [{ "$ref": "#/components/schemas/EntityInfo" }, { "type": "null" }] } } } } }
            } },
            "/api/kg/entities/{entity_id}/related": { "get": {
                "summary": "Get related KG entities",
                "responses": { "200": { "description": "KG relations", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/RelationInfo" } } } } } }
            } },
            "/api/kg/relations": { "post": {
                "summary": "Create KG relation",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/CreateRelationRequest" } } } },
                "responses": { "200": { "description": "KG relation created", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/CreateRelationResponse" } } } } }
            } },
            "/api/kg/search": { "post": {
                "summary": "Search KG entities",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SearchEntitiesRequest" } } } },
                "responses": { "200": { "description": "KG entity search results", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/EntityInfo" } } } } } }
            } },
            "/api/mm/list": { "get": { "summary": "List multimodal memories", "responses": { "200": { "description": "Multimodal memories", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/MMEntryListResponse" } } } } } } },
            "/api/mm/store": { "post": {
                "summary": "Store multimodal memory",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/StoreMMRequest" } } } },
                "responses": { "200": { "description": "Multimodal memory stored", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/StoreMMResponse" } } } } }
            } },
            "/api/mm/entry/{entry_id}": { "get": { "summary": "Get multimodal memory", "responses": { "200": { "description": "Multimodal memory", "content": { "application/json": { "schema": { "oneOf": [{ "$ref": "#/components/schemas/MMEntryInfo" }, { "type": "null" }] } } } } } } },
            "/api/mm/session/{session_id}": { "get": { "summary": "Get session multimodal memories", "responses": { "200": { "description": "Session multimodal memories", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/MMEntryInfo" } } } } } } } },
            "/api/mm/modality/{modality_type}": { "get": { "summary": "Get multimodal memories by modality", "responses": { "200": { "description": "Modality-filtered multimodal memories", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/MMEntryInfo" } } } } } } } },
            "/api/mcp/tools": { "get": { "summary": "List MCP memory tools" } },
            "/api/mcp/tools/call": { "post": {
                "summary": "Call MCP memory tool",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ToolCallParams" } } } },
                "responses": { "200": { "description": "MCP tool response", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ToolCallResponse" } } } } }
            } },
            "/api/mcp/resources": { "get": {
                "summary": "List MCP memory resources",
                "responses": { "200": { "description": "MCP resources", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ResourcesListResponse" } } } } }
            } },
            "/api/mcp/resources/read": { "post": {
                "summary": "Read MCP memory resource",
                "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ResourceReadParams" } } } },
                "responses": { "200": { "description": "MCP resource content", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ResourceReadResponse" } } } } }
            } }
        },
        "components": {
            "schemas": {
                "SelectMemoryRequest": {
                    "type": "object",
                    "required": ["task_context", "resource_constraints", "preferences"],
                    "properties": {
                        "task_context": { "$ref": "#/components/schemas/TaskContext" },
                        "resource_constraints": { "$ref": "#/components/schemas/ResourceConstraints" },
                        "preferences": { "$ref": "#/components/schemas/TaskPreferences" },
                        "explain": { "type": "boolean" },
                        "dry_run": { "type": "boolean" },
                        "persist_trace": { "type": "boolean" },
                        "what_if_constraints": { "$ref": "#/components/schemas/ResourceConstraints" }
                    }
                },
                "TaskContext": {
                    "type": "object",
                    "required": ["task_id", "task_type", "complexity", "modality_requirements", "temporal_scope", "reasoning_depth", "context_dependency", "user_id", "agent_id"],
                    "properties": {
                        "task_id": { "type": "string" },
                        "task_type": { "type": "string", "enum": ["conversation", "task", "query"] },
                        "complexity": { "type": "number" },
                        "modality_requirements": { "type": "array", "items": { "type": "string", "enum": ["text", "image", "audio", "video"] } },
                        "temporal_scope": { "type": "string", "enum": ["short", "medium", "long"] },
                        "reasoning_depth": { "type": "string", "enum": ["shallow", "medium", "deep"] },
                        "context_dependency": { "type": "number" },
                        "user_id": { "type": "string" },
                        "agent_id": { "type": "string" }
                    }
                },
                "ResourceConstraints": {
                    "type": "object",
                    "required": ["max_memory_usage_mb", "max_cpu_usage_percent", "max_response_time_ms", "storage_quota_percent"],
                    "properties": {
                        "max_memory_usage_mb": { "type": "integer", "format": "uint64" },
                        "max_cpu_usage_percent": { "type": "integer", "format": "uint8" },
                        "max_response_time_ms": { "type": "integer", "format": "uint64" },
                        "storage_quota_percent": { "type": "integer", "format": "uint8" }
                    }
                },
                "TaskPreferences": {
                    "type": "object",
                    "required": ["prioritize_efficiency", "prioritize_coherence", "enable_multimodal", "enable_reasoning"],
                    "properties": {
                        "prioritize_efficiency": { "type": "boolean" },
                        "prioritize_coherence": { "type": "boolean" },
                        "enable_multimodal": { "type": "boolean" },
                        "enable_reasoning": { "type": "boolean" }
                    }
                },
                "SelectMemoryResponse": { "type": "object", "properties": { "memory_config": { "type": "object" }, "performance_prediction": { "type": "object" }, "resource_requirements": { "type": "object" }, "trace": { "$ref": "#/components/schemas/DecisionTrace" } } },
                "DecisionTrace": { "type": "object", "additionalProperties": true },
                "MemoryFeedbackRequest": { "type": "object", "required": ["memoryId", "feedback"], "properties": { "memoryId": { "type": "string" }, "feedback": { "type": "string", "enum": ["useful", "not_useful"] }, "query": { "type": "string" }, "metadata": { "type": "object", "additionalProperties": true } } },
                "MemoryFeedbackResponse": { "type": "object", "required": ["recorded", "memoryId"], "properties": { "recorded": { "type": "boolean" }, "memoryId": { "type": "string" } } },
                "MemoryForgetRequest": { "type": "object", "required": ["memoryId", "layer"], "properties": { "memoryId": { "type": "string" }, "layer": { "type": "string", "enum": ["stm", "ltm", "kg", "mm"] } } },
                "MemoryForgetResponse": { "type": "object", "required": ["forgotten", "memoryId"], "properties": { "forgotten": { "type": "boolean" }, "memoryId": { "type": "string" }, "layer": { "type": "string" } } },
                "StoreSTMRequest": { "type": "object", "required": ["userId", "agentId", "sessionType", "role", "content"], "properties": { "userId": { "type": "string" }, "agentId": { "type": "string" }, "sessionType": { "type": "string" }, "role": { "type": "string" }, "content": { "type": "string" }, "maxContextLength": { "type": "integer" }, "retentionHours": { "type": "integer" } } },
                "StoreSTMResponse": { "type": "object", "required": ["sessionId", "messageId"], "properties": { "sessionId": { "type": "string" }, "messageId": { "type": "string" } } },
                "StoreLTMRequest": { "type": "object", "required": ["sourceId", "sourceType", "content"], "properties": { "sourceId": { "type": "string" }, "sourceType": { "type": "string" }, "content": { "type": "string" }, "title": { "type": "string" } } },
                "StoreLTMResponse": { "type": "object", "required": ["entryId", "indexStatus"], "properties": { "entryId": { "type": "string" }, "indexStatus": { "type": "string", "description": "pending when vector outbox not yet applied; ready when indexed synchronously" } } },
                "BatchStoreLTMRequest": { "type": "object", "required": ["entries"], "properties": { "entries": { "type": "array", "items": { "$ref": "#/components/schemas/BatchStoreEntry" } } } },
                "BatchStoreEntry": { "type": "object", "required": ["sourceId", "sourceType", "content"], "properties": { "tenantId": { "type": "string" }, "sourceId": { "type": "string" }, "sourceType": { "type": "string" }, "content": { "type": "string" }, "title": { "type": "string" } } },
                "BatchStoreLTMResponse": { "type": "object", "required": ["entryIds"], "properties": { "entryIds": { "type": "array", "items": { "type": "string" } } } },
                "TransferRequest": { "type": "object", "required": ["sessionId"], "properties": { "sessionId": { "type": "string" }, "messageCountThreshold": { "type": "integer" } } },
                "TransferResponse": { "type": "object", "required": ["entryIds"], "properties": { "entryIds": { "type": "array", "items": { "type": "string" } } } },
                "BackfillQdrantTenantMetadataRequest": { "type": "object", "properties": { "limit": { "type": "integer" }, "offset": { "type": "integer" }, "dryRun": { "type": "boolean", "default": true } } },
                "QdrantTenantBackfillReport": { "type": "object", "required": ["dryRun", "scanned", "planned", "updated", "skippedWithoutTenant"], "properties": { "dryRun": { "type": "boolean" }, "scanned": { "type": "integer" }, "planned": { "type": "integer" }, "updated": { "type": "integer" }, "skippedWithoutTenant": { "type": "integer" } } },
                "CompressionStrategy": { "type": "string", "enum": ["sliding_window", "llm_summary", "importance_prune", "hierarchical"] },
                "MessageEntry": { "type": "object", "required": ["role", "content"], "properties": { "role": { "type": "string" }, "content": { "type": "string" }, "timestamp": { "type": "string" }, "importance": { "type": "number" }, "metadata": { "type": "object", "additionalProperties": true } } },
                "CompressSessionRequest": { "type": "object", "required": ["session_id"], "properties": { "session_id": { "type": "string" }, "strategy": { "$ref": "#/components/schemas/CompressionStrategy" }, "token_budget": { "type": "integer" }, "window_size": { "type": "integer" }, "hierarchical_recent_k": { "type": "integer" } } },
                "CompressMessagesRequest": { "type": "object", "required": ["messages"], "properties": { "messages": { "type": "array", "items": { "$ref": "#/components/schemas/MessageEntry" } }, "strategy": { "$ref": "#/components/schemas/CompressionStrategy" }, "token_budget": { "type": "integer" }, "window_size": { "type": "integer" }, "hierarchical_recent_k": { "type": "integer" } } },
                "CompressionResult": { "type": "object", "required": ["messages", "original_token_count", "compressed_token_count", "compression_ratio", "strategy_used"], "properties": { "messages": { "type": "array", "items": { "$ref": "#/components/schemas/MessageEntry" } }, "summary": { "type": "string" }, "original_token_count": { "type": "integer" }, "compressed_token_count": { "type": "integer" }, "compression_ratio": { "type": "number" }, "strategy_used": { "$ref": "#/components/schemas/CompressionStrategy" } } },
                "SearchSTMRequest": { "type": "object", "required": ["userId", "agentId"], "properties": { "userId": { "type": "string" }, "agentId": { "type": "string" }, "sessionType": { "type": "string" }, "limit": { "type": "integer" } } },
                "SearchSTMResponse": { "type": "object", "required": ["messages"], "properties": { "messages": { "type": "array", "items": { "$ref": "#/components/schemas/SessionMessage" } } } },
                "SessionMessage": { "type": "object", "additionalProperties": true },
                "KnowledgeEntry": { "type": "object", "additionalProperties": true },
                "KGEntity": { "type": "object", "additionalProperties": true },
                "TimeTravelQuery": { "type": "object", "required": ["at"], "properties": { "at": { "type": "string", "format": "date-time" }, "limit": { "type": "integer" } } },
                "SearchLTMRequest": { "type": "object", "required": ["query"], "properties": { "query": { "type": "string" }, "topK": { "type": "integer" }, "enableRerank": { "type": "boolean" }, "minScore": { "type": "number" } } },
                "HybridSearchRequest": { "allOf": [{ "$ref": "#/components/schemas/SearchLTMRequest" }], "properties": { "keywordWeight": { "type": "number" }, "vectorWeight": { "type": "number" } } },
                "TripleHybridSearchRequest": { "allOf": [{ "$ref": "#/components/schemas/HybridSearchRequest" }], "properties": { "graphWeight": { "type": "number" } } },
                "ConfidenceConfigInput": { "type": "object", "properties": { "quality_weight": { "type": "number" }, "relevance_weight": { "type": "number" }, "recency_weight": { "type": "number" }, "access_weight": { "type": "number" }, "completeness_weight": { "type": "number" }, "recency_half_life_days": { "type": "number" } } },
                "ScoredSearchRequest": { "allOf": [{ "$ref": "#/components/schemas/TripleHybridSearchRequest" }], "properties": { "confidence_config": { "$ref": "#/components/schemas/ConfidenceConfigInput" } } },
                "SearchByEntityRequest": { "type": "object", "required": ["entity"], "properties": { "entity": { "type": "string" }, "limit": { "type": "integer" } } },
                "SearchResponse": { "type": "object", "required": ["results"], "properties": { "results": { "type": "array", "items": { "$ref": "#/components/schemas/SearchResult" } } } },
                "SearchResult": { "type": "object", "required": ["memoryId", "sourceLayer", "score", "content", "metadata"], "properties": { "memoryId": { "type": "string" }, "entry_id": { "type": "string" }, "sourceLayer": { "type": "string" }, "score": { "type": "number" }, "content": { "type": "string" }, "title": { "type": "string" }, "traceId": { "type": "string" }, "explanation": { "type": "string" }, "metadata": { "type": "object", "additionalProperties": true } } },
                "CreateEntityRequest": { "type": "object", "required": ["entityName", "entityType"], "properties": { "entityName": { "type": "string" }, "entityType": { "type": "string" }, "description": { "type": "string" }, "aliases": { "type": "array", "items": { "type": "string" } } } },
                "CreateEntityResponse": { "type": "object", "required": ["entityId"], "properties": { "entityId": { "type": "string" } } },
                "CreateRelationRequest": { "type": "object", "required": ["sourceEntityId", "targetEntityId", "relationType"], "properties": { "sourceEntityId": { "type": "string" }, "targetEntityId": { "type": "string" }, "relationType": { "type": "string" }, "weight": { "type": "number" }, "confidence": { "type": "number" } } },
                "CreateRelationResponse": { "type": "object", "required": ["relationId"], "properties": { "relationId": { "type": "string" } } },
                "SearchEntitiesRequest": { "type": "object", "required": ["query"], "properties": { "query": { "type": "string" }, "limit": { "type": "integer", "default": 10 } } },
                "EntityInfo": { "type": "object", "required": ["entityId", "entityName", "entityType"], "properties": { "entityId": { "type": "string" }, "entityName": { "type": "string" }, "entityType": { "type": "string" }, "description": { "type": "string" } } },
                "RelationInfo": { "type": "object", "required": ["relationId", "sourceEntityId", "targetEntityId", "relationType", "weight", "confidence"], "properties": { "relationId": { "type": "string" }, "sourceEntityId": { "type": "string" }, "targetEntityId": { "type": "string" }, "relationType": { "type": "string" }, "weight": { "type": "number" }, "confidence": { "type": "number" } } },
                "EntityListResponse": { "type": "object", "required": ["entities", "total", "limit", "offset"], "properties": { "entities": { "type": "array", "items": { "$ref": "#/components/schemas/EntityInfo" } }, "total": { "type": "integer" }, "limit": { "type": "integer" }, "offset": { "type": "integer" } } },
                "StoreMMRequest": { "type": "object", "required": ["sourceId", "modalityType"], "properties": { "sessionId": { "type": "string" }, "sourceId": { "type": "string" }, "tenantId": { "type": "string" }, "modalityType": { "type": "string", "enum": ["text", "image", "audio", "video"] }, "title": { "type": "string" }, "description": { "type": "string" }, "content": { "type": "string", "description": "Base64 encoded binary content" }, "textContent": { "type": "string" }, "imageUrl": { "type": "string" }, "audioUrl": { "type": "string" } } },
                "StoreMMResponse": { "type": "object", "required": ["entryId"], "properties": { "entryId": { "type": "string" } } },
                "MMEntryInfo": { "type": "object", "required": ["entryId", "sourceId", "modalityType"], "properties": { "entryId": { "type": "string" }, "sessionId": { "type": "string" }, "sourceId": { "type": "string" }, "modalityType": { "type": "string" }, "title": { "type": "string" }, "description": { "type": "string" } } },
                "MMEntryListResponse": { "type": "object", "required": ["entries", "total", "limit", "offset"], "properties": { "entries": { "type": "array", "items": { "$ref": "#/components/schemas/MMEntryInfo" } }, "total": { "type": "integer" }, "limit": { "type": "integer" }, "offset": { "type": "integer" } } },
                "ToolCallParams": { "type": "object", "required": ["name"], "properties": { "name": { "type": "string" }, "arguments": { "type": "object", "additionalProperties": true } } },
                "ToolCallResponse": { "type": "object", "required": ["content"], "properties": { "content": { "type": "array", "items": { "type": "object", "additionalProperties": true } }, "is_error": { "type": "boolean" } } },
                "McpResource": { "type": "object", "required": ["uri", "name"], "properties": { "uri": { "type": "string" }, "name": { "type": "string" }, "description": { "type": "string" }, "mimeType": { "type": "string" } } },
                "ResourcesListResponse": { "type": "object", "required": ["resources"], "properties": { "resources": { "type": "array", "items": { "$ref": "#/components/schemas/McpResource" } } } },
                "ResourceReadParams": { "type": "object", "required": ["uri"], "properties": { "uri": { "type": "string" } } },
                "ResourceReadResponse": { "type": "object", "required": ["contents"], "properties": { "contents": { "type": "array", "items": { "type": "object", "additionalProperties": true } } } }
            }
        }
    }))
}

async fn scalar_ui() -> Html<String> {
    Html(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width,initial-scale=1" />
    <title>API Scalar</title>
  </head>
  <body style="margin:0">
    <script id="api-reference" data-url="/api-doc/openapi.json"></script>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
  </body>
</html>"#
            .to_string(),
    )
}

async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Html("Page not found".to_string()))
}
