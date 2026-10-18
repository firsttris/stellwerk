//! An MCP server (https://modelcontextprotocol.io) at `/mcp`, so an assistant such as Claude
//! Code can steer Stellwerk: "sync Game of Thrones from the NAS to my laptop". Streamable HTTP
//! without streams: each POST carries one JSON-RPC message and gets one JSON answer; there are
//! no sessions and no server-sent events. Only API tokens get in, with the tools their scope
//! allows.

mod rpc;
mod schemas;
mod tools;
mod views;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};

use self::rpc::{INVALID_PARAMS, Incoming, METHOD_NOT_FOUND};
use self::tools::CallError;
use crate::api::access::token_scope;
use crate::app::App;
use crate::store::Scope;

pub const MCP_PATH: &str = "/mcp";

pub fn router() -> Router<App> {
    Router::new().route(MCP_PATH, post(message).get(no_stream).delete(no_stream))
}

/// GET would open a stream for messages from the server, DELETE end a session: this server
/// has neither.
async fn no_stream() -> StatusCode {
    StatusCode::METHOD_NOT_ALLOWED
}

async fn message(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    if !same_origin(&headers) {
        return refuse(StatusCode::FORBIDDEN, "cross-origin request");
    }
    let scope = match token_scope(&app, &headers).await {
        Ok(scope) => scope,
        Err(err) => return refuse(err.status, &err.message),
    };
    match rpc::parse(&body) {
        Err(answer) => (StatusCode::BAD_REQUEST, Json(answer)).into_response(),
        Ok(Incoming::Notification | Incoming::Response) => StatusCode::ACCEPTED.into_response(),
        Ok(Incoming::Request { id, method, params }) => {
            Json(answer(&app, scope, id, &method, params).await).into_response()
        }
    }
}

fn refuse(status: StatusCode, message: &str) -> Response {
    let challenge = [(header::WWW_AUTHENTICATE, "Bearer")];
    let body = Json(json!({ "error": message }));
    if status == StatusCode::UNAUTHORIZED {
        (status, challenge, body).into_response()
    } else {
        (status, body).into_response()
    }
}

async fn answer(app: &App, scope: Scope, id: Value, method: &str, params: Value) -> Value {
    match method {
        "initialize" => rpc::success(id, rpc::initialize(&params, tools::INSTRUCTIONS)),
        "ping" => rpc::success(id, json!({})),
        "tools/list" => rpc::success(id, tools::list(scope)),
        "tools/call" => call(app, scope, id, params).await,
        _ => rpc::failure(id, METHOD_NOT_FOUND, "method not found"),
    }
}

async fn call(app: &App, scope: Scope, id: Value, params: Value) -> Value {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return rpc::failure(id, INVALID_PARAMS, "tools/call needs a name");
    };
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);
    tracing::info!(tool = name, "MCP tool call");
    match tools::call(app, scope, name, args).await {
        Ok(result) => rpc::success(id, tool_result(&pretty(&result), false)),
        Err(CallError::Failed(message)) => rpc::success(id, tool_result(&message, true)),
        Err(CallError::Unknown(name)) => {
            rpc::failure(id, INVALID_PARAMS, &format!("unknown tool: {name}"))
        }
    }
}

fn tool_result(text: &str, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// A page on another site must not call the server through the browser (MCP "Security
/// Warning" under Streamable HTTP). This does not stop DNS rebinding, where Origin and Host
/// agree; the bearer token does, since a page cannot add it. Requests without `Origin` come
/// from programs, not pages.
fn same_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let host = |text: &str| {
        let authority = text.split_once("://").map_or(text, |(_, rest)| rest);
        let authority = authority.split('/').next().unwrap_or_default();
        authority
            .rsplit_once(':')
            .filter(|(_, port)| port.chars().all(|c| c.is_ascii_digit()))
            .map_or(authority, |(name, _)| name)
            .to_ascii_lowercase()
    };
    let forwarded = headers
        .get("x-forwarded-host")
        .or(headers.get(header::HOST));
    match (origin.to_str(), forwarded.and_then(|h| h.to_str().ok())) {
        (Ok(origin), Some(own)) => host(origin) == host(own),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        pairs
            .iter()
            .map(|(name, value)| {
                (
                    header::HeaderName::from_static(name),
                    HeaderValue::from_static(value),
                )
            })
            .collect()
    }

    #[test]
    fn only_pages_of_the_same_host_may_call() {
        assert!(same_origin(&headers(&[("host", "sync.example.org")])));
        assert!(same_origin(&headers(&[
            ("host", "sync.example.org:8080"),
            ("origin", "https://sync.example.org"),
        ])));
        assert!(same_origin(&headers(&[
            ("host", "127.0.0.1:8080"),
            ("x-forwarded-host", "sync.example.org"),
            ("origin", "https://sync.example.org"),
        ])));
        assert!(!same_origin(&headers(&[
            ("host", "sync.example.org"),
            ("origin", "https://evil.example"),
        ])));
        assert!(!same_origin(&headers(&[(
            "origin",
            "https://evil.example"
        )])));
    }

    #[test]
    fn tool_results_carry_text_and_say_whether_they_failed() {
        let failed = tool_result("the device is offline", true);
        assert_eq!(failed["isError"], true);
        assert_eq!(failed["content"][0]["type"], "text");
        assert_eq!(failed["content"][0]["text"], "the device is offline");
    }
}
