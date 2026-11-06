//! Live state for the UI: `GET /api/status` once, `/api/ws` for every change.

use std::time::Duration;

use axum::Json;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use serde::Serialize;

use super::access::Reader;
use super::error::ApiResult;
use super::session::Admin;
use crate::app::App;
use crate::live::{LiveView, view};
use crate::model::Config;

/// Changes closer together than this go to the UI as one update.
const THROTTLE: Duration = Duration::from_millis(300);

pub async fn status(State(app): State<App>, _: Reader) -> ApiResult<Json<LiveView>> {
    let config = app.store().config().await?;
    Ok(Json(live_view(&app, &config)))
}

fn live_view(app: &App, config: &Config) -> LiveView {
    LiveView {
        waiting: app.waiting(),
        ..view(config, &app.live(), &app.conflicts())
    }
}

/// One push to the UI: the live state, plus what tells it to reload the overview.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum UiMessage {
    #[serde(rename_all = "camelCase")]
    Update {
        revision: u64,
        online: Vec<String>,
        status: LiveView,
    },
}

pub async fn ws(State(app): State<App>, admin: Admin, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| session(app, admin, socket))
}

/// Streams updates while the login lasts: after a logout, expiry or new password the next
/// update closes the socket instead.
async fn session(app: App, admin: Admin, mut socket: WebSocket) {
    let mut ticks = app.ticks();
    ticks.mark_changed();
    loop {
        tokio::select! {
            changed = ticks.changed() => {
                if changed.is_err() {
                    return;
                }
                tokio::time::sleep(THROTTLE).await;
                ticks.borrow_and_update();
                if !admin.still_logged_in(&app).await {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
                let Ok(message) = update(&app).await else { return };
                if socket.send(Message::Text(message.into())).await.is_err() {
                    return;
                }
            }
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_)) | Err(_)) | None => return,
                Some(Ok(_)) => {}
            },
        }
    }
}

async fn update(app: &App) -> ApiResult<String> {
    let config = app.store().config().await?;
    let message = UiMessage::Update {
        revision: config.revision,
        online: app.online_devices().into_keys().collect(),
        status: live_view(app, &config),
    };
    Ok(serde_json::to_string(&message).expect("UI messages serialize"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// The contract with `web/src/api/types.ts`.
    #[test]
    fn an_update_has_the_shape_the_ui_parses() {
        let message = UiMessage::Update {
            revision: 3,
            online: vec!["laptop".into()],
            status: LiveView::default(),
        };
        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            json!({
                "type": "update",
                "revision": 3,
                "online": ["laptop"],
                "status": {
                    "devices": {},
                    "hub": { "folders": {}, "completion": {} },
                    "conflicts": {},
                    "waiting": [],
                },
            })
        );
    }
}
