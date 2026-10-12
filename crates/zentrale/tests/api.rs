//! The UI's API in-process, without Syncthing: login, CSRF, folders, matrix, pairing codes.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use stellwerk_zentrale::app::App;
use stellwerk_zentrale::auth::hash_password;
use stellwerk_zentrale::model::Hub;
use stellwerk_zentrale::server::router;
use stellwerk_zentrale::store::Store;
use tower::ServiceExt;

const PASSWORD: &str = "a long admin password";

async fn app() -> (Router, App, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("stellwerk.db")).await.unwrap();
    let hub = Hub {
        syncthing_id: "HUB".into(),
        address: "tcp://hub:22001".into(),
        folders_dir: "/data/folders".into(),
    };
    // No Syncthing behind it: the endpoints tested here do not reach the hub.
    let client = stellwerk_engine::Client::new("http://127.0.0.1:9".parse().unwrap(), "none");
    let app = App::new(store, hub, client).await.unwrap();
    (router(app.clone()), app, dir)
}

/// A browser: remembers the session cookie and the CSRF token.
#[derive(Default)]
struct Browser {
    cookie: Option<String>,
    csrf: Option<String>,
}

impl Browser {
    async fn send(
        &mut self,
        router: &Router,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(cookie) = &self.cookie {
            request = request.header(header::COOKIE, cookie);
        }
        if let Some(csrf) = &self.csrf {
            request = request.header("x-csrf-token", csrf);
        }
        let body = match body {
            Some(json) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                Body::from(json.to_string())
            }
            None => Body::empty(),
        };
        let response = router
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        if let Some(set) = response.headers().get(header::SET_COOKIE) {
            let pair = set.to_str().unwrap().split(';').next().unwrap().to_string();
            self.cookie = Some(pair);
        }
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        if let Some(csrf) = json.get("csrfToken").and_then(Value::as_str) {
            self.csrf = Some(csrf.to_string());
        }
        (status, json)
    }
}

#[tokio::test]
async fn login_needs_a_password_and_protects_everything_else() {
    let (router, app, _dir) = app().await;
    let mut browser = Browser::default();

    let (status, session) = browser.send(&router, "GET", "/api/session", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(session["loggedIn"], false);
    assert_eq!(session["passwordSet"], false);
    let (status, _) = browser
        .send(
            &router,
            "POST",
            "/api/login",
            Some(json!({ "password": PASSWORD })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    app.store()
        .set_admin_password_hash(&hash_password(PASSWORD).unwrap())
        .await
        .unwrap();
    let (status, _) = browser.send(&router, "GET", "/api/overview", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = browser
        .send(
            &router,
            "POST",
            "/api/login",
            Some(json!({ "password": "wrong password!" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, login) = browser
        .send(
            &router,
            "POST",
            "/api/login",
            Some(json!({ "password": PASSWORD })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(login["csrfToken"].is_string());
    let (status, overview) = browser.send(&router, "GET", "/api/overview", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(overview["hub"]["syncthingId"], "HUB");

    // A change without the CSRF token is refused.
    let csrf = browser.csrf.take();
    let (status, _) = browser
        .send(
            &router,
            "POST",
            "/api/folders",
            Some(json!({ "label": "Fotos" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    browser.csrf = csrf;

    let (status, _) = browser.send(&router, "POST", "/api/logout", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = browser.send(&router, "GET", "/api/overview", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn folders_drafts_and_pairing_codes() {
    let (router, app, _dir) = app().await;
    app.store()
        .set_admin_password_hash(&hash_password(PASSWORD).unwrap())
        .await
        .unwrap();
    let mut browser = Browser::default();
    browser
        .send(
            &router,
            "POST",
            "/api/login",
            Some(json!({ "password": PASSWORD })),
        )
        .await;
    let mut revisions = app.revisions();

    let (status, folder) = browser
        .send(
            &router,
            "POST",
            "/api/folders",
            Some(json!({ "label": "Fotos" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(folder["label"], "Fotos");
    assert!(
        revisions.has_changed().unwrap(),
        "agents hear about the new folder"
    );
    revisions.mark_unchanged();

    let id = folder["id"].as_str().unwrap();
    let (status, _) = browser
        .send(
            &router,
            "PATCH",
            &format!("/api/folders/{id}"),
            Some(json!({ "label": "Bilder" })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let draft =
        json!({ "changes": [{ "folderId": id, "deviceId": "nope", "mode": "sendReceive" }] });
    let (status, error) = browser
        .send(&router, "POST", "/api/shares", Some(draft))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(error["error"].is_string());

    let (status, enrollment) = browser
        .send(&router, "POST", "/api/enrollments", None)
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let code = enrollment["code"].as_str().unwrap();
    assert_eq!(code.len(), 14);
    let (status, pending) = browser
        .send(
            &router,
            "POST",
            "/api/enrollments/status",
            Some(json!({ "code": code })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pending["device"], Value::Null);

    let patterns = json!({ "ignorePatterns": ["*.tmp", "", "cache/ "] });
    let (status, _) = browser
        .send(
            &router,
            "PATCH",
            &format!("/api/folders/{id}"),
            Some(patterns),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, overview) = browser.send(&router, "GET", "/api/overview", None).await;
    assert_eq!(overview["folders"][0]["label"], "Bilder");
    assert_eq!(
        overview["folders"][0]["ignorePatterns"],
        json!(["*.tmp", "cache/"])
    );
    assert_eq!(overview["folders"][0]["versioning"]["type"], "staggered");

    let (status, _) = browser
        .send(&router, "DELETE", &format!("/api/folders/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, overview) = browser.send(&router, "GET", "/api/overview", None).await;
    assert_eq!(overview["folders"], json!([]));
}

#[tokio::test]
async fn unknown_api_paths_are_404_and_everything_else_goes_to_the_ui() {
    let (router, _app, _dir) = app().await;
    let mut browser = Browser::default();
    let (status, _) = browser.send(&router, "GET", "/api/nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // With web/dist built this is index.html, without it a hint how to build it.
    let (status, _) = browser.send(&router, "GET", "/folders/abc", None).await;
    assert!(status == StatusCode::OK || status == StatusCode::NOT_FOUND);
}
