use rinja::Template;
use salvo::prelude::*;

use crate::AppResult;

#[handler]
pub async fn hello(req: &mut Request) -> AppResult<Text<String>> {
    #[derive(Template)]
    #[template(path = "hello.html")]
    struct HelloTemplate<'a> {
        name: &'a str,
    }
    let hello_tmpl = HelloTemplate {
        name: req.query::<&str>("name").unwrap_or("World"),
    };
    let html = hello_tmpl
        .render()
        .map_err(|e| crate::AppError::Internal(e.to_string()))?;
    Ok(Text::Html(html))
}