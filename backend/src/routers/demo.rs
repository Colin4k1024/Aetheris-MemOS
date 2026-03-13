use axum::extract::Query;
use axum::response::Html;
use rinja::Template;
use serde::Deserialize;

use crate::AppResult;

#[derive(Debug, Deserialize, Default)]
pub struct HelloQuery {
    pub name: Option<String>,
}

pub async fn hello(Query(query): Query<HelloQuery>) -> AppResult<Html<String>> {
    #[derive(Template)]
    #[template(path = "hello.html")]
    struct HelloTemplate<'a> {
        name: &'a str,
    }
    let hello_tmpl = HelloTemplate {
        name: query.name.as_deref().unwrap_or("World"),
    };
    let html = hello_tmpl
        .render()
        .map_err(|e| crate::AppError::Internal(e.to_string()))?;
    Ok(Html(html))
}
