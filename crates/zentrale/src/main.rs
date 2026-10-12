use std::net::SocketAddr;
use std::path::PathBuf;

use std::io::BufRead;

use clap::{Parser, Subcommand};
use stellwerk_zentrale::auth::hash_password;
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
    };
    let listener = tokio::net::TcpListener::bind(serve.listen).await?;
    let running = start(&options, listener).await?;
    if running.app.store().admin_password_hash().await?.is_none() {
        tracing::warn!("no admin password yet: run `stellwerk set-password` to log in");
    }
    tokio::signal::ctrl_c().await?;
    running.stop().await
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
    let port = listen
        .rsplit(':')
        .next()
        .filter(|port| port.parse::<u16>().is_ok())
        .ok_or_else(|| anyhow::anyhow!("--hub-listen has no port: {listen}"))?;
    let host = hostname::get()?.to_string_lossy().into_owned();
    Ok(format!("tcp://{host}:{port}"))
}
