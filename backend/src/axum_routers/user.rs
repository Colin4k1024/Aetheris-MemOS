//! User routes - simplified

use axum::{
    routing::{get, post, put, delete},
    Router,
};

/// List users
async fn list_users() -> &'static str {
    "[]"
}

/// Create user
async fn create_user() -> &'static str {
    "{}"
}

/// Update user
async fn update_user() -> &'static str {
    "{}"
}

/// Delete user
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
