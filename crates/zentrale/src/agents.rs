//! The agents' WebSocket: challenge, signed hello, then the desired state on every change.
//! An unknown agent without a code waits on the open socket until it is approved or rejected
//! in the UI (LAN pairing).

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use stellwerk_protocol::messages::{AgentMessage, Applied, Hello, ServerMessage};

use crate::app::App;
use crate::desired;
use crate::model::{Device, DeviceKind};
use crate::pairing::{code_hash, new_nonce, verify_hello};
use crate::store::{self, Enrolled, NewDevice, now};
use crate::waiting::{Decision, Refused};

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
        _ => return reject(&mut socket, "expected a hello").await,
    };
    let device = match identify(&app, &nonce, &hello).await {
        Ok(Some(device)) => device,
        Ok(None) if app.lan_pairing() => match approval(&app, &mut socket, report(&hello)).await {
            Ok(Some(device)) => device,
            Ok(None) => return,
            Err(reason) => return reject(&mut socket, &reason).await,
        },
        Ok(None) => return reject(&mut socket, "this device is not paired").await,
        Err(reason) => return reject(&mut socket, &reason).await,
    };
    if let Err(reason) = admit(&app, &device, &hello).await {
        return reject(&mut socket, &reason).await;
    }

    let connection = app.set_online(&device.id, now());
    tracing::info!(device = %device.name, "agent connected");
    let _ = app
        .store()
        .add_event(now(), Some(&device.id), None, "online", "")
        .await;
    let welcome = ServerMessage::Welcome {
        device: device.id.clone(),
    };
    if send(&mut socket, &welcome).await.is_ok() {
        serve(&app, &device.id, &mut socket).await;
    }
    app.set_offline(&device.id, connection);
    let _ = app.store().touch_device(&device.id, now()).await;
    let _ = app
        .store()
        .add_event(now(), Some(&device.id), None, "offline", "")
        .await;
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
                Some(AgentMessage::Status(status)) => app.set_device_status(device_id, status.folders, now()),
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
            .add_event(at, Some(device_id), None, "error", &text)
            .await;
    }
}

/// The device for a verified hello: enrolled now with its code, or known by its key; `None`
/// for an unknown key without a code.
async fn identify(app: &App, nonce: &str, hello: &Hello) -> Result<Option<Device>, String> {
    verify_hello(&hello.public_key, &hello.signature, nonce).map_err(|err| err.to_string())?;
    let store = app.store();
    let at = now();

    if let Some(code) = &hello.enrollment_code {
        let enrolled = store
            .enroll(&code_hash(code), report(hello), at)
            .await
            .map_err(|err| err.to_string())?;
        if let Some(Enrolled { device, new }) = enrolled {
            if new {
                paired(app, &device).await;
            }
            return Ok(Some(device));
        }
    }
    // An agent that was paired but lost the answer still has its key: let it in.
    let known = store
        .device_by_key(&hello.public_key)
        .await
        .map_err(|err| err.to_string())?;
    match (known, &hello.enrollment_code) {
        (Some(device), _) => Ok(Some(device)),
        (None, Some(_)) => Err("the pairing code is unknown, expired or used".into()),
        (None, None) => Ok(None),
    }
}

/// Lets a known device in: not blocked, and what it reports is recorded.
async fn admit(app: &App, device: &Device, hello: &Hello) -> Result<(), String> {
    if !device.approved {
        return Err("this device is blocked".into());
    }
    app.store()
        .device_seen(&device.id, &report(hello), now())
        .await
        .map_err(|err| err.to_string())?;
    app.changed().await.map_err(|err| err.to_string())
}

/// Waits on the open socket until the UI decides about the agent. `Ok(None)` when the agent
/// went away or was rejected (it has been told).
async fn approval(
    app: &App,
    socket: &mut WebSocket,
    device: NewDevice,
) -> Result<Option<Device>, String> {
    let public_key = device.public_key.clone();
    let name = device.name.clone();
    let place = app
        .wait_for_approval(device, now())
        .map_err(|refused| refused.reason().to_string())?;
    tracing::info!(%name, fingerprint = %place.fingerprint, "agent waits for approval");
    let waiting = ServerMessage::Waiting {
        fingerprint: place.fingerprint.clone(),
    };
    let decision = if send(socket, &waiting).await.is_ok() {
        tokio::select! {
            decision = place.decision => decision.ok(),
            () = closed(socket) => None,
        }
    } else {
        None
    };
    app.stop_waiting(&place.fingerprint, place.ticket);
    match decision {
        Some(Decision::Approve) => app
            .store()
            .device_by_key(&public_key)
            .await
            .map_err(|err| err.to_string()),
        Some(Decision::Reject) => Err(Refused::Rejected.reason().into()),
        None => Ok(None),
    }
}

/// Approves or rejects a waiting agent; `false` when it is no longer waiting.
pub async fn decide(app: &App, fingerprint: &str, decision: Decision) -> store::Result<bool> {
    let Some((device, session)) = app.decide_waiting(fingerprint, decision) else {
        return Ok(false);
    };
    if decision == Decision::Approve {
        let device = app.store().add_device(device, now()).await?;
        paired(app, &device).await;
        app.changed().await?;
    }
    let _ = session.send(decision);
    Ok(true)
}

async fn paired(app: &App, device: &Device) {
    let text = format!("{} was paired", device.name);
    let _ = app
        .store()
        .add_event(now(), Some(&device.id), None, "paired", &text)
        .await;
}

/// Completes when the agent closes the socket; whatever it sends meanwhile is ignored.
async fn closed(socket: &mut WebSocket) {
    while receive(socket).await.is_some() {}
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

async fn reject(socket: &mut WebSocket, reason: &str) {
    tracing::info!(reason, "agent rejected");
    let _ = send(
        socket,
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
