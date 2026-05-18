use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

pub mod agent;
pub mod axum_routers;
pub mod config;
pub mod db;
pub mod distributed;
pub mod error;
pub mod hoops;
pub mod integrations;
pub mod kernel;
pub mod layers;
pub mod mcp;
pub mod models;
pub mod otel;
pub mod providers;
pub mod protocol;
pub mod routers;
pub mod runtime;
pub mod services;
pub mod tenant;
pub mod utils;
pub mod web;

pub use error::AppError;
pub use models::*;

pub type AppResult<T> = Result<T, AppError>;
pub type JsonResult<T> = Result<Json<T>, AppError>;
pub type EmptyResult = JsonResult<Empty>;

pub fn json_ok<T>(data: T) -> JsonResult<T> {
    Ok(Json(data))
}

#[derive(Serialize, ToSchema, Clone, Copy, Debug)]
pub struct Empty {}

pub fn empty_ok() -> JsonResult<Empty> {
    Ok(Json(Empty {}))
}
