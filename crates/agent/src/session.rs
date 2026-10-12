//! One connection to the Zentrale: answer the challenge, then apply every desired state and
//! report it back.

use std::path::Path;

use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use stellwerk_engine::Client;
use stellwerk_protocol::messages::{AgentMessage, Applied, ApplyError, Hello, ServerMessage};
use stellwerk_protocol::{DesiredState, DeviceId};
use tokio_tungstenite::tungstenite::Message;

use crate::config::{self, AgentConfig};
use crate::identity;

/// What the agent tells the Zentrale about itself besides its key.
#[derive(Debug, Clone)]
pub struct Report {
    pub name: String,
    pub os: String,
    pub syncthing_id: String,
    pub syncthing_version: String,
}

/// How a session ended without an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    Closed,
    Rejected(String),
}

/// Runs one connection until it closes. `last_applied` carries the applied revision across
/// reconnects.
pub async fn run(
    data_dir: &Path,
    config: &mut AgentConfig,
    syncthing: &Client,
    report: &Report,
    last_applied: &mut Option<u64>,
) -> anyhow::Result<End> {
    let url = config::ws_url(&config.server)?;
    let (mut socket, _) = tokio_tungstenite::connect_async(url.as_str())
        .await
        .with_context(|| format!("connecting to {url}"))?;

    let nonce = match receive(&mut socket).await? {
        Some(ServerMessage::Challenge { nonce }) => nonce,
        Some(ServerMessage::Rejected { reason }) => return Ok(End::Rejected(reason)),
        _ => anyhow::bail!("the Zentrale did not send a challenge"),
    };
    let key = identity::signing_key(&config.secret_key)?;
    let hello = Hello {
        public_key: identity::public_key(&key),
        signature: identity::sign_challenge(&key, &nonce),
        enrollment_code: config.enrollment_code.clone(),
        name: report.name.clone(),
        os: report.os.clone(),
        agent_version: env!("CARGO_PKG_VERSION").to_string(),
        syncthing_version: report.syncthing_version.clone(),
        syncthing_id: DeviceId::new(&report.syncthing_id),
        applied_revision: *last_applied,
    };
    send(&mut socket, &AgentMessage::Hello(hello)).await?;

    loop {
        match receive(&mut socket).await? {
            Some(ServerMessage::Welcome { device }) => {
                tracing::info!(%device, "connected to the Zentrale");
                if config.enrollment_code.take().is_some() {
                    config::save(data_dir, config)?;
                    tracing::info!("paired");
                }
            }
            Some(ServerMessage::DesiredState(state)) => {
                let applied = apply(syncthing, &state).await;
                *last_applied = Some(applied.revision);
                send(&mut socket, &AgentMessage::Applied(applied)).await?;
            }
            Some(ServerMessage::Rejected { reason }) => return Ok(End::Rejected(reason)),
            Some(ServerMessage::Challenge { .. }) => {}
            None => return Ok(End::Closed),
        }
    }
}

/// Reconciles the local Syncthing; whatever fails becomes an error in the report.
async fn apply(syncthing: &Client, state: &DesiredState) -> Applied {
    let errors = match stellwerk_engine::reconcile(syncthing, state).await {
        Ok(outcome) => outcome
            .failures
            .iter()
            .map(|failure| ApplyError {
                folder: failure.call.folder().map(str::to_string),
                message: failure.error.to_string(),
            })
            .collect(),
        Err(err) => vec![ApplyError {
            folder: None,
            message: err.to_string(),
        }],
    };
    tracing::info!(
        revision = state.revision,
        errors = errors.len(),
        "applied desired state"
    );
    Applied {
        revision: state.revision,
        errors,
    }
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn send(socket: &mut Socket, message: &AgentMessage) -> anyhow::Result<()> {
    let text = serde_json::to_string(message)?;
    socket.send(Message::text(text)).await?;
    Ok(())
}

/// The next message from the Zentrale; `None` when the connection closed. Unreadable
/// messages are skipped, so a newer Zentrale does not break an older agent.
async fn receive(socket: &mut Socket) -> anyhow::Result<Option<ServerMessage>> {
    while let Some(message) = socket.next().await {
        match message? {
            Message::Text(text) => match serde_json::from_str(&text) {
                Ok(message) => return Ok(Some(message)),
                Err(err) => tracing::warn!(%err, "unreadable message from the Zentrale"),
            },
            Message::Close(_) => return Ok(None),
            _ => {}
        }
    }
    Ok(None)
}
