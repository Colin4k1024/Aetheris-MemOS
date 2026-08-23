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
    let (token, _) = backend::hoops::jwt::get_token(user_id).expect("generate test JWT");
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
