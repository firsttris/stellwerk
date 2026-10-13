use std::net::SocketAddr;
use std::path::PathBuf;

use std::io::BufRead;

use clap::{Parser, Subcommand};
use stellwerk_engine::process::shutdown_signal;
use stellwerk_zentrale::auth::hash_password;
use stellwerk_zentrale::discovery::announce;
use stellwerk_zentrale::server::{Options, start};
use stellwerk_zentrale::store::Store;

/// Stellwerk Zentrale: central web UI to manage Syncthing on all your devices.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Database, hub Syncthing and the hub's copy of every folder.
    #[arg(long, env = "STELLWERK_DATA", default_value = "data", global = true)]
    data_dir: PathBuf,
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    serve: Serve,
}

#[derive(Subcommand)]
enum Command {
    /// Set the admin password, read from standard input; logs out every session.
    SetPassword,
    /// Download the pinned Syncthing, check it, and put binary and license into a directory
    /// (used when building the Docker image).
    FetchSyncthing {
        /// Target directory, e.g. /opt/syncthing
        dir: PathBuf,
    },
}

/// Running the Zentrale (the default).
#[derive(clap::Args)]
struct Serve {
    /// Address of the web UI and the agents' WebSocket.
    #[arg(long, env = "STELLWERK_LISTEN", default_value = "0.0.0.0:8080")]
    listen: SocketAddr,
    /// Where the hub Syncthing accepts sync connections. Not 22000, so a Syncthing already
    /// running on the server keeps its port.
    #[arg(
        long,
        env = "STELLWERK_HUB_LISTEN",
        default_value = "tcp://0.0.0.0:22001"
    )]
    hub_listen: String,
    /// Where devices reach the hub; by default the host name with the port of --hub-listen.
    #[arg(long, env = "STELLWERK_HUB_ADDRESS")]
    hub_address: Option<String>,
    /// Use this Syncthing binary instead of downloading the pinned version.
    #[arg(long, env = "STELLWERK_SYNCTHING")]
    syncthing: Option<PathBuf>,
    /// LAN pairing: announce the Zentrale over mDNS and let devices without a code wait for
    /// approval in the UI.
    #[arg(
        long,
        env = "STELLWERK_LAN_PAIRING",
        default_value_t = true,
        action = clap::ArgAction::Set
    )]
    lan_pairing: bool,
    /// The address devices use for the web UI, e.g. https://sync.example.org; announced in
    /// the LAN. By default devices use the announced address and port.
    #[arg(long, env = "STELLWERK_PUBLIC_URL")]
    public_url: Option<String>,
    /// The certificate (PEM) of a private CA behind the Zentrale's HTTPS, e.g. Caddy's
    /// `pki/authorities/local/root.crt`: served at /ca.crt, and its fingerprint goes into the
    /// pairing command, so devices trust it without setup.
    #[arg(long, env = "STELLWERK_CA_CERT")]
    ca_cert: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let cli = Cli::parse();
    match cli.command {
        Some(Command::SetPassword) => set_password(&cli.data_dir).await,
        Some(Command::FetchSyncthing { dir }) => fetch_syncthing(&dir).await,
        None => serve(cli.data_dir, cli.serve).await,
    }
}

async fn serve(data_dir: PathBuf, serve: Serve) -> anyhow::Result<()> {
    let hub_address = match serve.hub_address {
        Some(address) => address,
        None => default_hub_address(&serve.hub_listen)?,
    };
    let options = Options {
        data_dir,
        syncthing_binary: serve.syncthing,
        hub_listen: serve.hub_listen,
        hub_address,
        activity_window: stellwerk_zentrale::hub::ACTIVITY_WINDOW,
        lan_pairing: serve.lan_pairing,
        ca_cert: serve.ca_cert,
    };
    let listener = tokio::net::TcpListener::bind(serve.listen).await?;
    let mut running = start(&options, listener).await?;
    let announced = serve
        .lan_pairing
        .then(|| announce(serve.listen.port(), serve.public_url.as_deref()))
        .and_then(|result| {
            result
                .inspect_err(
                    |err| tracing::warn!(%err, "could not announce the Zentrale in the LAN"),
                )
                .ok()
        });
    if running.app.store().admin_password_hash().await?.is_none() {
        tracing::warn!("no admin password yet: run `stellwerk set-password` to log in");
    }
    let hub_exit = tokio::select! {
        () = shutdown_signal() => None,
        how = running.hub_exited() => Some(how),
    };
    if let Some(daemon) = announced {
        let _ = daemon.shutdown();
    }
    match hub_exit {
        None => {
            tracing::info!("shutting down");
            running.stop().await
        }
        Some(how) => anyhow::bail!("the hub Syncthing exited ({how})"),
    }
}

async fn fetch_syncthing(dir: &std::path::Path) -> anyhow::Result<()> {
    let cache = dir.join(".download");
    let binary = stellwerk_engine::download::ensure_syncthing(&cache).await?;
    std::fs::create_dir_all(dir)?;
    let name = binary.file_name().expect("the binary has a name");
    std::fs::copy(&binary, dir.join(name))?;
    if let Some(release) = binary.parent() {
        std::fs::copy(release.join("LICENSE.txt"), dir.join("LICENSE.txt"))?;
    }
    std::fs::remove_dir_all(&cache)?;
    println!("{}", dir.join(name).display());
    Ok(())
}

async fn set_password(data_dir: &std::path::Path) -> anyhow::Result<()> {
    eprintln!("New admin password (one line):");
    let mut password = String::new();
    std::io::stdin().lock().read_line(&mut password)?;
    let hash = hash_password(password.trim_end_matches(['\r', '\n']))?;
    std::fs::create_dir_all(data_dir)?;
    let store = Store::open(&data_dir.join("stellwerk.db")).await?;
    store.set_admin_password_hash(&hash).await?;
    eprintln!("Password set.");
    Ok(())
}

/// `tcp://0.0.0.0:22001` → `tcp://<host name>:22001`.
fn default_hub_address(listen: &str) -> anyhow::Result<String> {
    hub_address(listen, &hostname::get()?.to_string_lossy())
}

fn hub_address(listen: &str, host: &str) -> anyhow::Result<String> {
    let port = listen
        .rsplit(':')
        .next()
        .filter(|port| port.parse::<u16>().is_ok())
        .ok_or_else(|| anyhow::anyhow!("--hub-listen has no port: {listen}"))?;
    Ok(format!("tcp://{host}:{port}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hub_address_takes_the_port_of_the_listen_address() {
        let address = |listen| hub_address(listen, "nas").ok();
        assert_eq!(
            address("tcp://0.0.0.0:22001").as_deref(),
            Some("tcp://nas:22001")
        );
        assert_eq!(
            address("tcp://[::]:22002").as_deref(),
            Some("tcp://nas:22002")
        );
        assert_eq!(address("tcp://0.0.0.0"), None);
        assert_eq!(address("tcp://0.0.0.0:99999"), None);
    }
}
