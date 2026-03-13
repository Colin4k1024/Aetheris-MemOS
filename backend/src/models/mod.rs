use serde::{Deserialize, Serialize};
use sqlx::prelude::*;
use utoipa::ToSchema;

pub mod memory;
pub mod performance;
pub mod resource;
pub mod task;

pub use memory::*;
pub use performance::*;
pub use resource::*;
pub use task::*;

#[derive(FromRow, Serialize, Deserialize, Debug)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password: String,
}

#[derive(FromRow, Serialize, ToSchema, Debug)]
pub struct SafeUser {
    pub id: String,
    pub username: String,
}
