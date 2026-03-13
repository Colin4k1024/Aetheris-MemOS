//! JWT Authentication Middleware

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

/// JWT claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
}

/// JWT configuration
#[derive(Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub algorithm: Algorithm,
}

impl JwtConfig {
    pub fn new(secret: String) -> Self {
        Self {
            secret,
            algorithm: Algorithm::HS256,
        }
    }

    pub fn validation(&self) -> Validation {
        Validation::new(self.algorithm)
    }

    pub fn decoding_key(&self) -> DecodingKey {
        DecodingKey::from_secret(self.secret.as_bytes())
    }
}

/// Extract JWT from request
pub fn extract_jwt(request: &Request) -> Option<JwtClaims> {
    // Try Authorization header
    if let Some(auth_header) = request.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                return decode_token(token);
            }
        }
    }

    // Try query parameter
    if let Some(query) = request.uri().query() {
        for param in query.split('&') {
            if param.starts_with("token=") {
                let token = &param[6..];
                return decode_token(token);
            }
        }
    }

    None
}

fn decode_token(token: &str) -> Option<JwtClaims> {
    let config = crate::config::get();
    let jwt_config = JwtConfig::new(config.jwt.secret.clone());

    match decode::<JwtClaims>(
        token,
        &jwt_config.decoding_key(),
        &jwt_config.validation(),
    ) {
        Ok(token_data) => Some(token_data.claims),
        Err(_) => None,
    }
}

/// Auth middleware function (simplified)
pub async fn auth_middleware(request: Request, next: Next) -> Response {
    if extract_jwt(&request).is_some() {
        next.run(request).await
    } else {
        let body = serde_json::json!({ "error": "Invalid or missing token" });
        let mut res = axum::Json(body).into_response();
        *res.status_mut() = StatusCode::UNAUTHORIZED;
        res
    }
}
