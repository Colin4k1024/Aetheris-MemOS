//! Security boundary tests for agent sub-resource routes (#89):
//! `/api/v1/agents/{id}/equipment`, `/loadout`, `/acl`.
//!
//! Guard the auth boundary (JWT required). DB-level cross-tenant RLS
//! verification needs a PG + RLS test harness — follow-up.

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
async fn list_equipment_requires_a_jwt() {
    ensure_config();
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/agents/agent-1/equipment")
                .body(Body::empty())
                .expect("build list_equipment request"),
        )
        .await
        .expect("serve list_equipment request");

    let (status, body) = response_json(response).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unexpected response: {body}"
    );
}

#[tokio::test]
async fn add_equipment_requires_a_jwt() {
    ensure_config();
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/agents/agent-1/equipment")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "asset_type": "skill",
                        "asset_id": "sk-1",
                        "binding_type": "fixed",
                        "priority": 0
                    })
                    .to_string(),
                ))
                .expect("build add_equipment request"),
        )
        .await
        .expect("serve add_equipment request");

    let (status, body) = response_json(response).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unexpected response: {body}"
    );
}

// ── Loadout & ACL endpoints (#89) ──

#[tokio::test]
async fn loadout_requires_a_jwt() {
    ensure_config();
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/agents/agent-1/loadout")
                .body(Body::empty())
                .expect("build loadout request"),
        )
        .await
        .expect("serve loadout request");

    let (status, body) = response_json(response).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "unexpected response: {body}");
}

#[tokio::test]
async fn acl_requires_a_jwt() {
    ensure_config();
    let app = backend::axum_routers::create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/agents/agent-1/acl")
                .body(Body::empty())
                .expect("build acl request"),
        )
        .await
        .expect("serve acl request");

    let (status, body) = response_json(response).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "unexpected response: {body}");
}
