use serde::{Deserialize, Serialize};
use sqlx::prelude::*;
use utoipa::ToSchema;

pub mod agent;
pub mod agent_equip;
pub mod belief;
pub mod belief_record;
pub mod distillation;
pub mod dry_run;
pub mod evidence;
pub mod memory;
pub mod memory_enums;
pub mod memory_event;
pub mod performance;
pub mod principal;
pub mod procedural;
pub mod resource;
pub mod skill;
pub mod task;
pub mod validation;

pub use agent_equip::*;
pub use dry_run::*;
pub use evidence::*;
pub use memory::*;
pub use performance::*;
pub use resource::*;
pub use skill::*;
pub use task::*;
pub use validation::*;

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
