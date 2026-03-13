use anyhow::Result;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use jsonwebtoken::{decode, Algorithm, DecodingKey, EncodingKey, Validation};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::config;
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

pub async fn auth_middleware(mut req: Request, next: Next) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_string)
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
