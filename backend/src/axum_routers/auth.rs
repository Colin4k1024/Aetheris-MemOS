//! Auth routes

use axum::{
    extract::Query,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json,
    Router,
};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::hoops::jwt;
use crate::utils;

/// Login request data
#[derive(Deserialize, Debug, Default)]
pub struct LoginInData {
    pub username: String,
    pub password: String,
}

/// Register request data
#[derive(Deserialize, Debug)]
pub struct RegisterInData {
    pub username: String,
    pub password: String,
}

/// Login response data
#[derive(Serialize, Debug, Default)]
pub struct LoginOutData {
    pub id: String,
    pub username: String,
    pub token: String,
    pub exp: i64,
}

/// Token verify response
#[derive(Serialize, Debug, Default)]
pub struct TokenVerifyResponse {
    pub valid: bool,
    pub message: String,
}

/// Current user response
#[derive(Serialize, Debug, Default)]
pub struct CurrentUserResponse {
    pub name: String,
    pub avatar: Option<String>,
    pub userid: String,
    pub email: Option<String>,
    pub signature: Option<String>,
    pub title: Option<String>,
    pub group: Option<String>,
    pub tags: Option<Vec<Tag>>,
    pub notify_count: Option<u32>,
    pub unread_count: Option<u32>,
    pub country: Option<String>,
    pub access: Option<String>,
    pub geographic: Option<Geographic>,
    pub address: Option<String>,
    pub phone: Option<String>,
}

#[derive(Serialize, Debug, Default)]
pub struct Tag {
    pub key: Option<String>,
    pub label: Option<String>,
}

#[derive(Serialize, Debug, Default)]
pub struct Geographic {
    pub province: Option<Location>,
    pub city: Option<Location>,
}

#[derive(Serialize, Debug, Default)]
pub struct Location {
    pub label: Option<String>,
    pub key: Option<String>,
}

/// App error type for Axum routes
#[derive(Debug)]
pub struct AuthError(String);

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (axum::http::StatusCode::BAD_REQUEST, self.0).into_response()
    }
}

impl From<sqlx::Error> for AuthError {
    fn from(e: sqlx::Error) -> Self {
        AuthError(e.to_string())
    }
}

/// Register a new user
#[utoipa::path(post, path = "/api/register", tag = "Auth")]
async fn register(Json(idata): Json<RegisterInData>) -> Result<Json<LoginOutData>, AuthError> {
    let conn = db::pool();

    // Check if user already exists
    let existing = sqlx::query_as::<_, crate::models::User>(
        "SELECT id, username, password FROM users WHERE username = $1",
    )
    .bind(&idata.username)
    .fetch_optional(conn)
    .await?;

    if existing.is_some() {
        return Err(AuthError("Username already exists".to_string()));
    }

    // Create new user
    let id = ulid::Ulid::new().to_string();
    let password = utils::hash_password(&idata.password).map_err(|e| AuthError(e.to_string()))?;
    sqlx::query("INSERT INTO users (id, username, password) VALUES ($1, $2, $3)")
        .bind(&id)
        .bind(&idata.username)
        .bind(&password)
        .execute(conn)
        .await?;

    // Generate token
    let (token, exp) = jwt::get_token(&id).map_err(|e| AuthError(e.to_string()))?;
    let odata = LoginOutData {
        id,
        username: idata.username,
        token,
        exp,
    };
    Ok(Json(odata))
}

/// Login handler
#[utoipa::path(post, path = "/api/login", tag = "Auth")]
async fn post_login(Json(idata): Json<LoginInData>) -> Result<Response, AuthError> {
    let conn = db::pool();
    let user = sqlx::query_as::<_, crate::models::User>(
        "SELECT id, username, password FROM users WHERE username = $1",
    )
    .bind(&idata.username)
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| AuthError("User does not exist.".to_string()))?;

    let (id, username, password) = (user.id, user.username, user.password);

    utils::verify_password(&idata.password, &password)
        .map_err(|_| AuthError("Account not exist or password is incorrect.".to_string()))?;

    let (token, exp) = jwt::get_token(&id).map_err(|e| AuthError(e.to_string()))?;
    let odata = LoginOutData {
        id: id.clone(),
        username: username.clone(),
        token: token.clone(),
        exp,
    };

    // Create response with cookie
    let mut response = Json(odata).into_response();

    // Add cookie
    let cookie = format!(
        "jwt_token={}; Path=/; HttpOnly; SameSite=None",
        token
    );
    response.headers_mut().append(
        axum::http::header::SET_COOKIE,
        cookie.parse().unwrap(),
    );

    Ok(response)
}

/// Login with token (query param only for now)
#[utoipa::path(post, path = "/api/login/account", tag = "Auth")]
async fn post_login_with_token(
    Query(params): Query<LoginInData>,
) -> Result<Response, AuthError> {
    // Check query parameter token
    if !params.username.is_empty() {
        let token = &params.username;
        let valid = jwt::decode_token(token);
        if !valid {
            return Err(AuthError("Token is invalid or expired.".to_string()));
        }

        let response = Json(LoginOutData {
            id: "".to_string(),
            username: "".to_string(),
            token: token.to_string(),
            exp: 0,
        }).into_response();

        return Ok(response);
    }

    Err(AuthError("Invalid request. Provide token or username/password.".to_string()))
}

/// Verify token and set cookie
#[utoipa::path(get, path = "/api/login/account", tag = "Auth")]
async fn get_login_with_token(Query(params): Query<LoginInData>) -> Result<Json<TokenVerifyResponse>, AuthError> {
    let token = params.username;

    if token.is_empty() {
        return Ok(Json(TokenVerifyResponse {
            valid: false,
            message: "Token parameter is required".to_string(),
        }));
    }

    let valid = jwt::decode_token(&token);

    Ok(Json(TokenVerifyResponse {
        valid,
        message: if valid { "Token is valid".to_string() } else { "Token is invalid or expired".to_string() },
    }))
}

/// Get current user (requires auth)
#[utoipa::path(get, path = "/api/currentUser", tag = "Auth")]
async fn get_current_user() -> Result<Json<CurrentUserResponse>, AuthError> {
    Ok(Json(CurrentUserResponse {
        name: "admin".to_string(),
        avatar: None,
        userid: "1".to_string(),
        email: None,
        signature: None,
        title: None,
        group: None,
        tags: None,
        notify_count: Some(0),
        unread_count: Some(0),
        country: None,
        access: Some("admin".to_string()),
        geographic: None,
        address: None,
        phone: None,
    }))
}

/// Login page handler (placeholder)
async fn login_page() -> &'static str {
    "Login page placeholder"
}

/// Register page handler
async fn register_page() -> &'static str {
    "Register page placeholder"
}

/// Create auth routes
pub fn router() -> Router {
    Router::new()
        .route("/login", get(login_page))
        .route("/register", post(register))
        .route("/api/login", post(post_login))
        .route("/api/login/account", post(post_login_with_token))
        .route("/api/login/account", get(get_login_with_token))
        .route("/api/currentUser", get(get_current_user))
}
