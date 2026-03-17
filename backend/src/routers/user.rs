use crate::hoops::jwt;
use axum::extract::{Path, Query};
use axum::response::{Html, IntoResponse, Redirect};
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use rinja::Template;
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use utoipa::ToSchema;
use validator::Validate;

use crate::models::SafeUser;
use crate::{db, empty_ok, json_ok, utils, AppError, AppResult, EmptyResult, JsonResult};

#[derive(Template)]
#[template(path = "user_list_page.html")]
pub struct UserListPageTemplate {}

#[derive(Template)]
#[template(path = "user_list_frag.html")]
pub struct UserListFragTemplate {}

pub async fn list_page(
    headers: axum::http::HeaderMap,
    jar: CookieJar,
) -> AppResult<impl IntoResponse> {
    let is_fragment = headers.get("X-Fragment-Header");
    if let Some(cookie) = jar.get("jwt_token") {
        let token = cookie.value().to_string();
        if !jwt::decode_token(&token) {
            return Ok(Redirect::to("/login").into_response());
        }
    }
    match is_fragment {
        Some(_) => {
            let hello_tmpl = UserListFragTemplate {};
            let html = hello_tmpl
                .render()
                .map_err(|e| AppError::Internal(e.to_string()))?;
            Ok(Html(html).into_response())
        }
        None => {
            let hello_tmpl = UserListPageTemplate {};
            let html = hello_tmpl
                .render()
                .map_err(|e| AppError::Internal(e.to_string()))?;
            Ok(Html(html).into_response())
        }
    }
}

#[derive(Deserialize, Debug, Validate, ToSchema, Default)]
pub struct CreateInData {
    #[validate(length(min = 5, message = "username length must be greater than 5"))]
    pub username: String,
    #[validate(length(min = 6, message = "password length must be greater than 5"))]
    pub password: String,
}
pub async fn create_user(Json(idata): Json<CreateInData>) -> JsonResult<SafeUser> {
    let CreateInData { username, password } = idata;
    let id = Ulid::new().to_string();
    let password = utils::hash_password(&password)?;
    let conn = db::pool();
    sqlx::query("INSERT INTO users (id, username, password) VALUES ($1, $2, $3)")
        .bind(&id)
        .bind(&username)
        .bind(&password)
        .execute(conn)
        .await?;

    json_ok(SafeUser { id, username })
}

#[derive(Deserialize, Debug, Validate, ToSchema)]
pub struct UpdateInData {
    #[validate(length(min = 5, message = "username length must be greater than 5"))]
    username: String,
    #[validate(length(min = 6, message = "password length must be greater than 5"))]
    password: String,
}
pub async fn update_user(
    Path(user_id): Path<String>,
    Json(idata): Json<UpdateInData>,
) -> JsonResult<SafeUser> {
    let UpdateInData { username, password } = idata;
    let conn = db::pool();
    sqlx::query("UPDATE users SET username = $1, password = $2 WHERE id = $3")
        .bind(&username)
        .bind(&password)
        .bind(&user_id)
        .execute(conn)
        .await?;
    json_ok(SafeUser {
        id: user_id,
        username,
    })
}

pub async fn delete_user(Path(user_id): Path<String>) -> EmptyResult {
    let conn = db::pool();
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&user_id)
        .execute(conn)
        .await?;
    empty_ok()
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UserListQuery {
    pub username: Option<String>,
    #[serde(default = "default_page")]
    pub current_page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

fn default_page() -> i64 {
    1
}
fn default_page_size() -> i64 {
    10
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserListResponse {
    pub data: Vec<SafeUser>,
    pub total: i64,
    pub current_page: i64,
    pub page_size: i64,
}

pub async fn list_users(Query(query): Query<UserListQuery>) -> JsonResult<UserListResponse> {
    let conn = db::pool();
    let username_filter = query.username.clone().unwrap_or_default();
    let like_pattern = format!("%{}%", username_filter);
    let offset = (query.current_page - 1) * query.page_size;

    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM users WHERE username LIKE $1")
            .bind(&like_pattern)
            .fetch_one(conn)
            .await?;

    let users = sqlx::query_as::<_, SafeUser>(
        "SELECT id, username FROM users WHERE username LIKE $1 LIMIT $2 OFFSET $3",
    )
    .bind(&like_pattern)
    .bind(query.page_size)
    .bind(offset)
    .fetch_all(conn)
    .await?;

    json_ok(UserListResponse {
        data: users,
        total,
        current_page: query.current_page,
        page_size: query.page_size,
    })
}
