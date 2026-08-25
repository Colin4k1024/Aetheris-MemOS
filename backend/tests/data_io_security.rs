use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

static CONFIG_INIT: std::sync::Once = std::sync::Once::new();

fn ensure_config() {
    CONFIG_INIT.call_once(|| {
        backend::config::init();
    });
}

fn auth_header(user_id: &str) -> String {
    ensure_config();
    let (token, _) = backend::hoops::jwt::get_token(user_id, None).expect("generate test JWT");
    format!("Bearer {token}")
}

async fn response_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let json = serde_json::from_slice(&body).expect("parse JSON response");
    (status, json)
}

#[tokio::test]
async fn export_requires_a_jwt_before_accessing_storage() {
    ensure_config();
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/data/export?layer=unsupported")
                .body(Body::empty())
                .expect("build export request"),
        )
        .await
        .expect("serve export request");

    let (status, body) = response_json(response).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unexpected response: {body}"
    );
}

#[tokio::test]
async fn import_requires_a_jwt_before_accepting_data() {
    ensure_config();
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/data/import")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "format": "json", "data": {} }).to_string(),
                ))
                .expect("build import request"),
        )
        .await
        .expect("serve import request");

    let (status, body) = response_json(response).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unexpected response: {body}"
    );
}

#[tokio::test]
async fn authenticated_export_rejects_an_unsupported_layer_before_accessing_storage() {
    let user_id = "data-io-export-tenant";
    let auth = auth_header(user_id);
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/data/export?layer=unsupported")
                .header(header::AUTHORIZATION, auth)
                .body(Body::empty())
                .expect("build export request"),
        )
        .await
        .expect("serve export request");

    let (status, body) = response_json(response).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "unexpected response: {body}"
    );
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("Unsupported export layer"));
}

#[tokio::test]
async fn import_is_explicitly_not_implemented() {
    let auth = auth_header("data-io-import-tenant");
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/data/import")
                .header(header::AUTHORIZATION, auth)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "format": "json",
                        "data": { "ltm": { "entries": [{ "title": "not imported" }] } }
                    })
                    .to_string(),
                ))
                .expect("build import request"),
        )
        .await
        .expect("serve import request");

    let (status, body) = response_json(response).await;
    assert_eq!(
        status,
        StatusCode::NOT_IMPLEMENTED,
        "unexpected response: {body}"
    );
    assert_eq!(body["supported"], false);
}

// ── #85: import edge cases ──

#[tokio::test]
async fn import_rejects_empty_data() {
    let auth = auth_header("data-io-empty");
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/data/import")
                .header(header::AUTHORIZATION, auth)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"format": "json", "data": {}}).to_string()))
                .expect("build import request"),
        )
        .await
        .expect("serve import request");

    let (status, body) = response_json(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unexpected response: {body}");
    assert!(body["message"].as_str().unwrap().contains("empty"));
}

#[tokio::test]
async fn import_rejects_null_data() {
    let auth = auth_header("data-io-null");
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/data/import")
                .header(header::AUTHORIZATION, auth)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"format": "json", "data": null}).to_string()))
                .expect("build import request"),
        )
        .await
        .expect("serve import request");

    let (status, body) = response_json(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unexpected response: {body}");
    assert!(body["message"].as_str().unwrap().contains("empty"));
}

#[tokio::test]
async fn import_rejects_empty_array_data() {
    let auth = auth_header("data-io-arr-empty");
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/data/import")
                .header(header::AUTHORIZATION, auth)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"format": "json", "data": []}).to_string()))
                .expect("build import request"),
        )
        .await
        .expect("serve import request");

    let (status, body) = response_json(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unexpected response: {body}");
    assert!(body["message"].as_str().unwrap().contains("empty"));
}

#[tokio::test]
async fn import_rejects_unsupported_format() {
    let auth = auth_header("data-io-bad-format");
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/data/import")
                .header(header::AUTHORIZATION, auth)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"format": "yaml", "data": {"some": "data"}}).to_string(),
                ))
                .expect("build import request"),
        )
        .await
        .expect("serve import request");

    let (status, body) = response_json(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unexpected response: {body}");
    assert!(body["message"].as_str().unwrap().contains("Unsupported format"));
}

#[tokio::test]
async fn import_rejects_unsupported_mode() {
    let auth = auth_header("data-io-bad-mode");
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/data/import")
                .header(header::AUTHORIZATION, auth)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"format": "json", "mode": "upsert", "data": {"some": "data"}}).to_string(),
                ))
                .expect("build import request"),
        )
        .await
        .expect("serve import request");

    let (status, body) = response_json(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unexpected response: {body}");
    assert!(body["message"].as_str().unwrap().contains("Unsupported mode"));
}

#[tokio::test]
async fn import_rejects_unknown_fields() {
    let auth = auth_header("data-io-unknown");
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/data/import")
                .header(header::AUTHORIZATION, auth)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"format": "json", "data": {"k": "v"}, "tenant_id": "other"}).to_string(),
                ))
                .expect("build import request"),
        )
        .await
        .expect("serve import request");

    let status = response.status();
    let body_bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let body_text = String::from_utf8_lossy(&body_bytes);

    // axum's Json extractor with deny_unknown_fields returns 422 with a
    // text/plain body (not JSON) — verify the rejection.
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "unexpected status: {status}, body: {body_text}"
    );
    assert!(
        body_text.contains("unknown field"),
        "body should mention unknown field: {body_text}"
    );
}

#[tokio::test]
async fn import_dry_run_returns_not_implemented_with_flag() {
    let auth = auth_header("data-io-dry-run");
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/data/import")
                .header(header::AUTHORIZATION, auth)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"format": "json", "dry_run": true, "data": {"ltm": {"entries": [{"title": "x"}]}}}).to_string(),
                ))
                .expect("build import request"),
        )
        .await
        .expect("serve import request");

    let (status, body) = response_json(response).await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "unexpected response: {body}");
    assert_eq!(body["dry_run"], true);
    assert_eq!(body["success"], false);
    assert_eq!(body["supported"], false);
}

#[tokio::test]
async fn import_returns_not_implemented_for_valid_payload() {
    let auth = auth_header("data-io-valid");
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/data/import")
                .header(header::AUTHORIZATION, auth)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "format": "json",
                        "mode": "merge",
                        "data": {
                            "stm": {"sessions": [{"id": "s1", "messages": [{"role": "user", "content": "hello"}]}]},
                            "ltm": {"entries": [{"title": "test", "content": "value"}]},
                            "kg": {"entities": [{"name": "e1", "type": "concept"}]},
                            "mm": {"entries": [{"id": "m1", "type": "text"}]}
                        }
                    })
                    .to_string(),
                ))
                .expect("build import request"),
        )
        .await
        .expect("serve import request");

    let (status, body) = response_json(response).await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "unexpected response: {body}");
    assert_eq!(body["success"], false);
    assert_eq!(body["supported"], false);
    assert!(
        body["message"].as_str().unwrap().contains("not implemented"),
        "message should state not implemented: {body}"
    );
}
