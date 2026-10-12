//! Starts and stops a Syncthing that belongs to Stellwerk alone.
//!
//! Own `--home`, GUI and API only on 127.0.0.1 with a random port and a fresh API key, no
//! auto-upgrade, no usage reporting. Ports are always picked by the OS, never Syncthing's
//! standard ports 8384 and 22000, so a Syncthing the user runs is left alone.

use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use rand::Rng;
use rand::distr::Alphanumeric;
use tokio::process::{Child, Command};

use crate::client::Client;
use crate::error::{Error, Result, io_at};

/// Syncthing's defaults for the GUI and for sync connections; never used here.
pub const STANDARD_PORTS: [u16; 2] = [8384, 22000];

const STARTUP_TIMEOUT: Duration = Duration::from_secs(60);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);
const LOG_FILE: &str = "stellwerk-syncthing.log";

#[derive(Debug, Clone)]
pub struct SpawnOptions {
    pub binary: PathBuf,
    /// Configuration, keys and database.
    pub home: PathBuf,
    /// Where Syncthing accepts sync connections, e.g. `tcp://127.0.0.1:41234`.
    pub listen_address: String,
    /// Global and local discovery, relays and NAT traversal.
    pub discovery: bool,
}

impl SpawnOptions {
    /// Sync connections only on 127.0.0.1 with a random port and no discovery: for tests and
    /// for several instances on one machine.
    pub fn isolated(binary: PathBuf, home: PathBuf) -> Result<Self> {
        Ok(Self {
            binary,
            home,
            listen_address: format!("tcp://127.0.0.1:{}", free_port()?),
            discovery: false,
        })
    }
}

/// A running Syncthing; killed when dropped.
#[derive(Debug)]
pub struct Instance {
    child: Child,
    client: Client,
    listen_address: String,
    home: PathBuf,
}

impl Instance {
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// The sync address other devices dial.
    pub fn listen_address(&self) -> &str {
        &self.listen_address
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// Asks Syncthing to shut down and kills it if it does not.
    pub async fn stop(mut self) -> Result<()> {
        let asked = self.client.shutdown().await.is_ok();
        let exited = asked
            && tokio::time::timeout(SHUTDOWN_TIMEOUT, self.child.wait())
                .await
                .is_ok();
        if !exited {
            self.child.kill().await.map_err(io_at(&self.home))?;
        }
        Ok(())
    }
}

/// Starts Syncthing and waits until its API answers. A home without `config.xml` gets one with
/// the options above; an existing one is kept, only GUI address and API key are set anew.
pub async fn start(options: &SpawnOptions) -> Result<Instance> {
    let home = &options.home;
    std::fs::create_dir_all(home).map_err(io_at(home))?;
    let config = home.join("config.xml");
    if !config.exists() {
        let xml = seed_config(&options.listen_address, options.discovery);
        std::fs::write(&config, xml).map_err(io_at(&config))?;
    }

    let gui = format!("http://127.0.0.1:{}", free_port()?);
    let api_key = random_key();
    let log_path = home.join(LOG_FILE);
    let log = std::fs::File::create(&log_path).map_err(io_at(&log_path))?;
    let log_err = log.try_clone().map_err(io_at(&log_path))?;

    let child = Command::new(&options.binary)
        .arg("serve")
        .arg("--home")
        .arg(home)
        .args(["--no-browser", "--no-restart", "--no-upgrade"])
        .arg(format!("--gui-address={gui}"))
        .arg(format!("--gui-apikey={api_key}"))
        .env("STNOUPGRADE", "1")
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err)
        .kill_on_drop(true)
        .spawn()
        .map_err(io_at(&options.binary))?;

    let client = Client::new(gui.parse().expect("valid GUI URL"), api_key);
    let mut instance = Instance {
        child,
        client,
        listen_address: options.listen_address.clone(),
        home: home.clone(),
    };
    wait_until_ready(&mut instance).await?;
    Ok(instance)
}

async fn wait_until_ready(instance: &mut Instance) -> Result<()> {
    let deadline = tokio::time::Instant::now() + STARTUP_TIMEOUT;
    loop {
        if let Some(status) = instance.child.try_wait().map_err(io_at(&instance.home))? {
            return Err(Error::Startup(format!(
                "exited with {status}\n{}",
                log_tail(&instance.home)
            )));
        }
        if instance.client.ping().await.is_ok() {
            return Ok(());
        }
        if tokio::time::Instant::now() > deadline {
            return Err(Error::Startup(format!(
                "API did not answer within {STARTUP_TIMEOUT:?}\n{}",
                log_tail(&instance.home)
            )));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

fn log_tail(home: &Path) -> String {
    let log = std::fs::read_to_string(home.join(LOG_FILE)).unwrap_or_default();
    let start = log.len().saturating_sub(4000);
    log.get(start..).unwrap_or(&log).to_string()
}

/// Completes on Ctrl-C, and on SIGTERM where there is one (Docker, systemd), so that
/// Syncthing is shut down cleanly instead of being killed.
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = terminate.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// The first `config.xml`. Syncthing fills in defaults for everything left out
/// (lib/config/config.go, `ReadXML`) and creates keys on first start.
pub fn seed_config(listen_address: &str, discovery: bool) -> String {
    format!(
        r#"<configuration version="{version}">
    <options>
        <listenAddress>{listen}</listenAddress>
        <globalAnnounceEnabled>{discovery}</globalAnnounceEnabled>
        <localAnnounceEnabled>{discovery}</localAnnounceEnabled>
        <relaysEnabled>{discovery}</relaysEnabled>
        <natEnabled>{discovery}</natEnabled>
        <urAccepted>-1</urAccepted>
        <autoUpgradeIntervalH>0</autoUpgradeIntervalH>
        <crashReportingEnabled>false</crashReportingEnabled>
        <startBrowser>false</startBrowser>
    </options>
</configuration>
"#,
        version = SYNCTHING_CONFIG_VERSION,
        listen = xml_escape(listen_address),
    )
}

/// `CurrentVersion` in lib/config/config.go of the pinned release.
const SYNCTHING_CONFIG_VERSION: u32 = 52;

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A free port on 127.0.0.1 chosen by the OS, never one of [`STANDARD_PORTS`].
pub fn free_port() -> Result<u16> {
    loop {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(io_at("127.0.0.1:0"))?;
        let port = listener.local_addr().map_err(io_at("127.0.0.1:0"))?.port();
        if !STANDARD_PORTS.contains(&port) {
            return Ok(port);
        }
    }
}

fn random_key() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_config_listens_where_told_and_reports_nothing() {
        let xml = seed_config("tcp://127.0.0.1:41000", false);
        assert!(xml.contains("<listenAddress>tcp://127.0.0.1:41000</listenAddress>"));
        assert!(xml.contains("<globalAnnounceEnabled>false</globalAnnounceEnabled>"));
        assert!(xml.contains("<urAccepted>-1</urAccepted>"));
        assert!(xml.contains("<autoUpgradeIntervalH>0</autoUpgradeIntervalH>"));
    }

    #[test]
    fn free_ports_are_never_the_standard_ones() {
        for _ in 0..20 {
            assert!(!STANDARD_PORTS.contains(&free_port().unwrap()));
        }
    }

    #[test]
    fn api_keys_are_random() {
        assert_ne!(random_key(), random_key());
        assert_eq!(random_key().len(), 32);
    }
}
