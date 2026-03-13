use anyhow::Result;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::{Duration, OffsetDateTime};

use crate::AppError;
use crate::config;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JwtClaims {
    pub uid: String,
    pub exp: i64,
}

fn parse_query_params(query: Option<&str>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if let Some(q) = query {
        for pair in q.split('&') {
            let mut parts = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                out.insert(k.to_string(), v.to_string());
            }
        }
    }
    out
}

fn token_from_cookie(cookie_header: Option<&str>) -> Option<String> {
    let header = cookie_header?;
    for seg in header.split(';') {
        let part = seg.trim();
        if let Some(value) = part.strip_prefix("jwt_token=") {
            return Some(value.to_string());
        }
    }
    None
}

pub fn decode_token_claims(token: &str) -> Option<JwtClaims> {
    let validation = Validation::new(Algorithm::HS256);
    decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(config::get().jwt.secret.as_bytes()),
        &validation,
    )
    .ok()
    .map(|d| d.claims)
}

pub async fn auth_middleware(mut req: Request, next: Next) -> Result<Response, AppError> {
    let auth_header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));

    let query_token = parse_query_params(req.uri().query())
        .get("token")
        .cloned()
        .filter(|s| !s.is_empty());

    let cookie_token = token_from_cookie(
        req.headers()
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok()),
    );

    let token = auth_header
        .or(query_token)
        .or(cookie_token)
        .ok_or_else(|| AppError::Unauthorized("Missing authentication token".to_string()))?;

    let claims = decode_token_claims(&token)
        .ok_or_else(|| AppError::Unauthorized("Token is invalid or expired".to_string()))?;

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

pub fn get_token(uid: impl Into<String>) -> Result<(String, i64)> {
    let exp = OffsetDateTime::now_utc() + Duration::seconds(config::get().jwt.expiry);
    let claim = JwtClaims {
        uid: uid.into(),
        exp: exp.unix_timestamp(),
    };
    let token: String = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claim,
        &EncodingKey::from_secret(config::get().jwt.secret.as_bytes()),
    )?;
    Ok((token, exp.unix_timestamp()))
}

#[allow(dead_code)]
pub fn decode_token(token: &str) -> bool {
    decode_token_claims(token).is_some()
}
