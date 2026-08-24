use axum::extract::Query;
use axum::response::Html;
use rinja::Template;
use serde::Deserialize;

use crate::AppResult;

// Tolerate unknown query params (e.g. `?request_id=…`): a public endpoint must
// not 400 on extra params. Keeping this strict would break the cardinality-leak
// guard in `axum_routers::tests::records_completed_requests_with_the_matched_path`,
// which sends a high-cardinality `request_id` to assert it never reaches labels.
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
