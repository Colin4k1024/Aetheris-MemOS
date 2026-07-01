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
        "/api/v1/memory/storage/sessions",
        "/api/v1/memory/search/ltm",
        "/api/kg/entities",
        "/api/mm/list",
    ] {
        let response = get(path).await;
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{path} should be registered behind auth, not missing from the live router"
        );
    }
}
