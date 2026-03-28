use anyhow::Result;
use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, EncodingKey, Validation};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::config;
use crate::tenant::RequestTenantContext;
use crate::AppError;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JwtClaims {
    pub uid: String,
    pub exp: i64,
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

/// Extract JWT token from request.
///
/// Priority:
/// 1. `jwt_token` httpOnly cookie (primary - eliminates XSS vector)
/// 2. `Authorization: Bearer <token>` header (fallback for API clients)
///
/// Query string tokens are explicitly rejected.
fn extract_token(req: &Request) -> Option<String> {
    // 1. Try httpOnly cookie first
    if let Some(cookie_header) = req.headers().get(axum::http::header::COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for pair in cookie_str.split(';') {
                let pair = pair.trim();
                if let Some((key, value)) = pair.split_once('=') {
                    if key == "jwt_token" {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }

    // 2. Fall back to Authorization header for API clients (Brave, curl, etc.)
    req.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_string)
}

pub async fn auth_middleware(mut req: Request, next: Next) -> Result<Response, AppError> {
    // Reject tokens in query strings (security: prevent token leakage in logs/referrer)
    if let Some(query) = req.uri().query() {
        if query.contains("token=") || query.contains("jwt_token=") {
            return Err(AppError::Unauthorized(
                "Token query parameter not supported. Use httpOnly cookie or Authorization header."
                    .to_string(),
            ));
        }
    }

    let token = extract_token(&req)
        .ok_or_else(|| AppError::Unauthorized("Missing authentication token".to_string()))?;

    let claims = decode_token_claims(&token)
        .ok_or_else(|| AppError::Unauthorized("Token is invalid or expired".to_string()))?;

    req.extensions_mut().insert(claims.clone());

    // Populate tenant context from JWT uid claim (MVP: each user is their own tenant)
    let tenant_ctx = RequestTenantContext::new(&claims.uid);
    req.extensions_mut().insert(tenant_ctx);

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
