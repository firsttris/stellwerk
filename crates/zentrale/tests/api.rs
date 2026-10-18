//! The UI's API in-process, without Syncthing: login, CSRF, folders, matrix, pairing codes.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use stellwerk_zentrale::app::App;
use stellwerk_zentrale::auth::hash_password;
use stellwerk_zentrale::model::{DeviceKind, Hub};
use stellwerk_zentrale::server::router;
use stellwerk_zentrale::store::{NewDevice, Store, now};
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

/// A browser logged in as the admin.
async fn logged_in(router: &Router, app: &App) -> Browser {
    app.store()
        .set_admin_password_hash(&hash_password(PASSWORD).unwrap())
        .await
        .unwrap();
    let mut browser = Browser::default();
    let (status, _) = browser
        .send(
            router,
            "POST",
            "/api/login",
            Some(json!({ "password": PASSWORD })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    browser
}

/// A browser: remembers the session cookie and the CSRF token. With `bearer` it is a script
/// that sends an API token instead.
#[derive(Default)]
struct Browser {
    cookie: Option<String>,
    csrf: Option<String>,
    bearer: Option<String>,
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
        if let Some(token) = &self.bearer {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
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
    assert_eq!(overview["version"], env!("CARGO_PKG_VERSION"));

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
async fn the_folder_picker_needs_a_known_device_that_is_online() {
    let (router, app, _dir) = app().await;
    let mut browser = logged_in(&router, &app).await;
    let (status, _) = browser
        .send(&router, "GET", "/api/devices/nope/browse?path=/", None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let device = app
        .store()
        .add_device(
            NewDevice {
                name: "laptop".into(),
                kind: DeviceKind::Laptop,
                os: "linux".into(),
                public_key: "key".into(),
                syncthing_id: "LAPTOP".into(),
                agent_version: "0.1.6".into(),
                syncthing_version: "v2".into(),
            },
            now(),
        )
        .await
        .unwrap();
    let (status, error) = browser
        .send(
            &router,
            "GET",
            &format!("/api/devices/{}/browse", device.id),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["error"], "the device is offline");
}

#[tokio::test]
async fn unknown_api_paths_are_404_and_everything_else_goes_to_the_ui() {
    let (router, _app, _dir) = app().await;
    let mut browser = Browser::default();
    for path in ["/api/nope", "/api"] {
        let (status, _) = browser.send(&router, "GET", path, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    // With web/dist built this is index.html, without it a hint how to build it.
    let (status, _) = browser.send(&router, "GET", "/folders/abc", None).await;
    assert!(status == StatusCode::OK || status == StatusCode::NOT_FOUND);
}

/// A Zentrale behind a private CA serves it without login and puts its fingerprint, with the
/// release to fetch the agent from, into every pairing code; without one, neither.
#[tokio::test]
async fn the_private_ca_is_served_and_named_in_new_codes() {
    let (router, app, _dir) = app().await;
    let mut browser = logged_in(&router, &app).await;
    let get_ca = || async {
        let response = router
            .clone()
            .oneshot(Request::get("/ca.crt").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    };

    assert_eq!(get_ca().await.0, StatusCode::NOT_FOUND);
    let (_, enrollment) = browser
        .send(&router, "POST", "/api/enrollments", None)
        .await;
    assert_eq!(enrollment["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(enrollment["caSha256"], Value::Null);

    let key = rcgen::KeyPair::generate().unwrap();
    let pem = rcgen::CertificateParams::new(Vec::<String>::new())
        .unwrap()
        .self_signed(&key)
        .unwrap()
        .pem();
    let ca = stellwerk_zentrale::ca::from_pem(pem.clone()).unwrap();
    app.set_ca(ca.clone());
    assert_eq!(get_ca().await, (StatusCode::OK, pem));
    let (_, enrollment) = browser
        .send(&router, "POST", "/api/enrollments", None)
        .await;
    assert_eq!(enrollment["caSha256"], ca.sha256.as_str());
}

#[tokio::test]
async fn the_live_socket_closes_once_the_session_is_gone() {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let (router, app, _dir) = app().await;
    app.store()
        .set_admin_password_hash(&hash_password(PASSWORD).unwrap())
        .await
        .unwrap();
    let mut browser = Browser::default();
    let login = json!({ "password": PASSWORD });
    browser
        .send(&router, "POST", "/api/login", Some(login))
        .await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, router.clone()).into_future());

    let mut request = format!("ws://{addr}/api/ws").into_client_request().unwrap();
    let cookie = browser.cookie.clone().unwrap();
    request
        .headers_mut()
        .insert(header::COOKIE, cookie.parse().unwrap());
    let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let first = socket.next().await.unwrap().unwrap();
    assert!(matches!(first, Message::Text(_)), "{first:?}");

    let (status, _) = browser.send(&router, "POST", "/api/logout", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    app.tick();
    let next = tokio::time::timeout(std::time::Duration::from_secs(5), socket.next())
        .await
        .expect("the socket closes");
    assert!(
        matches!(next, None | Some(Ok(Message::Close(_)) | Err(_))),
        "{next:?}"
    );
}

#[tokio::test]
async fn parallel_wrong_logins_are_checked_one_after_another() {
    let (router, app, _dir) = app().await;
    app.store()
        .set_admin_password_hash(&hash_password(PASSWORD).unwrap())
        .await
        .unwrap();
    let guess = || {
        let router = router.clone();
        async move {
            let wrong = json!({ "password": "wrong password!" });
            Browser::default()
                .send(&router, "POST", "/api/login", Some(wrong))
                .await
                .0
        }
    };

    let started = std::time::Instant::now();
    let (a, b) = tokio::join!(guess(), guess());
    assert_eq!((a, b), (StatusCode::UNAUTHORIZED, StatusCode::UNAUTHORIZED));
    // 0.5 s after the first failure, then 1 s after the second, one after the other.
    assert!(started.elapsed() >= std::time::Duration::from_millis(1500));

    let (status, _) = Browser::default()
        .send(
            &router,
            "POST",
            "/api/login",
            Some(json!({ "password": PASSWORD })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

/// A script with an API token of the given scope, created by the logged-in admin.
async fn script(router: &Router, admin: &mut Browser, scope: &str) -> (Browser, String) {
    let (status, created) = admin
        .send(
            router,
            "POST",
            "/api/tokens",
            Some(json!({ "name": format!("{scope} script"), "scope": scope })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let secret = created["secret"].as_str().unwrap();
    assert!(secret.starts_with("stw_"));
    let script = Browser {
        bearer: Some(secret.to_string()),
        ..Browser::default()
    };
    (script, created["id"].as_str().unwrap().to_string())
}

#[tokio::test]
async fn api_tokens_may_do_what_their_scope_allows() {
    let (router, app, _dir) = app().await;
    let mut admin = logged_in(&router, &app).await;
    let (mut reader, reader_id) = script(&router, &mut admin, "read").await;
    let (mut manager, _) = script(&router, &mut admin, "manage").await;

    // Both read, without a CSRF token.
    for script in [&mut reader, &mut manager] {
        let (status, _) = script.send(&router, "GET", "/api/overview", None).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = script.send(&router, "GET", "/api/status", None).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Only "manage" changes folders and shares.
    let new_folder = Some(json!({ "label": "Serien" }));
    let (status, _) = reader
        .send(&router, "POST", "/api/folders", new_folder.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, folder) = manager
        .send(&router, "POST", "/api/folders", new_folder)
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = manager
        .send(
            &router,
            "POST",
            "/api/shares",
            Some(json!({ "changes": [] })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // Searching is reading; it needs a name and a known device.
    let (status, _) = reader
        .send(&router, "GET", "/api/devices/unknown/search?q=got", None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = reader
        .send(&router, "GET", "/api/devices/unknown/search?q=%20", None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Removing, pairing and the tokens themselves need the browser.
    let folder_uri = format!("/api/folders/{}", folder["id"].as_str().unwrap());
    let (status, _) = manager.send(&router, "DELETE", &folder_uri, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = manager
        .send(&router, "POST", "/api/enrollments", None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = manager.send(&router, "GET", "/api/tokens", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The list shows when a token was last used, never the token.
    let (status, tokens) = admin.send(&router, "GET", "/api/tokens", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tokens.as_array().unwrap().len(), 2);
    assert!(tokens[0]["lastUsedAt"].is_number());
    assert!(tokens[0].get("secret").is_none());

    // A revoked token is unknown at once.
    let (status, _) = admin
        .send(&router, "DELETE", &format!("/api/tokens/{reader_id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = reader.send(&router, "GET", "/api/overview", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let mut unknown = Browser {
        bearer: Some("stw_unknown".into()),
        ..Browser::default()
    };
    let (status, _) = unknown.send(&router, "GET", "/api/overview", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
