//! Who may call an endpoint: the admin's browser session may do everything; an API token
//! (`Authorization: Bearer stw_…`) only what its scope allows. A token is not sent by the
//! browser on its own, so it needs no CSRF token.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode, header};

use super::error::{ApiError, ApiResult};
use super::session::Admin;
use crate::app::App;
use crate::auth::{new_token, token_hash};
use crate::store::{Scope, now};

/// Every API token starts with this, so it is recognised in a config file or a leak scan.
pub const TOKEN_PREFIX: &str = "stw_";

/// A new API token; only its hash is stored.
pub fn new_api_token() -> String {
    format!("{TOKEN_PREFIX}{}", new_token())
}

/// The token from `Authorization: Bearer …`, if the request carries one.
pub fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (kind, token) = value.split_once(' ')?;
    kind.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

/// What the caller may do.
async fn caller_scope(parts: &mut Parts, app: &App) -> ApiResult<Scope> {
    let Some(token) = bearer(&parts.headers) else {
        Admin::from_request_parts(parts, app).await?;
        return Ok(Scope::Manage);
    };
    let token = app
        .store()
        .use_api_token(&token_hash(token), now())
        .await?
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "unknown or revoked API token"))?;
    Ok(token.scope)
}

/// May read: the admin, or any API token.
#[derive(Debug, Clone, Copy)]
pub struct Reader;

impl FromRequestParts<App> for Reader {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, app: &App) -> Result<Self, Self::Rejection> {
        caller_scope(parts, app).await?;
        Ok(Self)
    }
}

/// May change folders and shares: the admin, or an API token with the scope "manage".
#[derive(Debug, Clone, Copy)]
pub struct Manager;

impl FromRequestParts<App> for Manager {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, app: &App) -> Result<Self, Self::Rejection> {
        match caller_scope(parts, app).await? {
            Scope::Manage => Ok(Self),
            Scope::Read => Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "this API token may only read",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn reads_the_bearer_token() {
        let mut headers = HeaderMap::new();
        assert_eq!(bearer(&headers), None);
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Basic abc"));
        assert_eq!(bearer(&headers), None);
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("bearer stw_abc"),
        );
        assert_eq!(bearer(&headers), Some("stw_abc"));
    }

    #[test]
    fn new_tokens_carry_the_prefix_and_differ() {
        let token = new_api_token();
        assert!(token.starts_with(TOKEN_PREFIX));
        assert_ne!(token, new_api_token());
    }
}
