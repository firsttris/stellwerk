//! API tokens: listed, created and revoked by the admin in the web UI only.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use super::access::new_api_token;
use super::error::ApiResult;
use super::session::Admin;
use crate::app::App;
use crate::auth::token_hash;
use crate::store::{ApiToken, Scope, now};

pub async fn list(State(app): State<App>, _: Admin) -> ApiResult<Json<Vec<ApiToken>>> {
    Ok(Json(app.store().api_tokens().await?))
}

#[derive(Debug, Deserialize)]
pub struct NewToken {
    name: String,
    scope: Scope,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedToken {
    #[serde(flatten)]
    token: ApiToken,
    /// The token itself, shown this once.
    secret: String,
}

pub async fn create(
    State(app): State<App>,
    _: Admin,
    Json(body): Json<NewToken>,
) -> ApiResult<(StatusCode, Json<CreatedToken>)> {
    let secret = new_api_token();
    let token = app
        .store()
        .create_api_token(&body.name, body.scope, &token_hash(&secret), now())
        .await?;
    Ok((StatusCode::CREATED, Json(CreatedToken { token, secret })))
}

pub async fn delete(
    State(app): State<App>,
    _: Admin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    app.store().delete_api_token(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}
