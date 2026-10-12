use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use stellwerk_zentrale::server::{Options, start};

/// Stellwerk Zentrale: central web UI to manage Syncthing on all your devices.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Database, hub Syncthing and the hub's copy of every folder.
    #[arg(long, env = "STELLWERK_DATA", default_value = "data")]
    data_dir: PathBuf,
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let cli = Cli::parse();
    let hub_address = match cli.hub_address {
        Some(address) => address,
        None => default_hub_address(&cli.hub_listen)?,
    };
    let options = Options {
        data_dir: cli.data_dir,
        syncthing_binary: cli.syncthing,
        hub_listen: cli.hub_listen,
        hub_address,
    };
    let listener = tokio::net::TcpListener::bind(cli.listen).await?;
    let running = start(&options, listener).await?;
    tokio::signal::ctrl_c().await?;
    running.stop().await
}

/// `tcp://0.0.0.0:22001` → `tcp://<host name>:22001`.
fn default_hub_address(listen: &str) -> anyhow::Result<String> {
    let port = listen
        .rsplit(':')
        .next()
        .filter(|port| port.parse::<u16>().is_ok())
        .ok_or_else(|| anyhow::anyhow!("--hub-listen has no port: {listen}"))?;
    let host = hostname::get()?.to_string_lossy().into_owned();
    Ok(format!("tcp://{host}:{port}"))
}
