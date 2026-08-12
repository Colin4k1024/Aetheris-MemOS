use anyhow::Result;
use axum::{extract::Request, middleware::Next, response::Response};
use jsonwebtoken::{decode, Algorithm, DecodingKey, EncodingKey, Validation};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::config;
use crate::services::prometheus_exporter::get_exporter;
use crate::tenant::RequestTenantContext;
use crate::AppError;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JwtClaims {
    pub uid: String,
    pub exp: i64,
    /// The org (tenant) this token is scoped to. `None` means the caller's
    /// personal org (`tenant_id = uid`), which is the backward-compatible default
    /// for tokens issued before the org claim existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Transport-agnostic authenticator core (ADR-0007, P2 PR-1).
//
// All four protocols (REST, MCP-over-HTTP, gRPC, WebSocket, A2A) converge on
// `authenticate()` through their respective transport adapters. This is the
// **single source of truth** for JWT validation + tenant context construction.
// ─────────────────────────────────────────────────────────────────────────────

/// Transport-agnostic authentication: validate a raw JWT token and return
/// claims + tenant context.
///
/// This is the **single source of truth** for JWT validation. All protocols
/// (REST/MCP/gRPC/WS/A2A) call this through their transport adapters.
///
/// # Security
/// - HS256 only (matches config.jwt.secret).
/// - Rejects expired tokens.
/// - Constructs `RequestTenantContext` from the `uid` claim (MVP: user == tenant).
///
/// # Errors
/// Returns `AppError::Unauthorized` if the token is missing, malformed,
/// expired, or fails signature verification.
pub fn authenticate(token: &str) -> Result<(JwtClaims, RequestTenantContext), AppError> {
    let Some(claims) = decode_token_claims(token) else {
        // A malformed/bad-signature token and — because jsonwebtoken validates
        // `exp` by default — an already-expired one both land here, so
        // `invalid_token` is the dominant reason. The explicit `expired` branch
        // below is a rarely-hit belt-and-suspenders.
        get_exporter().inc_auth_failure("invalid_token");
        return Err(AppError::Unauthorized(
            "Token is invalid or expired".to_string(),
        ));
    };

    // Explicit expiry check — jwtwebtoken's Validation does this by default,
    // but we double-check to be explicit about the security boundary.
    let now = OffsetDateTime::now_utc().unix_timestamp();
    if claims.exp < now {
        get_exporter().inc_auth_failure("expired");
        return Err(AppError::Unauthorized("Token has expired".to_string()));
    }

    let tenant_id = claims.org.clone().unwrap_or_else(|| claims.uid.clone());
    let tenant_ctx = RequestTenantContext::from_authenticated(tenant_id, claims.uid.clone());
    Ok((claims, tenant_ctx))
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

/// REST/HTTP auth middleware — delegates to `authenticate()` core.
///
/// For gRPC (tonic Interceptor), WebSocket (axum handshake), and A2A (HTTP
/// middleware), each transport adapter calls `authenticate()` directly with the
/// raw token extracted from its protocol-specific headers.
pub async fn auth_middleware(mut req: Request, next: Next) -> Result<Response, AppError> {
    // Allow disabling auth via config (for Docker demo/dev environments)
    if config::get().jwt.disabled {
        let claims = JwtClaims {
            uid: "anonymous".to_string(),
            exp: i64::MAX,
            org: None,
        };
        req.extensions_mut().insert(claims);
        let tenant_ctx = RequestTenantContext::from_authenticated(
            "anonymous".to_string(),
            "anonymous".to_string(),
        );
        req.extensions_mut().insert(tenant_ctx);
        return Ok(next.run(req).await);
    }

    // Reject tokens in query strings (security: prevent token leakage in logs/referrer)
    if let Some(query) = req.uri().query() {
        if query.contains("token=") || query.contains("jwt_token=") {
            get_exporter().inc_auth_failure("query_param_token");
            return Err(AppError::Unauthorized(
                "Token query parameter not supported. Use httpOnly cookie or Authorization header."
                    .to_string(),
            ));
        }
    }

    let token = extract_token(&req).ok_or_else(|| {
        get_exporter().inc_auth_failure("missing_token");
        AppError::Unauthorized("Missing authentication token".to_string())
    })?;

    // Delegate to the transport-agnostic authenticator core. Its own invalid/
    // expired failures are counted inside `authenticate()` (so all transports
    // share that accounting) — we must NOT re-count them here.
    let (claims, tenant_ctx) = authenticate(&token)?;

    req.extensions_mut().insert(claims);
    req.extensions_mut().insert(tenant_ctx);

    Ok(next.run(req).await)
}

/// Mint a JWT. `org` is the tenant this token is scoped to; pass `None` for the
/// caller's personal org (= their user id, the backward-compatible default).
pub fn get_token(uid: impl Into<String>, org: Option<String>) -> Result<(String, i64)> {
    let exp = OffsetDateTime::now_utc() + Duration::seconds(config::get().jwt.expiry);
    let claim = JwtClaims {
        uid: uid.into(),
        org,
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
