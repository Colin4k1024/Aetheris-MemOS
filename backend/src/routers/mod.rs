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
    let rate_limit_state = hoops::rate_limit_state(100, 60);
    let memory_rate_limit =
        middleware::from_fn_with_state(rate_limit_state, hoops::rate_limit_middleware);

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
        // Enterprise routes
        .nest(
            "/enterprise",
            Router::new()
                // Cluster management
                .route("/cluster/node", post(enterprise::register_node))
                .route("/cluster/nodes", get(enterprise::get_cluster_nodes))
                .route("/cluster/active", get(enterprise::get_active_nodes))
                .route("/cluster/leader", get(enterprise::get_leader))
                .route("/cluster/become-leader", post(enterprise::become_leader))
                .route("/cluster/is-leader", get(enterprise::is_leader))
                // Sharding
                .route("/shards", post(enterprise::create_shard))
                .route("/shards", get(enterprise::get_shards))
                .route("/shards/{key}", get(enterprise::get_shard)),
        )
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
        );

    let protected_api_router = Router::new()
        .route("/currentUser", get(auth::get_current_user))
        .merge(user_routes)
        .nest("/v1", agent_routes)
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
                .route("/search", post(knowledge_graph::search_by_entity)),
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
                .route("/list", get(multimodal::list_mm)),
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
                .route("/access/check", post(multi_tenant_router::check_access)),
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

    let api_router = Router::new()
        .route("/login", post(auth::post_login))
        .route(
            "/login/account",
            post(auth::post_login_with_token).get(auth::get_login_with_token),
        )
        .merge(protected_api_router)
        .merge(mcp::router())
        .merge(data_io::router())
        .nest("/v1/tracing", tracing::router());

    Router::new()
        .route("/", get(demo::hello))
        .route("/login", get(auth::login_page))
        .route("/register", post(auth::register))
        .route("/users", get(user::list_page))
        .nest("/api", api_router)
        .route("/api-doc/openapi.json", get(openapi_json))
        .route("/scalar", get(scalar_ui))
        .route("/scalar/", get(scalar_ui))
        .route("/favicon.ico", get(favicon))
        .route("/metrics", get(prometheus_exporter::metrics_handler))
        .nest_service("/assets", ServeDir::new("assets"))
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
        "info": { "title": "adaptive-memory-system api", "version": "0.0.1" },
        "paths": {}
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
