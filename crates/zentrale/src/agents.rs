//! The agents' WebSocket: challenge, signed hello, then the desired state on every change.

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use stellwerk_protocol::messages::{AgentMessage, Applied, Hello, ServerMessage};

use crate::app::App;
use crate::desired;
use crate::model::{Device, DeviceKind};
use crate::pairing::{code_hash, new_nonce, verify_hello};
use crate::store::{NewDevice, now};

/// How long an agent has to answer the challenge.
const HELLO_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn ws(State(app): State<App>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| session(app, socket))
}

async fn session(app: App, mut socket: WebSocket) {
    let nonce = new_nonce();
    if send(
        &mut socket,
        &ServerMessage::Challenge {
            nonce: nonce.clone(),
        },
    )
    .await
    .is_err()
    {
        return;
    }
    let hello = match tokio::time::timeout(HELLO_TIMEOUT, receive(&mut socket)).await {
        Ok(Some(AgentMessage::Hello(hello))) => hello,
        _ => return reject(socket, "expected a hello").await,
    };
    let device = match authenticate(&app, &nonce, &hello).await {
        Ok(device) => device,
        Err(reason) => return reject(socket, &reason).await,
    };

    let connection = app.set_online(&device.id, now());
    tracing::info!(device = %device.name, "agent connected");
    let welcome = ServerMessage::Welcome {
        device: device.id.clone(),
    };
    if send(&mut socket, &welcome).await.is_ok() {
        serve(&app, &device.id, &mut socket).await;
    }
    app.set_offline(&device.id, connection);
    let _ = app.store().touch_device(&device.id, now()).await;
    tracing::info!(device = %device.name, "agent disconnected");
}

/// Sends the desired state now and after every change, records what the agent applied.
async fn serve(app: &App, device_id: &str, socket: &mut WebSocket) {
    let mut revisions = app.revisions();
    revisions.mark_changed();
    loop {
        tokio::select! {
            changed = revisions.changed() => {
                if changed.is_err() {
                    return;
                }
                let Ok(config) = app.store().config().await else { return };
                let Some(state) = desired::for_device(&config, app.hub(), device_id) else {
                    let _ = send(socket, &ServerMessage::Rejected {
                        reason: "this device is blocked or was removed".into(),
                    }).await;
                    return;
                };
                if send(socket, &ServerMessage::DesiredState(state)).await.is_err() {
                    return;
                }
            }
            message = receive(socket) => match message {
                Some(AgentMessage::Applied(applied)) => record_applied(app, device_id, applied).await,
                Some(AgentMessage::Hello(_)) => {}
                None => return,
            },
        }
    }
}

async fn record_applied(app: &App, device_id: &str, applied: Applied) {
    let at = now();
    if let Err(err) = app
        .store()
        .set_applied(device_id, applied.revision, at)
        .await
    {
        tracing::warn!(%err, "could not record applied revision");
    }
    for error in &applied.errors {
        let text = match &error.folder {
            Some(folder) => format!("Folder {folder}: {}", error.message),
            None => error.message.clone(),
        };
        let _ = app
            .store()
            .add_event(at, Some(device_id), "error", &text)
            .await;
    }
}

/// The device for a verified hello: enrolled now with its code, or known by its key.
async fn authenticate(app: &App, nonce: &str, hello: &Hello) -> Result<Device, String> {
    verify_hello(&hello.public_key, &hello.signature, nonce).map_err(|err| err.to_string())?;
    let report = report(hello);
    let store = app.store();
    let at = now();

    let enrolled = match &hello.enrollment_code {
        Some(code) => store
            .enroll(&code_hash(code), report.clone(), at)
            .await
            .map_err(|err| err.to_string())?,
        None => None,
    };
    let device = match enrolled {
        Some(device) => {
            let text = format!("{} was paired", device.name);
            let _ = store.add_event(at, Some(&device.id), "paired", &text).await;
            device
        }
        // An agent that was paired but lost the answer still has its key: let it in.
        None => store
            .device_by_key(&hello.public_key)
            .await
            .map_err(|err| err.to_string())?
            .ok_or_else(|| match hello.enrollment_code {
                Some(_) => "the pairing code is unknown, expired or used".to_string(),
                None => "this device is not paired".to_string(),
            })?,
    };
    if !device.approved {
        return Err("this device is blocked".into());
    }
    store
        .device_seen(&device.id, &report, at)
        .await
        .map_err(|err| err.to_string())?;
    app.changed().await.map_err(|err| err.to_string())?;
    Ok(device)
}

fn report(hello: &Hello) -> NewDevice {
    NewDevice {
        name: hello.name.clone(),
        kind: match hello.os.as_str() {
            "android" | "ios" => DeviceKind::Phone,
            _ => DeviceKind::Desktop,
        },
        os: hello.os.clone(),
        public_key: hello.public_key.clone(),
        syncthing_id: hello.syncthing_id.to_string(),
        agent_version: hello.agent_version.clone(),
        syncthing_version: hello.syncthing_version.clone(),
    }
}

async fn reject(mut socket: WebSocket, reason: &str) {
    tracing::info!(reason, "agent rejected");
    let _ = send(
        &mut socket,
        &ServerMessage::Rejected {
            reason: reason.to_string(),
        },
    )
    .await;
    let _ = socket.send(Message::Close(None)).await;
}

async fn send(socket: &mut WebSocket, message: &ServerMessage) -> Result<(), axum::Error> {
    let text = serde_json::to_string(message).expect("messages serialize");
    socket.send(Message::Text(text.into())).await
}

/// The next agent message; `None` when the socket is closed or broken. Unreadable messages
/// are logged and skipped, so a newer agent does not break an older Zentrale.
async fn receive(socket: &mut WebSocket) -> Option<AgentMessage> {
    loop {
        match socket.recv().await? {
            Ok(Message::Text(text)) => match serde_json::from_str(&text) {
                Ok(message) => return Some(message),
                Err(err) => tracing::warn!(%err, "unreadable agent message"),
            },
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
}
