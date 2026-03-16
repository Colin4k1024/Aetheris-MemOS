//! User routes - simplified

use axum::{
    routing::{get, post, put, delete},
    Router,
};

/// List users
#[utoipa::path(get, path = "/users", tag = "User")]
async fn list_users() -> &'static str {
    "[]"
}

/// Create user
#[utoipa::path(post, path = "/users", tag = "User")]
async fn create_user() -> &'static str {
    "{}"
}

/// Update user
#[utoipa::path(put, path = "/users/{user_id}", tag = "User")]
async fn update_user() -> &'static str {
    "{}"
}

/// Delete user
#[utoipa::path(delete, path = "/users/{user_id}", tag = "User")]
async fn delete_user() -> &'static str {
    "{}"
}

/// User list page
async fn list_page() -> &'static str {
    "User list page placeholder"
}

/// Create user routes
pub fn router() -> Router {
    Router::new()
        .route("/users", get(list_users))
        .route("/users", post(create_user))
        .route("/users/{user_id}", put(update_user))
        .route("/users/{user_id}", delete(delete_user))
}
