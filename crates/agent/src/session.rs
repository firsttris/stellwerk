//! One connection to the Zentrale: answer the challenge, then apply every desired state and
//! report it back.

use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use stellwerk_engine::Client;
use stellwerk_engine::monitor::Snapshot;
use stellwerk_protocol::messages::{
    AgentMessage, Applied, ApplyError, Directory, Hello, ServerMessage, Status,
};
use stellwerk_protocol::{DesiredState, DeviceId};
use tokio::sync::watch;
use tokio::time::{Instant, MissedTickBehavior};
use tokio_tungstenite::tungstenite::Message;

use crate::config::{self, AgentConfig};
use crate::{identity, local_ignores};

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

/// How the agent notices a connection that died without a word (suspend, NAT timeout, Wi-Fi
/// roaming), which TCP alone notices only after hours: it pings `every` so often, and a
/// connection without any frame from the Zentrale for `silence` counts as lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heartbeat {
    pub every: Duration,
    pub silence: Duration,
    /// How often the exclusion lists of other programs are read again (see [`local_ignores`]).
    pub lists: Duration,
}

pub const HEARTBEAT: Heartbeat = Heartbeat {
    every: Duration::from_secs(30),
    silence: Duration::from_secs(90),
    lists: Duration::from_secs(10),
};

/// Runs one connection until it closes. `status` is the monitor's latest snapshot, sent on
/// connecting and on change.
pub async fn run(
    data_dir: &Path,
    config: &mut AgentConfig,
    syncthing: &Client,
    report: &Report,
    status: &mut watch::Receiver<Snapshot>,
    heartbeat: Heartbeat,
) -> anyhow::Result<End> {
    let url = config::ws_url(&config.server)?;
    let connector = crate::ca::connector(config.ca.as_deref())?;
    let (socket, _) =
        tokio_tungstenite::connect_async_tls_with_config(url.as_str(), None, false, connector)
            .await
            .with_context(|| format!("connecting to {url}"))?;
    let (mut sink, mut stream) = socket.split();

    let challenge = tokio::time::timeout(heartbeat.silence, receive(&mut stream))
        .await
        .map_err(|_| anyhow::anyhow!("the Zentrale sent no challenge"))??;
    let nonce = match challenge {
        Some(ServerMessage::Challenge { nonce }) => nonce,
        Some(ServerMessage::Rejected { reason }) => return Ok(End::Rejected(reason)),
        _ => anyhow::bail!("the Zentrale did not send a challenge"),
    };
    let hello = hello(config, report, &nonce)?;
    send(&mut sink, &AgentMessage::Hello(hello)).await?;

    let mut ping = tokio::time::interval_at(Instant::now() + heartbeat.every, heartbeat.every);
    ping.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut deadline = Instant::now() + heartbeat.silence;
    let mut welcomed = false;
    // The last desired state and the exclusion lists it was applied with: a changed list is
    // applied without waiting for the next state from the Zentrale.
    let lists_dir = local_ignores::dir(data_dir);
    let mut last: Option<DesiredState> = None;
    let mut lists_seen = String::new();
    let mut lists_tick =
        tokio::time::interval_at(Instant::now() + heartbeat.lists, heartbeat.lists);
    lists_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            frame = tokio::time::timeout_at(deadline, next_frame(&mut stream)) => {
                let frame = frame.map_err(|_| {
                    anyhow::anyhow!("no answer from the Zentrale for {} s", heartbeat.silence.as_secs())
                })??;
                deadline = Instant::now() + heartbeat.silence;
                let message = match frame {
                    Frame::Message(message) => message,
                    Frame::Other => continue,
                    Frame::Closed => return Ok(End::Closed),
                };
                match message {
                    ServerMessage::Welcome { device } => {
                        tracing::info!(%device, "connected to the Zentrale");
                        if config.enrollment_code.take().is_some() {
                            config::save(data_dir, config)?;
                            tracing::info!("paired");
                        }
                        welcomed = true;
                        status.mark_changed();
                    }
                    ServerMessage::DesiredState(state) => {
                        lists_seen = local_ignores::read_lists(&lists_dir);
                        let applied = apply(syncthing, &with_lists(&state, &lists_seen)).await;
                        last = Some(state);
                        send(&mut sink, &AgentMessage::Applied(applied)).await?;
                    }
                    ServerMessage::Waiting { fingerprint } => {
                        tracing::info!(%fingerprint, "waiting for approval in the Zentrale's UI; check that it shows this fingerprint");
                    }
                    ServerMessage::Browse { id, path } => {
                        let result = browse(path).await;
                        send(&mut sink, &AgentMessage::Listing { id, result }).await?;
                    }
                    ServerMessage::Rejected { reason } => return Ok(End::Rejected(reason)),
                    ServerMessage::Challenge { .. } => {}
                }
            }
            _ = ping.tick() => sink.send(Message::Ping(Default::default())).await?,
            _ = lists_tick.tick(), if last.is_some() => {
                let lists = local_ignores::read_lists(&lists_dir);
                if lists != lists_seen && let Some(state) = &last {
                    tracing::info!("exclusion lists changed; applying them");
                    let applied = apply(syncthing, &with_lists(state, &lists)).await;
                    lists_seen = lists;
                    send(&mut sink, &AgentMessage::Applied(applied)).await?;
                }
            }
            changed = status.changed(), if welcomed => {
                if changed.is_err() {
                    return Ok(End::Closed);
                }
                let folders = status.borrow_and_update().folders.clone();
                send(&mut sink, &AgentMessage::Status(Status { folders })).await?;
            }
        }
    }
}

/// The desired state with the exclusions of other programs on this device added.
fn with_lists(state: &DesiredState, lists: &str) -> DesiredState {
    local_ignores::merge(
        state,
        &local_ignores::paths(lists),
        dirs::home_dir().as_deref(),
    )
}

/// The hello for this connection's nonce, signed with the device's key.
fn hello(config: &AgentConfig, report: &Report, nonce: &str) -> anyhow::Result<Hello> {
    let key = identity::signing_key(&config.secret_key)?;
    let hello = Hello {
        public_key: identity::public_key(&key),
        signature: String::new(),
        enrollment_code: config.enrollment_code.clone(),
        name: report.name.clone(),
        os: report.os.clone(),
        agent_version: env!("CARGO_PKG_VERSION").to_string(),
        syncthing_version: report.syncthing_version.clone(),
        syncthing_id: DeviceId::new(&report.syncthing_id),
    };
    Ok(identity::sign_hello(&key, nonce, hello))
}

/// A directory for the UI's folder picker, read off the async runtime.
async fn browse(path: String) -> Result<Directory, String> {
    tokio::task::spawn_blocking(move || crate::browse::list(&path, dirs::home_dir().as_deref()))
        .await
        .unwrap_or_else(|err| Err(err.to_string()))
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
type Sink = SplitSink<Socket, Message>;
type Stream = SplitStream<Socket>;

async fn send(sink: &mut Sink, message: &AgentMessage) -> anyhow::Result<()> {
    let text = serde_json::to_string(message)?;
    sink.send(Message::text(text)).await?;
    Ok(())
}

/// One frame from the Zentrale, as far as the agent cares.
enum Frame {
    Message(ServerMessage),
    /// A pong, a ping or a message this agent cannot read; it still shows the link is alive.
    Other,
    Closed,
}

/// The next frame. Unreadable messages are skipped, so a newer Zentrale does not break an
/// older agent.
async fn next_frame(stream: &mut Stream) -> anyhow::Result<Frame> {
    Ok(match stream.next().await.transpose()? {
        None | Some(Message::Close(_)) => Frame::Closed,
        Some(Message::Text(text)) => match serde_json::from_str(&text) {
            Ok(message) => Frame::Message(message),
            Err(err) => {
                tracing::warn!(%err, "unreadable message from the Zentrale");
                Frame::Other
            }
        },
        Some(_) => Frame::Other,
    })
}

/// The next message from the Zentrale; `None` when the connection closed.
async fn receive(stream: &mut Stream) -> anyhow::Result<Option<ServerMessage>> {
    loop {
        match next_frame(stream).await? {
            Frame::Message(message) => return Ok(Some(message)),
            Frame::Other => {}
            Frame::Closed => return Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;
    use tokio_tungstenite::WebSocketStream;

    use super::*;

    const QUICK: Heartbeat = Heartbeat {
        every: Duration::from_millis(100),
        silence: Duration::from_millis(400),
        lists: Duration::from_millis(100),
    };

    /// A Zentrale on 127.0.0.1 that welcomes the agent, then hands the socket to `then`.
    async fn zentrale<F, Fut>(then: F) -> String
    where
        F: FnOnce(WebSocketStream<tokio::net::TcpStream>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let say =
                |message: &ServerMessage| Message::text(serde_json::to_string(message).unwrap());
            let challenge = ServerMessage::Challenge { nonce: "n".into() };
            socket.send(say(&challenge)).await.unwrap();
            socket.next().await.unwrap().unwrap(); // the hello
            let welcome = ServerMessage::Welcome { device: "d".into() };
            socket.send(say(&welcome)).await.unwrap();
            then(socket).await;
        });
        url
    }

    async fn session(server: String) -> anyhow::Result<End> {
        let dir = tempfile::tempdir().unwrap();
        session_in(dir.path(), server).await
    }

    async fn session_in(dir: &Path, server: String) -> anyhow::Result<End> {
        let mut config = AgentConfig {
            server,
            secret_key: identity::new_secret(),
            enrollment_code: None,
            ca: None,
        };
        let syncthing = Client::new("http://127.0.0.1:9".parse().unwrap(), "unused");
        let report = Report {
            name: "laptop".into(),
            os: "linux".into(),
            syncthing_id: "AAAA".into(),
            syncthing_version: "v2".into(),
        };
        let (_snapshots, mut status) = watch::channel(Snapshot::default());
        run(dir, &mut config, &syncthing, &report, &mut status, QUICK).await
    }

    #[tokio::test]
    async fn a_silent_zentrale_counts_as_lost() {
        // Reads nothing, so it answers no ping.
        let url = zentrale(|socket| async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
            drop(socket);
        })
        .await;
        let ended = tokio::time::timeout(Duration::from_secs(5), session(url)).await;
        assert!(ended.expect("the session noticed the silence").is_err());
    }

    #[tokio::test]
    async fn pings_keep_a_quiet_connection_open() {
        let (pings_tx, pings_rx) = tokio::sync::oneshot::channel();
        // Reads, and so answers every ping with a pong, but never says anything itself.
        let url = zentrale(|mut socket| async move {
            let mut pings = 0;
            let until = Instant::now() + QUICK.silence * 3;
            while let Ok(Some(Ok(frame))) = tokio::time::timeout_at(until, socket.next()).await {
                pings += usize::from(matches!(frame, Message::Ping(_)));
            }
            let _ = pings_tx.send(pings);
            tokio::time::sleep(Duration::from_secs(10)).await;
        })
        .await;
        let running = tokio::spawn(session(url));
        let pings = tokio::time::timeout(Duration::from_secs(5), pings_rx)
            .await
            .unwrap()
            .unwrap();
        assert!(pings >= 5, "{pings} pings");
        assert!(!running.is_finished(), "{:?}", running.await);
        running.abort();
    }

    #[tokio::test]
    async fn a_browse_request_gets_the_listing_under_its_id() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Fotos")).unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        let (answer_tx, answer_rx) = tokio::sync::oneshot::channel();
        let url = zentrale(move |mut socket| async move {
            let browse = ServerMessage::Browse { id: 42, path };
            let text = serde_json::to_string(&browse).unwrap();
            socket.send(Message::text(text)).await.unwrap();
            while let Some(Ok(frame)) = socket.next().await {
                let Message::Text(text) = frame else { continue };
                if let Ok(listing @ AgentMessage::Listing { .. }) = serde_json::from_str(&text) {
                    let _ = answer_tx.send(listing);
                    break;
                }
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        })
        .await;
        let running = tokio::spawn(session(url));
        let answer = tokio::time::timeout(Duration::from_secs(5), answer_rx)
            .await
            .unwrap()
            .unwrap();
        running.abort();
        let AgentMessage::Listing { id, result } = answer else {
            unreachable!()
        };
        assert_eq!(id, 42);
        let names: Vec<String> = result.unwrap().dirs.into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["Fotos"]);
    }

    /// A changed exclusion list is applied at once, without a new desired state.
    #[tokio::test]
    async fn a_changed_exclusion_list_is_applied_again() {
        let dir = tempfile::tempdir().unwrap();
        let (applied_tx, mut applied_rx) = tokio::sync::mpsc::unbounded_channel();
        let url = zentrale(move |mut socket| async move {
            let state = ServerMessage::DesiredState(DesiredState {
                revision: 7,
                ..DesiredState::default()
            });
            socket
                .send(Message::text(serde_json::to_string(&state).unwrap()))
                .await
                .unwrap();
            while let Some(Ok(frame)) = socket.next().await {
                let Message::Text(text) = frame else { continue };
                if let Ok(AgentMessage::Applied(applied)) = serde_json::from_str(&text) {
                    let _ = applied_tx.send(applied.revision);
                }
            }
        })
        .await;
        let data = dir.path().to_path_buf();
        let running = tokio::spawn(async move { session_in(&data, url).await });
        let wait = Duration::from_secs(5);
        let first = tokio::time::timeout(wait, applied_rx.recv()).await.unwrap();
        assert_eq!(first, Some(7));
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            applied_rx.try_recv().is_err(),
            "unchanged lists apply nothing"
        );

        let lists = local_ignores::dir(dir.path());
        std::fs::create_dir_all(&lists).unwrap();
        std::fs::write(lists.join("spielstand.list"), "/home/t/Games/x.sav\n").unwrap();
        let again = tokio::time::timeout(wait, applied_rx.recv()).await.unwrap();
        assert_eq!(again, Some(7), "the same state, applied with the new list");
        running.abort();
    }
}
