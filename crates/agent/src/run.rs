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
    let mut syncthing = start(&spawn).await?;
    let client = syncthing.client().clone();
    let report = Report {
        name: options.name.clone().unwrap_or_else(host_name),
        os: std::env::consts::OS.to_string(),
        syncthing_id: client.my_id().await?,
        syncthing_version: SYNCTHING_VERSION.to_string(),
    };
    tracing::info!(id = %report.syncthing_id, "Syncthing running");

    let (snapshots, mut status) = watch::channel(Snapshot::default());
    let monitor = tokio::spawn(monitor::run(
        client.clone(),
        monitor::Options::default(),
        snapshots,
        None,
    ));

    let connect = async {
        let mut pause = MIN_PAUSE;
        let mut last_problem: Option<String> = None;
        loop {
            let result = session::run(
                data_dir,
                &mut config,
                &client,
                &report,
                &mut status,
                session::HEARTBEAT,
            )
            .await;
            let problem = match &result {
                Ok(End::Closed) => None,
                Ok(End::Rejected(reason)) => {
                    Some(format!("the Zentrale turned the agent away: {reason}"))
                }
                Err(err) => Some(format!("connection to the Zentrale failed: {err:#}")),
            };
            // A warning once per problem: retrying an unreachable Zentrale for weeks must not
            // fill agent.log (it is not rotated) or the journal.
            if let Some(problem) = &problem {
                if last_problem.as_ref() == Some(problem) {
                    tracing::debug!("{problem}");
                } else {
                    tracing::warn!("{problem}");
                }
            }
            last_problem = problem;
            let (wait, next) = backoff(result.as_ref().ok(), pause);
            pause = next;
            tokio::time::sleep(wait).await;
        }
    };
    // Syncthing runs with --no-restart: when it exits, the agent exits with an error and the
    // service manager (systemd Restart=on-failure, launchd KeepAlive, the task's
    // RestartOnFailure) starts both again, instead of the agent reporting a dead Syncthing.
    let exited = tokio::select! {
        () = connect => None,
        () = shutdown => None,
        status = syncthing.exited() => Some(status),
    };
    monitor.abort();
    if let Some(status) = exited {
        let how = status.map_or_else(|err| err.to_string(), |status| status.to_string());
        anyhow::bail!("Syncthing exited ({how})");
    }
    syncthing.stop().await?;
    Ok(())
}

/// How long to wait before connecting again after a session that `ended` (`None`: failed), and
/// the pause for the failure after that: 1 s, doubling up to 30 s while connecting fails; a
/// minute after being turned away. A session that ended without an error starts over at 1 s.
fn backoff(ended: Option<&End>, pause: Duration) -> (Duration, Duration) {
    match ended {
        Some(End::Closed) => (MIN_PAUSE, MIN_PAUSE),
        Some(End::Rejected(_)) => (REJECTED_PAUSE, MIN_PAUSE),
        None => (pause, (pause * 2).min(MAX_PAUSE)),
    }
}

fn host_name() -> String {
    hostname::get()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "device".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waits_after_failures(count: usize) -> Vec<u64> {
        let mut pause = MIN_PAUSE;
        (0..count)
            .map(|_| {
                let (wait, next) = backoff(None, pause);
                pause = next;
                wait.as_secs()
            })
            .collect()
    }

    #[test]
    fn failures_wait_one_second_then_double_up_to_thirty() {
        assert_eq!(waits_after_failures(7), [1, 2, 4, 8, 16, 30, 30]);
    }

    #[test]
    fn a_rejection_or_a_closed_session_starts_over() {
        let rejected = End::Rejected("no".into());
        assert_eq!(
            backoff(Some(&rejected), MAX_PAUSE),
            (REJECTED_PAUSE, MIN_PAUSE)
        );
        assert_eq!(
            backoff(Some(&End::Closed), MAX_PAUSE),
            (MIN_PAUSE, MIN_PAUSE)
        );
    }
}
