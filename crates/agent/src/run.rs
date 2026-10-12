//! The agent's life: start its own Syncthing, stay connected to the Zentrale, reconnect with
//! a growing pause when the connection drops.

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::watch;

use stellwerk_engine::download::ensure_syncthing;
use stellwerk_engine::monitor::{self, Snapshot};
use stellwerk_engine::pin::SYNCTHING_VERSION;
use stellwerk_engine::process::{SpawnOptions, free_port, start};

use crate::config;
use crate::session::{self, End, Report};

/// Where the agent's Syncthing accepts sync connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    /// All interfaces, on a port picked once and kept in Syncthing's config.
    Lan,
    /// 127.0.0.1 only, for tests and several agents on one machine.
    Isolated,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub data_dir: PathBuf,
    /// A Syncthing binary to use instead of downloading the pinned one.
    pub syncthing_binary: Option<PathBuf>,
    pub network: Network,
    /// The name the device suggests to the Zentrale; the host name by default.
    pub name: Option<String>,
}

const MIN_PAUSE: Duration = Duration::from_secs(1);
const MAX_PAUSE: Duration = Duration::from_secs(30);
const REJECTED_PAUSE: Duration = Duration::from_secs(60);

/// Runs until `shutdown` completes, then stops Syncthing.
pub async fn run(options: Options, shutdown: impl Future<Output = ()>) -> anyhow::Result<()> {
    let data_dir = &options.data_dir;
    let mut config = config::load(data_dir)?;
    let binary = match &options.syncthing_binary {
        Some(binary) => binary.clone(),
        None => ensure_syncthing(&data_dir.join("syncthing-bin")).await?,
    };
    let home = data_dir.join("syncthing");
    let spawn = match options.network {
        Network::Isolated => SpawnOptions::isolated(binary, home)?,
        Network::Lan => SpawnOptions {
            binary,
            home,
            listen_address: format!("tcp://0.0.0.0:{}", free_port()?),
            discovery: false,
        },
    };
    let syncthing = start(&spawn).await?;
    let report = Report {
        name: options.name.clone().unwrap_or_else(host_name),
        os: std::env::consts::OS.to_string(),
        syncthing_id: syncthing.client().my_id().await?,
        syncthing_version: SYNCTHING_VERSION.to_string(),
    };
    tracing::info!(id = %report.syncthing_id, "Syncthing running");

    let (snapshots, mut status) = watch::channel(Snapshot::default());
    let monitor = tokio::spawn(monitor::run(
        syncthing.client().clone(),
        monitor::Options::default(),
        snapshots,
        None,
    ));

    let connect = async {
        let mut pause = MIN_PAUSE;
        loop {
            let result = session::run(
                data_dir,
                &mut config,
                syncthing.client(),
                &report,
                &mut status,
            )
            .await;
            let wait = match result {
                Ok(End::Closed) => {
                    pause = MIN_PAUSE;
                    MIN_PAUSE
                }
                Ok(End::Rejected(reason)) => {
                    tracing::warn!(%reason, "the Zentrale turned the agent away");
                    REJECTED_PAUSE
                }
                Err(err) => {
                    tracing::warn!(%err, "connection to the Zentrale failed");
                    pause = (pause * 2).min(MAX_PAUSE);
                    pause
                }
            };
            tokio::time::sleep(wait).await;
        }
    };
    tokio::select! {
        () = connect => {}
        () = shutdown => {}
    }
    monitor.abort();
    syncthing.stop().await?;
    Ok(())
}

fn host_name() -> String {
    hostname::get()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "device".into())
}
