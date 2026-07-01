use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

static CONFIG_INIT: std::sync::Once = std::sync::Once::new();

fn ensure_config() {
    CONFIG_INIT.call_once(|| {
        backend::config::init();
    });
}

async fn get(path: &str) -> axum::response::Response {
    ensure_config();
    backend::axum_routers::create_router()
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("serve request")
}

#[tokio::test]
async fn live_entry_serves_root_page() {
    let response = get("/").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    assert_eq!(&body[..], b"Hello World from axum");
}

#[tokio::test]
async fn mvp_memory_routes_are_registered_on_live_entry() {
    for path in [
        "/api/v1/memory/adaptive/status",
        "/api/v1/memory/traces",
        "/api/v1/memory/explain",
        "/api/v1/memory/storage/sessions",
        "/api/v1/memory/search/ltm",
        "/api/v1/memory/search/hybrid",
        "/api/v1/memory/search/triple",
        "/api/v1/memory/search/scored",
        "/api/v1/memory/storage/qdrant/backfill-tenant-metadata",
        "/api/kg/entities",
        "/api/kg/entities/by-name/example",
        "/api/kg/entities/example/related",
        "/api/mm/list",
        "/api/mm/entry/example",
        "/api/mm/session/example",
        "/api/mm/modality/text",
        "/api/mcp/tools",
    ] {
        let response = get(path).await;
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{path} should be registered behind auth, not missing from the live router"
        );
    }
}

#[tokio::test]
async fn openapi_lists_stable_mvp_routes() {
    let response = get("/api-doc/openapi.json").await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let spec: serde_json::Value = serde_json::from_slice(&body).expect("valid openapi json");
    let paths = spec
        .get("paths")
        .and_then(|v| v.as_object())
        .expect("openapi paths object");

    for path in [
        "/api/v1/memory/feedback",
        "/api/v1/memory/forget",
        "/api/v1/memory/storage/stm",
        "/api/v1/memory/storage/ltm",
        "/api/v1/memory/search/ltm",
        "/api/v1/memory/search/hybrid",
        "/api/v1/memory/search/triple",
        "/api/v1/memory/search/scored",
        "/api/v1/memory/storage/qdrant/backfill-tenant-metadata",
        "/api/kg/entities",
        "/api/kg/entities/by-name/{name}",
        "/api/kg/entities/{entity_id}/related",
        "/api/mm/list",
        "/api/mm/store",
        "/api/mm/entry/{entry_id}",
        "/api/mm/session/{session_id}",
        "/api/mm/modality/{modality_type}",
        "/api/mcp/tools",
        "/api/mcp/tools/call",
    ] {
        assert!(paths.contains_key(path), "missing stable MVP path {path}");
    }

    let schemas = spec
        .pointer("/components/schemas")
        .and_then(|v| v.as_object())
        .expect("openapi components.schemas object");

    for schema in [
        "SelectMemoryRequest",
        "StoreSTMRequest",
        "StoreLTMRequest",
        "BackfillQdrantTenantMetadataRequest",
        "QdrantTenantBackfillReport",
        "SearchResult",
        "ToolCallParams",
        "ToolCallResponse",
    ] {
        assert!(schemas.contains_key(schema), "missing schema {schema}");
    }

    assert!(spec
        .pointer("/paths/~1api~1v1~1memory~1storage~1ltm/post/requestBody")
        .is_some());
    assert!(spec
        .pointer(
            "/paths/~1api~1mcp~1tools~1call/post/responses/200/content/application~1json/schema"
        )
        .is_some());
}
