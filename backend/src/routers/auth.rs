use axum::extract::{Extension, Query};
use axum::response::{Html, IntoResponse, Redirect};
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use rinja::Template;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::hoops::jwt;
use crate::models::User;
use crate::{db, json_ok, utils, AppError, AppResult, JsonResult};

pub async fn login_page(jar: CookieJar) -> AppResult<impl IntoResponse> {
    #[derive(Template)]
    #[template(path = "login.html")]
    struct LoginTemplate {}
    if let Some(cookie) = jar.get("jwt_token") {
        let token = cookie.value().to_string();
        if jwt::decode_token(&token) {
            return Ok(Redirect::to("/users").into_response());
        }
    }
    let hello_tmpl = LoginTemplate {};
    let html = hello_tmpl
        .render()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Html(html).into_response())
}

#[derive(Deserialize, ToSchema, Default, Debug)]
pub struct LoginInData {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema, Debug)]
pub struct RegisterInData {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, ToSchema, Default, Debug)]
pub struct LoginOutData {
    pub id: String,
    pub username: String,
    pub token: String,
    pub exp: i64,
}

/// Register a new user (public endpoint)
pub async fn register(Json(idata): Json<RegisterInData>) -> JsonResult<LoginOutData> {
    let conn = db::pool();

    // Check if user already exists
    let existing =
        sqlx::query_as::<_, User>("SELECT id, username, password FROM users WHERE username = $1")
            .bind(&idata.username)
            .fetch_optional(conn)
            .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Username already exists".to_string()));
    }

    // Create new user
    let id = ulid::Ulid::new().to_string();
    let password = utils::hash_password(&idata.password)?;
    sqlx::query("INSERT INTO users (id, username, password) VALUES ($1, $2, $3)")
        .bind(&id)
        .bind(&idata.username)
        .bind(&password)
        .execute(conn)
        .await?;

    // Generate token
    let (token, exp) = jwt::get_token(&id)?;
    let odata = LoginOutData {
        id,
        username: idata.username,
        token,
        exp,
    };
    json_ok(odata)
}

pub async fn post_login(
    jar: CookieJar,
    Json(idata): Json<LoginInData>,
) -> AppResult<(CookieJar, Json<LoginOutData>)> {
    let conn = db::pool();
    let Some(User {
        id,
        username,
        password,
    }) = sqlx::query_as::<_, User>("SELECT id, username, password FROM users WHERE username = $1")
        .bind(&idata.username)
        .fetch_optional(conn)
        .await?
    else {
        return Err(AppError::Unauthorized("User does not exist.".to_string()));
    };

    if utils::verify_password(&idata.password, &password).is_err() {
        return Err(AppError::Unauthorized(
            "Account not exist or password is incorrect.".to_string(),
        ));
    }

    let (token, exp) = jwt::get_token(&id)?;
    let odata = LoginOutData {
        id,
        username,
        token,
        exp,
    };
    let cookie = Cookie::build(("jwt_token", odata.token.clone()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::None)
        .build();
    Ok((jar.add(cookie), Json(odata)))
}

#[derive(Deserialize, Debug, Default)]
pub struct TokenQuery {
    pub token: Option<String>,
}

pub async fn post_login_with_token(
    jar: CookieJar,
    Query(query): Query<TokenQuery>,
    body: Option<Json<LoginInData>>,
) -> AppResult<(CookieJar, Json<LoginOutData>)> {
    if let Some(token) = query.token.as_deref() {
        if !token.is_empty() {
            if !jwt::decode_token(token) {
                return Err(AppError::Unauthorized(
                    "Token is invalid or expired.".to_string(),
                ));
            }
            let cookie = Cookie::build(("jwt_token", token.to_string()))
                .path("/")
                .http_only(true)
                .same_site(SameSite::None)
                .build();
            return Ok((
                jar.add(cookie),
                Json(LoginOutData {
                    id: "".to_string(),
                    username: "".to_string(),
                    token: token.to_string(),
                    exp: 0,
                }),
            ));
        }
    }

    let Json(idata) = body.ok_or_else(|| {
        AppError::BadRequest(
            "Invalid request body. Expected JSON with username and password, or provide token as query parameter.".to_string(),
        )
    })?;

    let conn = db::pool();
    let Some(User {
        id,
        username,
        password,
    }) = sqlx::query_as::<_, User>("SELECT id, username, password FROM users WHERE username = $1")
        .bind(&idata.username)
        .fetch_optional(conn)
        .await?
    else {
        return Err(AppError::Unauthorized("User does not exist.".to_string()));
    };

    if utils::verify_password(&idata.password, &password).is_err() {
        return Err(AppError::Unauthorized(
            "Account not exist or password is incorrect.".to_string(),
        ));
    }

    let (token, exp) = jwt::get_token(&id)?;
    let odata = LoginOutData {
        id,
        username,
        token,
        exp,
    };
    let cookie = Cookie::build(("jwt_token", odata.token.clone()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::None)
        .build();
    Ok((jar.add(cookie), Json(odata)))
}

#[derive(Serialize, ToSchema, Default, Debug)]
pub struct TokenVerifyResponse {
    pub valid: bool,
    pub message: String,
}

pub async fn get_login_with_token(
    jar: CookieJar,
    Query(query): Query<TokenQuery>,
) -> AppResult<(CookieJar, Json<TokenVerifyResponse>)> {
    let token = match query.token {
        Some(t) if !t.is_empty() => t,
        _ => {
            return Ok((
                jar,
                Json(TokenVerifyResponse {
                    valid: false,
                    message: "Token parameter is required".to_string(),
                }),
            ));
        }
    };

    let valid = jwt::decode_token(&token);

    if valid {
        let cookie = Cookie::build(("jwt_token", token.clone()))
            .path("/")
            .http_only(true)
            .same_site(SameSite::None)
            .build();
        Ok((
            jar.add(cookie),
            Json(TokenVerifyResponse {
                valid: true,
                message: "Token is valid".to_string(),
            }),
        ))
    } else {
        Ok((
            jar,
            Json(TokenVerifyResponse {
                valid: false,
                message: "Token is invalid or expired".to_string(),
            }),
        ))
    }
}

#[derive(Serialize, ToSchema, Default, Debug)]
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

#[derive(Serialize, ToSchema, Default, Debug)]
pub struct Tag {
    pub key: Option<String>,
    pub label: Option<String>,
}

#[derive(Serialize, ToSchema, Default, Debug)]
pub struct Geographic {
    pub province: Option<Location>,
    pub city: Option<Location>,
}

#[derive(Serialize, ToSchema, Default, Debug)]
pub struct Location {
    pub label: Option<String>,
    pub key: Option<String>,
}

pub async fn get_current_user(
    Extension(claims): Extension<jwt::JwtClaims>,
) -> JsonResult<CurrentUserResponse> {
    let conn = db::pool();
    let user = sqlx::query_as::<_, User>("SELECT id, username, password FROM users WHERE id = $1")
        .bind(&claims.uid)
        .fetch_optional(conn)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    json_ok(CurrentUserResponse {
        name: user.username.clone(),
        avatar: None,
        userid: user.id.clone(),
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
    })
}
