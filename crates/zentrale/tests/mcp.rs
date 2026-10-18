//! The MCP server end to end, as an assistant uses it: find "Game of Thrones" on the NAS, make
//! a folder, preview and apply the shares, and the files arrive on the laptop.

mod common;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use common::{pair_agent, start_zentrale, wait_for, wait_for_file};
use serde_json::{Value, json};
use stellwerk_zentrale::auth::token_hash;
use stellwerk_zentrale::server::router;
use stellwerk_zentrale::store::{Scope, now};
use tower::ServiceExt;

/// An MCP client with a token: one JSON-RPC message per POST.
struct Client {
    router: Router,
    token: &'static str,
    next_id: u64,
}

impl Client {
    async fn post(&self, message: Value) -> (StatusCode, Value) {
        let request = Request::post("/mcp")
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream")
            .body(Body::from(message.to_string()))
            .unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let message =
            json!({ "jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params });
        let (status, answer) = self.post(message).await;
        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["id"], self.next_id);
        answer
    }

    /// A tool's result: the JSON it returned, or the text of its error.
    async fn call(&mut self, tool: &str, arguments: Value) -> Result<Value, String> {
        let answer = self
            .request(
                "tools/call",
                json!({ "name": tool, "arguments": arguments }),
            )
            .await;
        let result = &answer["result"];
        let text = result["content"][0]["text"].as_str().unwrap().to_string();
        if result["isError"] == true {
            Err(text)
        } else {
            Ok(serde_json::from_str(&text).unwrap())
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_assistant_finds_a_folder_and_syncs_it_to_another_device() {
    let tmp = tempfile::tempdir().unwrap();
    let zentrale = start_zentrale(tmp.path()).await;
    let app = zentrale.app.clone();
    let nas = pair_agent(&zentrale, tmp.path().join("nas")).await;
    let laptop = pair_agent(&zentrale, tmp.path().join("laptop")).await;
    wait_for("both devices online", || async {
        (app.online_devices().len() == 2).then_some(())
    })
    .await;

    let media = tmp.path().join("media");
    let thrones = media.join("Serien/Game.of.Thrones");
    std::fs::create_dir_all(thrones.join("S01")).unwrap();
    std::fs::write(thrones.join("S01/E01.txt"), "Winter is coming").unwrap();

    let store = app.store();
    store
        .create_api_token("reader", Scope::Read, &token_hash("stw_read"), now())
        .await
        .unwrap();
    store
        .create_api_token("claude", Scope::Manage, &token_hash("stw_manage"), now())
        .await
        .unwrap();
    let router = router(app.clone());
    let mut client = Client {
        router: router.clone(),
        token: "stw_manage",
        next_id: 0,
    };

    let init = client
        .request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "test", "version": "1" } }),
        )
        .await;
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    let (status, _) = client
        .post(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let tools = client.request("tools/list", json!({})).await;
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 7);

    let devices = client.call("list_devices", json!({})).await.unwrap();
    assert_eq!(devices.as_array().unwrap().len(), 2);

    let found = client
        .call(
            "search_device",
            json!({ "device": "nas", "query": "game of thrones",
                    "path": media.to_string_lossy() }),
        )
        .await
        .unwrap();
    let source = found["dirs"][0]["path"].as_str().unwrap().to_string();
    assert_eq!(source, thrones.to_string_lossy());

    let folder = client
        .call("create_folder", json!({ "label": "Game of Thrones" }))
        .await
        .unwrap();
    assert_eq!(folder["label"], "Game of Thrones");

    let target = tmp.path().join("laptop-files/Game of Thrones");
    let draft = json!({ "changes": [
        { "folder": "Game of Thrones", "device": "nas", "mode": "sendOnly", "path": source },
        { "folder": "Game of Thrones", "device": "laptop", "mode": "receiveOnly",
          "path": target.to_string_lossy() },
    ]});
    let before = app.store().revision().await.unwrap();
    let preview = client.call("preview_shares", draft.clone()).await.unwrap();
    let summary = preview["summary"][0].as_str().unwrap();
    assert!(
        summary.starts_with("Game of Thrones on nas: only sends at"),
        "{summary}"
    );
    let after = app.store().revision().await.unwrap();
    assert_eq!(before, after, "a preview changes nothing");

    // A read-only token can look, but not apply.
    let mut reader = Client {
        router,
        token: "stw_read",
        next_id: 0,
    };
    assert!(reader.call("preview_shares", draft.clone()).await.is_ok());
    let refused = reader
        .call("apply_shares", draft.clone())
        .await
        .unwrap_err();
    assert!(refused.contains("only read"), "{refused}");

    let applied = client.call("apply_shares", draft).await.unwrap();
    assert_eq!(applied["applied"], 2);
    wait_for_file(&target.join("S01/E01.txt"), "Winter is coming").await;

    let folders = client.call("list_folders", json!({})).await.unwrap();
    assert_eq!(folders[0]["shares"].as_array().unwrap().len(), 2);

    nas.stop().await;
    laptop.stop().await;
    zentrale.stop().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_endpoint_wants_a_token_and_speaks_json_rpc() {
    let tmp = tempfile::tempdir().unwrap();
    let zentrale = start_zentrale(tmp.path()).await;
    let router = router(zentrale.app.clone());
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });

    let anonymous = Request::post("/mcp")
        .body(Body::from(ping.to_string()))
        .unwrap();
    let response = router.clone().oneshot(anonymous).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[header::WWW_AUTHENTICATE], "Bearer");

    zentrale
        .app
        .store()
        .create_api_token("t", Scope::Read, &token_hash("stw_t"), now())
        .await
        .unwrap();
    let mut client = Client {
        router: router.clone(),
        token: "stw_t",
        next_id: 0,
    };
    assert_eq!(client.request("ping", json!({})).await["result"], json!({}));
    let unknown = client.request("resources/list", json!({})).await;
    assert_eq!(unknown["error"]["code"], -32601);
    let tool = client
        .request("tools/call", json!({ "name": "rm_rf" }))
        .await;
    assert_eq!(tool["error"]["code"], -32602);
    let offline = client
        .call("search_device", json!({ "device": "nobody", "query": "x" }))
        .await
        .unwrap_err();
    assert!(offline.contains("nobody"), "{offline}");
    let (status, _) = client.post(json!([ping])).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let stream = Request::get("/mcp")
        .header(header::AUTHORIZATION, "Bearer stw_t")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(stream).await.unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    zentrale.stop().await.unwrap();
}
