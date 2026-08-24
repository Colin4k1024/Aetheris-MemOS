//! Security boundary tests for the `/api/v1/skills` routes (#90 first
//! increment). Guards the auth boundary only (JWT required). DB-level
//! cross-tenant RLS verification needs a PG + RLS test harness — follow-up.

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
async fn list_skills_requires_a_jwt() {
    ensure_config();
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills")
                .body(Body::empty())
                .expect("build list_skills request"),
        )
        .await
        .expect("serve list_skills request");

    let (status, body) = response_json(response).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unexpected response: {body}"
    );
}

#[tokio::test]
async fn create_skill_requires_a_jwt() {
    ensure_config();
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "name": "deploy",
                        "description": "deploy a service",
                        "trigger_conditions": [],
                        "execution_steps": [],
                        "validation_rules": [],
                        "owner_agent_id": "agent-1",
                        "visibility": "private"
                    })
                    .to_string(),
                ))
                .expect("build create_skill request"),
        )
        .await
        .expect("serve create_skill request");

    let (status, body) = response_json(response).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unexpected response: {body}"
    );
}

#[tokio::test]
async fn publish_skill_requires_a_jwt() {
    ensure_config();
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills/sk-1/publish")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({}).to_string()))
                .expect("build publish_skill request"),
        )
        .await
        .expect("serve publish_skill request");

    let (status, body) = response_json(response).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unexpected response: {body}"
    );
}

#[tokio::test]
async fn extract_skills_requires_a_jwt() {
    ensure_config();
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills/extract")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "transcript": "user did X then Y" }).to_string(),
                ))
                .expect("build extract_skills request"),
        )
        .await
        .expect("serve extract_skills request");

    let (status, body) = response_json(response).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unexpected response: {body}"
    );
}
