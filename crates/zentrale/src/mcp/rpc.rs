//! JSON-RPC 2.0 as MCP uses it (https://modelcontextprotocol.io/specification, "Base
//! Protocol" and "Lifecycle"): reading a message, building answers, agreeing on a version.

use serde::Deserialize;
use serde_json::{Value, json};

/// The protocol versions this server speaks, newest first.
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;

/// One message from the client.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// Wants an answer under `id`.
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// Wants none, e.g. `notifications/initialized`.
    Notification,
    /// The client answering a request of ours; this server sends none.
    Response,
}

#[derive(Deserialize)]
struct Message {
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: Option<String>,
    #[serde(default)]
    params: Value,
}

/// Reads a POST body; a malformed one gives the error answer to send back.
pub fn parse(body: &[u8]) -> Result<Incoming, Value> {
    let value: Value =
        serde_json::from_slice(body).map_err(|_| failure(Value::Null, PARSE_ERROR, "not JSON"))?;
    if value.is_array() {
        // Batches were dropped from the protocol in 2025-06-18.
        return Err(failure(
            Value::Null,
            INVALID_REQUEST,
            "batches are not supported",
        ));
    }
    let message: Message = serde_json::from_value(value)
        .map_err(|_| failure(Value::Null, INVALID_REQUEST, "not a JSON-RPC message"))?;
    if message.jsonrpc.as_deref() != Some("2.0") {
        return Err(failure(
            message.id.unwrap_or(Value::Null),
            INVALID_REQUEST,
            "jsonrpc must be \"2.0\"",
        ));
    }
    Ok(match (message.id, message.method) {
        (Some(id), Some(method)) => Incoming::Request {
            id,
            method,
            params: message.params,
        },
        (None, Some(_)) => Incoming::Notification,
        (_, None) => Incoming::Response,
    })
}

pub fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub fn failure(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// The client's version if this server speaks it, else the newest this server speaks; the
/// client then decides whether to go on.
pub fn negotiate(requested: Option<&str>) -> &'static str {
    PROTOCOL_VERSIONS
        .iter()
        .find(|version| Some(**version) == requested)
        .unwrap_or(&PROTOCOL_VERSIONS[0])
}

/// The answer to `initialize`.
pub fn initialize(params: &Value, instructions: &str) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    json!({
        "protocolVersion": negotiate(requested),
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "stellwerk",
            "title": "Stellwerk",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": instructions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_requests_notifications_and_responses_apart() {
        let request = parse(br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
        assert_eq!(
            request,
            Incoming::Request {
                id: json!(1),
                method: "tools/list".into(),
                params: Value::Null,
            }
        );
        let notification =
            parse(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).unwrap();
        assert_eq!(notification, Incoming::Notification);
        let response = parse(br#"{"jsonrpc":"2.0","id":"a","result":{}}"#).unwrap();
        assert_eq!(response, Incoming::Response);
    }

    #[test]
    fn malformed_messages_get_an_error_answer() {
        assert_eq!(parse(b"{").unwrap_err()["error"]["code"], PARSE_ERROR);
        assert_eq!(parse(b"[]").unwrap_err()["error"]["code"], INVALID_REQUEST);
        assert_eq!(parse(b"7").unwrap_err()["error"]["code"], INVALID_REQUEST);
        let old = parse(br#"{"jsonrpc":"1.0","id":3,"method":"ping"}"#).unwrap_err();
        assert_eq!(old["error"]["code"], INVALID_REQUEST);
        assert_eq!(old["id"], 3);
    }

    #[test]
    fn agrees_on_a_version_the_server_speaks() {
        assert_eq!(negotiate(Some("2025-03-26")), "2025-03-26");
        assert_eq!(negotiate(Some("2099-01-01")), PROTOCOL_VERSIONS[0]);
        assert_eq!(negotiate(None), PROTOCOL_VERSIONS[0]);
        let result = initialize(&json!({ "protocolVersion": "2025-06-18" }), "hi");
        assert_eq!(result["protocolVersion"], "2025-06-18");
        assert_eq!(result["serverInfo"]["name"], "stellwerk");
        assert_eq!(result["instructions"], "hi");
        assert!(result["capabilities"]["tools"].is_object());
    }
}
