use cookie::Cookie;
use rinja::Template;
use salvo::oapi::extract::*;
use salvo::prelude::*;
use salvo::jwt_auth::JwtAuthState;
use serde::{Deserialize, Serialize};

use crate::hoops::jwt;
use crate::models::User;
use crate::{db, json_ok, utils, AppResult, JsonResult};

#[handler]
pub async fn login_page(res: &mut Response) -> AppResult<()> {
    #[derive(Template)]
    #[template(path = "login.html")]
    struct LoginTemplate {}
    if let Some(cookie) = res.cookies().get("jwt_token") {
        let token = cookie.value().to_string();
        if jwt::decode_token(&token) {
            res.render(Redirect::other("/users"));
            return Ok(());
        }
    }
    let hello_tmpl = LoginTemplate {};
    res.render(Text::Html(hello_tmpl.render().unwrap()));
    Ok(())
}

#[derive(Deserialize, ToSchema, Default, Debug)]
pub struct LoginInData {
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
#[endpoint(tags("auth"))]
pub async fn post_login(
    idata: JsonBody<LoginInData>,
    res: &mut Response,
) -> JsonResult<LoginOutData> {
    let idata = idata.into_inner();
    let conn = db::pool();
    let Some(User {
        id,
        username,
        password,
    }) = sqlx::query_as!(
        User,
        r#"
            SELECT id, username, password FROM users
            WHERE username = $1
            "#,
        idata.username
    )
    .fetch_optional(conn)
    .await?
    else {
        return Err(StatusError::unauthorized()
            .brief("User does not exist.")
            .into());
    };

    if utils::verify_password(&idata.password, &password).is_err()
    {
        return Err(StatusError::unauthorized()
            .brief("Account not exist or password is incorrect.")
            .into());
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
        .same_site(cookie::SameSite::None)
        .build();
    res.add_cookie(cookie);
    json_ok(odata)
}

#[endpoint(tags("auth"))]
pub async fn post_login_with_token(
    req: &mut Request,
    res: &mut Response,
) -> JsonResult<LoginOutData> {
    // 首先检查是否有查询参数 token
    if let Some(token) = req.query::<&str>("token") {
        if !token.is_empty() {
            // 如果有 token，验证 token
            let valid = jwt::decode_token(token);
            if !valid {
                return Err(StatusError::unauthorized()
                    .brief("Token is invalid or expired.")
                    .into());
            }
            
            // Token 有效，设置 cookie 并返回成功
            // 注意：这里无法从 token 中提取用户信息，所以返回一个简化的响应
            // 如果需要完整的用户信息，需要解码 token
            let cookie = Cookie::build(("jwt_token", token.to_string()))
                .path("/")
                .http_only(true)
                .same_site(cookie::SameSite::None)
                .build();
            res.add_cookie(cookie);
            
            // 返回一个简化的登录响应
            // 实际应用中应该从 token 中解析用户信息
            return json_ok(LoginOutData {
                id: "".to_string(),
                username: "".to_string(),
                token: token.to_string(),
                exp: 0,
            });
        }
    }
    
    // 如果没有 token 查询参数，尝试从 JSON body 获取用户名和密码
    let idata: LoginInData = req.parse_json().await
        .map_err(|_| StatusError::bad_request().brief("Invalid request body. Expected JSON with username and password, or provide token as query parameter."))?;
    
    let conn = db::pool();
    let Some(User {
        id,
        username,
        password,
    }) = sqlx::query_as!(
        User,
        r#"
            SELECT id, username, password FROM users
            WHERE username = $1
            "#,
        idata.username
    )
    .fetch_optional(conn)
    .await?
    else {
        return Err(StatusError::unauthorized()
            .brief("User does not exist.")
            .into());
    };

    if utils::verify_password(&idata.password, &password).is_err()
    {
        return Err(StatusError::unauthorized()
            .brief("Account not exist or password is incorrect.")
            .into());
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
        .same_site(cookie::SameSite::None)
        .build();
    res.add_cookie(cookie);
    json_ok(odata)
}


#[derive(Serialize, ToSchema, Default, Debug)]
pub struct TokenVerifyResponse {
    pub valid: bool,
    pub message: String,
}

#[endpoint(tags("auth"))]
pub async fn get_login_with_token(
    req: &mut Request,
    res: &mut Response,
) -> JsonResult<TokenVerifyResponse> {
    // 从查询参数获取 token
    let token = req.query::<&str>("token").map(|s| s.to_string());
    
    // 如果没有 token，返回错误
    let token = match token {
        Some(t) if !t.is_empty() => t,
        _ => {
            return json_ok(TokenVerifyResponse {
                valid: false,
                message: "Token parameter is required".to_string(),
            });
        }
    };

    // 验证 token
    let valid = jwt::decode_token(&token);
    
    if valid {
        // 如果 token 有效，设置 cookie
        let cookie = Cookie::build(("jwt_token", token.clone()))
            .path("/")
            .http_only(true)
            .same_site(cookie::SameSite::None)
            .build();
        res.add_cookie(cookie);
        
        json_ok(TokenVerifyResponse {
            valid: true,
            message: "Token is valid".to_string(),
        })
    } else {
        json_ok(TokenVerifyResponse {
            valid: false,
            message: "Token is invalid or expired".to_string(),
        })
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
    pub notifyCount: Option<u32>,
    pub unreadCount: Option<u32>,
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

#[endpoint(tags("auth"))]
pub async fn get_current_user(
    depot: &mut Depot,
) -> JsonResult<CurrentUserResponse> {
    // 检查认证状态
    match depot.jwt_auth_state() {
        JwtAuthState::Authorized => {
            // 获取 JWT claims
            let jwt_data = depot.jwt_auth_data::<jwt::JwtClaims>()
                .ok_or_else(|| StatusError::unauthorized().brief("Not authenticated"))?;
            
            let user_id = &jwt_data.claims.uid;
            let conn = db::pool();
            
            // 从数据库查询用户信息
            let user = sqlx::query_as!(
                User,
                r#"
                    SELECT id, username, password FROM users
                    WHERE id = $1
                    "#,
                user_id
            )
            .fetch_optional(conn)
            .await?
            .ok_or_else(|| StatusError::not_found().brief("User not found"))?;
            
            // 构建返回数据
            json_ok(CurrentUserResponse {
                name: user.username.clone(),
                avatar: None,
                userid: user.id.clone(),
                email: None,
                signature: None,
                title: None,
                group: None,
                tags: None,
                notifyCount: Some(0),
                unreadCount: Some(0),
                country: None,
                access: Some("admin".to_string()), // 默认设置为 admin
                geographic: None,
                address: None,
                phone: None,
            })
        }
        JwtAuthState::Unauthorized => {
            Err(StatusError::unauthorized().brief("Not authenticated").into())
        }
        JwtAuthState::Forbidden => {
            Err(StatusError::forbidden().brief("Access forbidden").into())
        }
    }
}
