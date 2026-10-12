//! Runs the agent in the background for the current user, from login on: a systemd user
//! service on Linux, a launchd agent on macOS, a task at logon on Windows. Everything belongs
//! to the user, so no admin rights are needed and the agent can later replace its own binary.
//!
//! The definitions are pure functions of [`Service`]; installing them is in the platform
//! modules.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub mod definitions;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// What a service starts: this binary with this data directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    pub exe: PathBuf,
    pub data_dir: PathBuf,
}

/// Sets the service up and starts it now.
pub fn install(service: &Service) -> anyhow::Result<String> {
    platform::install(service)
}

/// Stops the agent cleanly (see [`STOP_FILE`]) and removes the service.
pub fn uninstall(data_dir: &Path) -> anyhow::Result<()> {
    platform::uninstall(data_dir)
}

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(windows)]
use windows as platform;

/// A file in the data directory that asks a running agent to stop. Windows ends a task
/// without a signal, which would leave Syncthing running; the agent watches for this file
/// instead and stops Syncthing first.
pub const STOP_FILE: &str = "stop";

/// How often a running agent looks for [`STOP_FILE`].
const STOP_POLL: Duration = Duration::from_secs(1);

/// Completes when a [`STOP_FILE`] written at or after `since` (the agent's start) appears in
/// `data_dir`, and removes it. An older one is left over from an uninstaller that did not
/// finish: it is removed without stopping. Comparing times instead of clearing the file on
/// start keeps a request that comes while the agent is still starting.
pub async fn stop_requested(data_dir: PathBuf, since: SystemTime) {
    let file = data_dir.join(STOP_FILE);
    loop {
        if let Ok(modified) = std::fs::metadata(&file).and_then(|m| m.modified()) {
            let _ = std::fs::remove_file(&file);
            if modified >= since {
                return;
            }
        }
        tokio::time::sleep(STOP_POLL).await;
    }
}

/// Asks a running agent to stop and waits until it removed the stop file (or `timeout`
/// passed, e.g. because no agent was running).
pub fn request_stop(data_dir: &Path, timeout: Duration) -> anyhow::Result<()> {
    let file = data_dir.join(STOP_FILE);
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(&file, "")?;
    let deadline = std::time::Instant::now() + timeout;
    while file.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
    }
    let _ = std::fs::remove_file(&file);
    Ok(())
}

/// Runs a command and fails with its output when it fails.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn command(program: &str, args: &[&str]) -> anyhow::Result<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|err| anyhow::anyhow!("running {program}: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        anyhow::bail!(
            "{program} {} failed: {}{}",
            args.join(" "),
            stdout,
            String::from_utf8_lossy(&output.stderr)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_stop_file_stops_a_running_agent() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().to_path_buf();
        let agent = tokio::spawn(stop_requested(data_dir.clone(), SystemTime::now()));
        tokio::time::sleep(Duration::from_millis(100)).await;
        let requester = tokio::task::spawn_blocking(move || {
            request_stop(&data_dir, Duration::from_secs(10)).unwrap();
        });
        tokio::time::timeout(Duration::from_secs(10), agent)
            .await
            .unwrap()
            .unwrap();
        requester.await.unwrap();
        assert!(!dir.path().join(STOP_FILE).exists());
    }

    #[tokio::test]
    async fn an_old_stop_file_does_not_stop_a_new_agent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(STOP_FILE), "").unwrap();
        let started = SystemTime::now() + Duration::from_secs(5);
        let agent = tokio::spawn(stop_requested(dir.path().to_path_buf(), started));
        tokio::time::sleep(STOP_POLL * 2).await;
        assert!(!agent.is_finished());
        assert!(!dir.path().join(STOP_FILE).exists());
        agent.abort();
    }

    #[tokio::test]
    async fn a_request_while_the_agent_starts_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let started = SystemTime::now() - Duration::from_secs(5);
        std::fs::write(dir.path().join(STOP_FILE), "").unwrap();
        let agent = tokio::spawn(stop_requested(dir.path().to_path_buf(), started));
        tokio::time::timeout(Duration::from_secs(5), agent)
            .await
            .unwrap()
            .unwrap();
    }
}
