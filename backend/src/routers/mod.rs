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
/// Metering / billing surface. Enterprise-gated — see [`billing_router`] for why
/// the gate covers this and nothing else.
#[cfg(feature = "enterprise")]
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
mod openapi;
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

/// Metering / billing routes — the one genuinely commercial surface in this
/// repository, and therefore the only thing `feature = "enterprise"` gates
/// (backlog A-3).
///
/// ## Why the gate stops here
///
/// `Cargo.toml` used to describe this feature as covering "billing, cluster
/// management, RBAC, governance hooks". Implementing that literally would compile
/// RBAC and the governance middleware **out of the default build** — and those are
/// what the authorization path runs on across all three protocol planes:
/// `hoops/governance.rs` (REST, C-1), `routers/mcp.rs` (MCP capability
/// derivation, A-1) and `a2a/handler.rs` (in-handler governance, A-4c). The MIT
/// build would ship with no authorization anywhere, which is a security
/// regression wearing a packaging fix's clothes.
///
/// C-1 explicitly required that the strong-typed authorization path not depend on
/// enterprise wiring, precisely so SQLite/dev deployments stay guarded. Security
/// is not the monetised surface; metering is. The `Cargo.toml` comment has been
/// corrected to match.
///
/// The quota half of governance needs no compile gate either: it runs through
/// `try_enterprise_hooks()`, which returns `Option` and is already inert whenever
/// the hook set was not initialised.
///
/// ## How drift is caught
///
/// In this direction, by the **compiler**: any ungated reference to `billing::`
/// fails the default build, which is why CI compiles both channels
/// (`cargo check` with default features and with `--features enterprise`).
/// The opposite direction — someone *adding* a cfg to the authorization surface —
/// compiles cleanly and is caught by
/// `enterprise_gate::authorization_surface_is_not_enterprise_gated`.
#[cfg(feature = "enterprise")]
fn billing_router() -> Router {
    Router::new()
        .route("/init", post(billing::init_tenant))
        .route("/usage", post(billing::get_usage))
        .route("/usage/{tenant_id}", get(billing::get_current_usage))
        .route("/quota/{tenant_id}", get(billing::get_quota_status))
        .route("/record", post(billing::record_usage))
}

/// Default (MIT) build: no billing surface.
///
/// An empty nested router has no fallback of its own, so it delegates to the
/// outer one — `/billing/*` answers the same 404 it would if the prefix had never
/// been mounted. Returning an empty `Router` rather than `#[cfg]`-ing the `.nest`
/// call keeps the 170-line builder chain in `root()` intact; the alternative is
/// splitting that chain around one conditional step.
#[cfg(not(feature = "enterprise"))]
fn billing_router() -> Router {
    Router::new()
}

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
                .route(
                    "/backfill-summaries",
                    post(memory_storage::backfill_pending_summaries),
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
        // Billing routes — enterprise-gated, see `billing_router`.
        .nest("/billing", billing_router())
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

    let ws_state = crate::protocol::websocket::WsHandlerState::default();

    let protected_api_router = Router::new()
        .route("/currentUser", get(auth::get_current_user))
        .route("/auth/switch-org", post(auth::switch_org))
        .route(
            "/ws",
            get(crate::protocol::websocket::ws_upgrade_handler).with_state(ws_state),
        )
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
                // Role management (C-3): preserved from routers/tenant.rs, now mounted.
                .route(
                    "/{tenant_id}/roles",
                    get(tenant::list_roles).post(tenant::assign_role),
                )
                .route("/{tenant_id}/roles/{user_id}", get(tenant::get_user_role))
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
        .route("/api-doc/openapi.json", get(openapi::openapi_json))
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

/// Query-parameter naming contract (D-i).
///
/// # Why this exists
///
/// `Query<T>` deserializes the query string into `T`. An `Option<_>` field whose
/// wire name does not match what the caller sent stays `None` — the extractor
/// succeeds, the handler runs, and the parameter is **silently dropped**: no
/// error to the caller, no log line. That shape already produced four separate
/// bugs here (`docs/memory/backlog.md`, D-i), because the wire name is decided
/// by a `#[serde(rename)]` attribute sitting far from the call site.
///
/// The naming is *not* uniform and deliberately stays that way. Request
/// **bodies** are camelCase (hundreds of renames); most **query** structs are
/// snake_case; three are camelCase (`ExplainQuery`, `ListMemoryConfigsRequest`,
/// and the `tenantId` fields in `multimodal.rs`). Each of those three already
/// has a live consumer — the frontend typings, the Python SDK (whose own
/// contract test pins `traceId`), and the Rust SDK. Renaming them to make the
/// codebase look tidy would be a breaking change for external integrators, so
/// this module pins the wire contract **as it is** and lets
/// `deny_unknown_fields` reject everything else. Fixing the silent drop matters;
/// cosmetic uniformity does not.
///
/// These tests live in-crate because the router submodules are private — making
/// them `pub` just to reach the structs from `tests/` would widen the public API
/// surface for a test's benefit.
#[cfg(test)]
mod query_param_contract {
    use axum::extract::Query;
    use axum::http::Uri;

    /// Deserialize through the **real** extractor. `Query::try_from_uri` is the
    /// same entry point axum uses when a handler binds `Query<T>`, so a passing
    /// assertion here means a live request with that spelling is accepted.
    fn parse<T>(query_string: &str) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned,
    {
        let uri: Uri = format!("http://test/x?{query_string}")
            .parse()
            .expect("uri");
        Query::<T>::try_from_uri(&uri)
            .map(|Query(value)| value)
            .map_err(|rejection| rejection.body_text())
    }

    // --- snake_case query structs: the majority convention -----------------

    #[test]
    fn list_sessions_query_accepts_snake_case() {
        use super::memory_storage::ListSessionsQuery;

        // Consumers: `sdks/rust/src/client.rs::list_sessions_request` and the
        // frontend `services/memory/storageApi.ts` both send these spellings.
        let q: ListSessionsQuery = parse("user_id=u1&status=active&limit=10&offset=5")
            .expect("snake_case is this endpoint's contract");
        assert_eq!(q.user_id.as_deref(), Some("u1"));
        assert_eq!(q.status.as_deref(), Some("active"));
        assert_eq!(q.limit, Some(10));
        assert_eq!(q.offset, Some(5));
    }

    #[test]
    fn list_sessions_query_rejects_camel_case_rather_than_dropping_it() {
        use super::memory_storage::ListSessionsQuery;

        // `userId` is not this endpoint's contract. Before `deny_unknown_fields`
        // this parsed happily with `user_id == None`, so the filter vanished and
        // every session came back — a wrong answer, not an error.
        let err = parse::<ListSessionsQuery>("userId=u1")
            .expect_err("unknown parameter must be rejected, not ignored");
        assert!(
            err.contains("userId"),
            "rejection should name the offending parameter, got: {err}"
        );
    }

    #[test]
    fn list_entities_query_accepts_snake_case() {
        use super::knowledge_graph::ListEntitiesQuery;

        let q: ListEntitiesQuery =
            parse("entity_type=person&limit=20&offset=0").expect("snake_case contract");
        assert_eq!(q.entity_type.as_deref(), Some("person"));
        assert_eq!(q.limit, Some(20));
        assert_eq!(q.offset, Some(0));
    }

    #[test]
    fn list_traces_query_accepts_snake_case() {
        use super::memory::ListTracesQuery;

        let q: ListTracesQuery = parse("task_id=t1&limit=5").expect("snake_case contract");
        assert_eq!(q.task_id.as_deref(), Some("t1"));
        assert_eq!(q.limit, Some(5));
    }

    // --- camelCase query structs: each pinned to a known consumer ----------

    #[test]
    fn explain_query_accepts_camel_case() {
        use super::memory::ExplainQuery;

        // Consumer: Python SDK `client.explain()` sends `traceId` / `taskId`,
        // pinned there by `test_explain_uses_rest_contract`.
        let q: ExplainQuery = parse("traceId=tr1&taskId=tk1&limit=3").expect("camelCase contract");
        assert_eq!(q.trace_id.as_deref(), Some("tr1"));
        assert_eq!(q.task_id.as_deref(), Some("tk1"));
        assert_eq!(q.limit, Some(3));
    }

    #[test]
    fn explain_query_rejects_snake_case_rather_than_dropping_it() {
        use super::memory::ExplainQuery;

        // Mirror image of the ListSessions case: here *snake_case* is wrong.
        // `trace_id=tr1` used to mean "explain the newest trace" instead of
        // "explain trace tr1".
        let err = parse::<ExplainQuery>("trace_id=tr1")
            .expect_err("unknown parameter must be rejected, not ignored");
        assert!(
            err.contains("trace_id"),
            "rejection should name the offending parameter, got: {err}"
        );
    }

    #[test]
    fn list_memory_configs_query_accepts_camel_case() {
        use super::memory::ListMemoryConfigsRequest;

        // Consumer: `pages/MemoryManagement/index.tsx`.
        let q: ListMemoryConfigsRequest =
            parse("page=2&pageSize=20&userId=u1&agentId=a1&status=active&configType=default")
                .expect("camelCase contract");
        assert_eq!(q.page, Some(2));
        assert_eq!(q.page_size, Some(20));
        assert_eq!(q.user_id.as_deref(), Some("u1"));
        assert_eq!(q.agent_id.as_deref(), Some("a1"));
        assert_eq!(q.status.as_deref(), Some("active"));
        assert_eq!(q.config_type.as_deref(), Some("default"));
    }

    #[test]
    fn multimodal_queries_mix_snake_case_fields_with_camel_case_tenant_id() {
        use super::multimodal::{LimitQuery, ListMMQuery};

        // The starkest case: `modality_type` and `tenantId` sit side by side in
        // one struct with different conventions. Both are load-bearing.
        let q: ListMMQuery = parse("modality_type=image&limit=10&offset=0&tenantId=t1")
            .expect("mixed-case contract");
        assert_eq!(q.modality_type.as_deref(), Some("image"));
        assert_eq!(q.limit, Some(10));
        assert_eq!(q.offset, Some(0));
        assert_eq!(q.tenant_id.as_deref(), Some("t1"));

        let q: LimitQuery = parse("limit=7&tenantId=t1").expect("mixed-case contract");
        assert_eq!(q.limit, Some(7));
        assert_eq!(q.tenant_id.as_deref(), Some("t1"));
    }

    // --- omitted parameters must stay omissible ---------------------------

    /// `deny_unknown_fields` must not turn optional parameters into required
    /// ones. Several of these endpoints are called with no query string at all,
    /// so an empty query must still deserialize.
    #[test]
    fn empty_query_string_still_parses() {
        use super::knowledge_graph::{ListEntitiesQuery, RelatedEntitiesQuery};
        use super::memory::{ExplainQuery, ListMemoryConfigsRequest, ListTracesQuery};
        use super::memory_storage::{GetSessionMessagesQuery, ListSessionsQuery};
        use super::multimodal::{LimitQuery, ListMMQuery};

        parse::<ListSessionsQuery>("").expect("ListSessionsQuery");
        parse::<GetSessionMessagesQuery>("").expect("GetSessionMessagesQuery");
        parse::<ListEntitiesQuery>("").expect("ListEntitiesQuery");
        parse::<RelatedEntitiesQuery>("").expect("RelatedEntitiesQuery");
        parse::<ListTracesQuery>("").expect("ListTracesQuery");
        parse::<ExplainQuery>("").expect("ExplainQuery");
        parse::<ListMemoryConfigsRequest>("").expect("ListMemoryConfigsRequest");
        parse::<LimitQuery>("").expect("LimitQuery");
        parse::<ListMMQuery>("").expect("ListMMQuery");
    }

    // --- structural anti-drift guards -------------------------------------

    /// Every query struct reached by a `Query<..>` extractor must carry
    /// `deny_unknown_fields`, or opt out with an `ALLOWS_UNKNOWN_QUERY_PARAMS`
    /// comment stating why.
    ///
    /// Keyed on the *shape* of the code rather than a hand-maintained list, so a
    /// query struct added tomorrow is covered without editing this test. Without
    /// the attribute a misspelled parameter is silently dropped, which is the
    /// root cause D-i exists to close.
    ///
    /// The escape hatch is deliberate but narrow: it requires a comment next to
    /// the struct, so an exemption is a documented decision rather than an
    /// omission. `TokenQuery` (routers/auth.rs) is the only current user — its
    /// URL travels through email clients that append tracking parameters.
    #[test]
    fn every_query_struct_denies_unknown_fields() {
        let mut checked = 0usize;
        let mut exempt = 0usize;
        for s in query_structs() {
            checked += 1;
            // The exemption is read from the comments, the attribute from the
            // attribute lines. Keeping the two sources separate is what makes
            // this guard capable of failing — see `query_structs`.
            if s.comments.contains("ALLOWS_UNKNOWN_QUERY_PARAMS") {
                exempt += 1;
                continue;
            }
            assert!(
                s.attrs.contains("deny_unknown_fields"),
                "{}: query struct `{}` lacks `#[serde(deny_unknown_fields)]`. Without it a caller \
                 who misspells an optional parameter gets a silent `None` — the filter disappears \
                 and the endpoint returns a confidently wrong result instead of a 400 (see \
                 docs/memory/backlog.md, D-i). Add the attribute and pin the accepted spellings \
                 with a test above. If unknown parameters genuinely must be tolerated (e.g. a URL \
                 that travels through link-rewriting clients), document that with an \
                 `ALLOWS_UNKNOWN_QUERY_PARAMS:` comment on the struct explaining why the \
                 silent-drop hazard does not apply.",
                s.file,
                s.name
            );
        }
        assert!(
            checked >= 16,
            "expected to find the known query structs (>=16), found {checked}; did the scanner or \
             the `pub struct` shape change?"
        );
        // Pins the exemption count: a further opt-out has to be a conscious edit
        // here, not something that rides along unnoticed. Current exemptions are
        // `TokenQuery` (email-borne URL) and `WorkflowEvidenceQuery` (no fields,
        // so nothing can be dropped).
        assert_eq!(
            exempt, 2,
            "expected exactly 2 documented exemptions (TokenQuery, WorkflowEvidenceQuery), found \
             {exempt}"
        );
    }

    /// Any explicit `#[serde(rename = "...")]` on a query field must rename to
    /// the camelCase form of that field.
    ///
    /// This permits both conventions — a field with no attribute stays
    /// snake_case, which is the majority — while forbidding what actually broke
    /// things: a rename inventing a *third* spelling (`trace-id`, `TraceID`)
    /// that matches neither convention and therefore matches no caller. With
    /// `deny_unknown_fields` now in place, such a field would reject every
    /// realistic request instead of quietly ignoring it.
    #[test]
    fn renamed_query_fields_use_camel_case() {
        let mut checked = 0usize;
        for (file, name, body) in query_struct_bodies() {
            for (field, wire_name) in renamed_fields(&body) {
                checked += 1;
                let expected = to_camel_case(&field);
                assert_eq!(
                    wire_name, expected,
                    "{file}: `{name}.{field}` renames to `{wire_name}`, which is neither the \
                     snake_case field name nor its camelCase form (`{expected}`). A wire name \
                     matching no convention matches no caller. Use `{expected}`, or drop the \
                     attribute to accept `{field}`."
                );
            }
        }
        // ExplainQuery's 2 + ListMemoryConfigsRequest's 4 + multimodal's 2.
        assert!(
            checked >= 8,
            "expected to check the known renamed query fields (>=8), checked {checked}; did the \
             attribute shape change?"
        );
    }

    // --- source scanning helpers ------------------------------------------
    //
    // Text-level scanning (not a real parser) mirrors the tenant guards in
    // `src/tenant/context.rs`: dependency-free, and robust to these structs
    // moving between router files. `CARGO_MANIFEST_DIR` keeps it independent of
    // the test's working directory.

    /// A query struct is one consumed by a `Query<..>` extractor. Detected by
    /// the `*Query` name suffix plus the single historical outlier
    /// (`ListMemoryConfigsRequest`, bound as `Query<..>` despite its name).
    fn is_query_struct(name: &str) -> bool {
        name.ends_with("Query") || name == "ListMemoryConfigsRequest"
    }

    /// Yields `(file, struct_name, attributes, comments)` for every query struct
    /// under `src/routers`.
    ///
    /// Attributes and comments are returned **separately and deliberately**. The
    /// two signals the guard reads live in different places — the
    /// `deny_unknown_fields` attribute is code, the `ALLOWS_UNKNOWN_QUERY_PARAMS`
    /// exemption is a comment — and conflating them makes the guard unable to
    /// fail: several of these structs have doc comments *explaining* the
    /// attribute, so prose containing the words `deny_unknown_fields` would
    /// satisfy an attribute check by itself. Deleting the real attribute would
    /// then keep the test green. That is precisely the "gate exists but does not
    /// bite" defect this remediation batch is about; the control-group run caught
    /// it here.
    fn query_structs() -> Vec<QueryStruct> {
        let mut out = Vec::new();
        for (file, src) in router_sources() {
            let chunks: Vec<&str> = src.split("pub struct ").collect();
            for (idx, chunk) in chunks.iter().enumerate().skip(1) {
                let name = leading_ident(chunk);
                if !is_query_struct(&name) {
                    continue;
                }
                // Attributes and comments sit in the tail of the *previous*
                // chunk, since splitting on "pub struct " cuts right after them.
                let window = chunks[idx - 1].rsplit("\n\n").next().unwrap_or("");
                let (comment_lines, attr_lines): (Vec<&str>, Vec<&str>) = window
                    .lines()
                    .partition(|line| line.trim_start().starts_with("//"));
                out.push(QueryStruct {
                    file: file.clone(),
                    name,
                    attrs: attr_lines.join("\n"),
                    comments: comment_lines.join("\n"),
                });
            }
        }
        out
    }

    struct QueryStruct {
        file: String,
        name: String,
        /// Non-comment lines preceding the struct, i.e. the real `#[...]`
        /// attributes.
        attrs: String,
        /// Comment lines preceding the struct, where an exemption is declared.
        comments: String,
    }

    /// Yields `(file, struct_name, struct_body)` for every query struct.
    fn query_struct_bodies() -> Vec<(String, String, String)> {
        let mut out = Vec::new();
        for (file, src) in router_sources() {
            for chunk in src.split("pub struct ").skip(1) {
                let name = leading_ident(chunk);
                if !is_query_struct(&name) {
                    continue;
                }
                let Some(open) = chunk.find('{') else {
                    continue;
                };
                let Some(close) = chunk[open..].find('}') else {
                    continue;
                };
                out.push((file.clone(), name, chunk[open..open + close].to_string()));
            }
        }
        out
    }

    fn router_sources() -> Vec<(String, String)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/routers");
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
        let mut out = Vec::new();
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            out.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                src,
            ));
        }
        out
    }

    fn leading_ident(s: &str) -> String {
        s.chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect()
    }

    /// Pairs each `#[serde(rename = "wire")]` in a struct body with the field it
    /// decorates (the next `ident:` after the attribute).
    fn renamed_fields(body: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for seg in body.split("rename = \"").skip(1) {
            let Some(end) = seg.find('"') else { continue };
            let wire_name = seg[..end].to_string();
            let after = &seg[end..];
            let Some(colon) = after.find(':') else {
                continue;
            };
            let field: String = after[..colon]
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            if wire_name.is_empty() || field.is_empty() {
                continue;
            }
            out.push((field, wire_name));
        }
        out
    }

    fn to_camel_case(snake: &str) -> String {
        let mut out = String::with_capacity(snake.len());
        let mut upper_next = false;
        for c in snake.chars() {
            if c == '_' {
                upper_next = true;
            } else if upper_next {
                out.extend(c.to_uppercase());
                upper_next = false;
            } else {
                out.push(c);
            }
        }
        out
    }
}

/// Guards for the open-core feature gate (backlog A-3).
#[cfg(test)]
mod enterprise_gate {
    /// Source files carrying the authorization path. These must compile into
    /// **every** build, including the default MIT one.
    const AUTHORIZATION_SURFACE: &[&str] = &[
        "src/hoops/governance.rs",
        "src/services/rbac.rs",
        "src/mcp/capability.rs",
        "src/routers/mcp.rs",
        "src/a2a/handler.rs",
    ];

    const ENTERPRISE_CFG: &str = "feature = \"enterprise\"";

    fn read(relative: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
    }

    /// This file's source with comments and this very test module stripped, so a
    /// mention of a symbol in prose can never be counted as a code reference.
    ///
    /// The D-i guard shipped with exactly that bug: doc comments *explaining* the
    /// thing being guarded satisfied a `contains` check, so deleting the real
    /// attribute left the test green. The first draft of the test below repeated
    /// it — its own assertion strings mention the symbol it counts. Prose and code
    /// have to be separated before either is measured.
    fn router_code_only() -> String {
        let src = read("src/routers/mod.rs");
        let code = src
            .split("mod enterprise_gate {")
            .next()
            .unwrap_or(&src)
            .to_string();
        code.lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// One half of the over-gating defence, and **not** the load-bearing half.
    ///
    /// Gating a definition alone does not actually compile: `routers/mod.rs`
    /// imports `governance_middleware`, so adding a cfg to it fails the default
    /// build with `unresolved import` — verified by negative control. This test is
    /// belt-and-braces for that case; it turns a confusing link error into a
    /// message that names the constraint.
    ///
    /// The case the compiler genuinely cannot see is covered by
    /// [`enterprise_cfg_count_in_this_file_is_pinned`].
    ///
    /// Encodes the C-1 constraint that the strong-typed authorization path must not
    /// depend on enterprise wiring.
    #[test]
    fn authorization_surface_is_not_enterprise_gated() {
        for file in AUTHORIZATION_SURFACE {
            let src = read(file);
            assert!(
                !src.contains(ENTERPRISE_CFG),
                "{file} is on the authorization path and must compile into the default \
                 build. Gating it yields an MIT binary with no authorization on \
                 REST/MCP/A2A — see `billing_router` and backlog C-1/A-3."
            );
        }
    }

    /// The last line of over-gating defence, for the one shape that compiles.
    ///
    /// Most ways of gating security out of the default build simply do not build,
    /// each verified by negative control:
    ///
    /// - cfg on the definition → `unresolved import` (this file imports it);
    /// - cfg on the `governance_layer` binding alone → `cannot find value`;
    /// - cfg on a chained `.route_layer(...)` → not valid Rust in that position.
    ///
    /// What does compile is restructuring the chain and applying the layer
    /// conditionally — `let r = ...;` then `#[cfg(...)] let r = r.route_layer(...);`.
    /// Both channels build cleanly, every definition still exists, and the MIT
    /// binary quietly serves every route ungoverned. The sibling test above cannot
    /// see it either, because no cfg ever appears in the authorization-path files.
    ///
    /// Any such edit must add a cfg **to this file**, so the count is pinned. Three
    /// are expected, all billing: the `mod billing;` declaration, the enterprise
    /// `billing_router`, and its `cfg(not(...))` counterpart. A fourth is not
    /// automatically wrong — but it is a new decision about what the open-core split
    /// removes, and it should be made deliberately rather than arrived at.
    #[test]
    fn enterprise_cfg_count_in_this_file_is_pinned() {
        let code = router_code_only();
        let count = code.matches(ENTERPRISE_CFG).count();
        assert_eq!(
            count, 3,
            "expected exactly 3 enterprise cfgs in routers/mod.rs (mod billing, \
             billing_router, and its cfg(not) counterpart); found {count}. A new gate \
             here may be removing security from the default MIT build — see \
             `billing_router` for why the A-3 gate deliberately stops at billing."
        );
    }

    /// The gate must actually cover the billing surface: the module declaration is
    /// gated, and every handler reference sits inside the gated `billing_router`.
    #[test]
    fn billing_surface_is_fully_behind_the_gate() {
        let src = read("src/routers/mod.rs");
        assert!(
            src.contains(&format!("#[cfg({ENTERPRISE_CFG})]\nmod billing;")),
            "the billing module declaration must carry the enterprise cfg, or it \
             compiles into the default MIT build"
        );

        let code = router_code_only();
        let gated = code
            .split("fn billing_router() -> Router {")
            .nth(1)
            .expect("the enterprise billing_router must exist");
        let gated = &gated[..gated.find("\n}").unwrap_or(gated.len())];

        let inside = gated.matches("billing::").count();
        let total = code.matches("billing::").count();
        assert!(
            inside > 0,
            "billing_router must register the billing handlers"
        );
        assert_eq!(
            inside, total,
            "every `billing::` reference must live inside the gated billing_router; \
             {total} found in this file, {inside} of them inside it. A reference \
             elsewhere breaks the default build."
        );
    }
}
