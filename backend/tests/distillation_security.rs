//! Security boundary tests for the `/api/v1/distillation/*` routes.
//!
//! These routes were landed in 952a899 reading the shared default tenant
//! (`get_default_tenant()`) instead of the caller's authenticated tenant — a
//! cross-tenant data leak (same class as the P0s fixed in 77daf57). They now
//! scope every read/trigger by `RequestTenantContext::tenant_id`.
//!
//! These tests guard the auth boundary only (JWT required). Verifying the
//! tenant scoping end-to-end needs the `distillation_*` tables present in the
//! test DB with two tenants' rows — that is a test-infra follow-up, not
//! covered here.

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

async fn response_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let json = serde_json::from_slice(&body).expect("parse JSON response");
    (status, json)
}

#[tokio::test]
async fn list_atoms_requires_a_jwt_before_accessing_storage() {
    ensure_config();
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/distillation/atoms?user_id=u&agent_id=a")
                .body(Body::empty())
                .expect("build list_atoms request"),
        )
        .await
        .expect("serve list_atoms request");

    let (status, body) = response_json(response).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unexpected response: {body}"
    );
}

#[tokio::test]
async fn trigger_distillation_requires_a_jwt_before_enqueuing() {
    ensure_config();
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/distillation/trigger")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"user_id":"u","agent_id":"a","session_id":"s"}).to_string(),
                ))
                .expect("build trigger request"),
        )
        .await
        .expect("serve trigger request");

    let (status, body) = response_json(response).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unexpected response: {body}"
    );
}
