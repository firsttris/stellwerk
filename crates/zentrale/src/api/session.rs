//! Login for the one admin: session cookie (HttpOnly, SameSite=Strict) plus a CSRF token the
//! UI sends back in `X-CSRF-Token` on every change.

use std::time::Duration;

use axum::Json;
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use super::error::{ApiError, ApiResult};
use crate::app::App;
use crate::auth::{new_token, same_token, token_hash, verify_password};
use crate::store::{Session, now};

const COOKIE: &str = "stellwerk_session";
const CSRF_HEADER: &str = "x-csrf-token";
const SESSION_LIFETIME_SECS: i64 = 30 * 24 * 60 * 60;
/// Slows down guessing; argon2 already takes a moment per try.
const FAILED_LOGIN_DELAY: Duration = Duration::from_millis(500);

/// A request from the logged-in admin; changes also carry the right CSRF token.
#[derive(Debug, Clone)]
pub struct Admin;

impl FromRequestParts<App> for Admin {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, app: &App) -> Result<Self, Self::Rejection> {
        let session = current_session(&parts.headers, app)
            .await?
            .ok_or_else(ApiError::unauthorized)?;
        let safe = matches!(parts.method, Method::GET | Method::HEAD);
        let csrf = parts
            .headers
            .get(CSRF_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !safe && !same_token(csrf, &session.csrf_token) {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "missing or wrong CSRF token",
            ));
        }
        Ok(Admin)
    }
}

async fn current_session(headers: &HeaderMap, app: &App) -> ApiResult<Option<Session>> {
    let Some(token) = cookie(headers, COOKIE) else {
        return Ok(None);
    };
    Ok(app.store().session(&token_hash(&token), now()).await?)
}

/// The value of one cookie from the `Cookie` headers.
fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_string())
}

/// `Secure` only behind HTTPS, so a first look over plain HTTP in the LAN still works.
fn session_cookie(headers: &HeaderMap, value: &str, max_age: i64) -> String {
    let https = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|proto| proto.eq_ignore_ascii_case("https"));
    let secure = if https { "; Secure" } else { "" };
    format!("{COOKIE}={value}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}{secure}")
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    logged_in: bool,
    password_set: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    csrf_token: Option<String>,
}

pub async fn info(State(app): State<App>, headers: HeaderMap) -> ApiResult<Json<SessionInfo>> {
    let session = current_session(&headers, &app).await?;
    Ok(Json(SessionInfo {
        logged_in: session.is_some(),
        password_set: app.store().admin_password_hash().await?.is_some(),
        csrf_token: session.map(|s| s.csrf_token),
    }))
}

#[derive(Debug, Deserialize)]
pub struct Login {
    password: String,
}

pub async fn login(
    State(app): State<App>,
    headers: HeaderMap,
    Json(login): Json<Login>,
) -> ApiResult<Response> {
    let hash = app.store().admin_password_hash().await?.ok_or_else(|| {
        ApiError::new(
            StatusCode::CONFLICT,
            "no password set yet; run `stellwerk set-password` on the server",
        )
    })?;
    let password = login.password;
    let valid = tokio::task::spawn_blocking(move || verify_password(&password, &hash))
        .await
        .unwrap_or(false);
    if !valid {
        tokio::time::sleep(FAILED_LOGIN_DELAY).await;
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "wrong password"));
    }
    let token = new_token();
    let session = Session {
        token_hash: token_hash(&token),
        csrf_token: new_token(),
        expires_at: now() + SESSION_LIFETIME_SECS,
    };
    app.store().create_session(&session).await?;
    let body = SessionInfo {
        logged_in: true,
        password_set: true,
        csrf_token: Some(session.csrf_token),
    };
    Ok((
        [(
            header::SET_COOKIE,
            session_cookie(&headers, &token, SESSION_LIFETIME_SECS),
        )],
        Json(body),
    )
        .into_response())
}

pub async fn logout(State(app): State<App>, _: Admin, headers: HeaderMap) -> ApiResult<Response> {
    if let Some(token) = cookie(&headers, COOKIE) {
        app.store().delete_session(&token_hash(&token)).await?;
    }
    Ok((
        [(header::SET_COOKIE, session_cookie(&headers, "", 0))],
        StatusCode::NO_CONTENT,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn finds_a_cookie_among_others() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("a=1; stellwerk_session=tok; b=2"),
        );
        assert_eq!(cookie(&headers, COOKIE).as_deref(), Some("tok"));
        assert_eq!(cookie(&headers, "missing"), None);
    }

    #[test]
    fn the_cookie_is_secure_only_behind_https() {
        let mut headers = HeaderMap::new();
        assert!(!session_cookie(&headers, "t", 10).contains("Secure"));
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        let cookie = session_cookie(&headers, "t", 10);
        assert!(cookie.contains("HttpOnly; SameSite=Strict"));
        assert!(cookie.ends_with("; Secure"));
    }
}
