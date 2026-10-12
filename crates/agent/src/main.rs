use std::path::PathBuf;

use clap::{Parser, Subcommand};
use stellwerk_agent::{Network, Options};
use stellwerk_engine::process::shutdown_signal;

/// Stellwerk agent: runs its own Syncthing and keeps it at the state the Zentrale sets.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Where the agent keeps its key, its Syncthing and Syncthing's database.
    #[arg(long, env = "STELLWERK_AGENT_DATA", global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pair this device with a Zentrale, using the code from "Pair device".
    Join {
        /// The Zentrale's address, e.g. https://sync.example.org
        #[arg(long)]
        server: String,
        /// The one-time code, e.g. K7Q2-MX9P-41TZ
        code: String,
    },
    /// Run: start Syncthing and follow the Zentrale.
    Run {
        /// Use this Syncthing binary instead of downloading the pinned version.
        #[arg(long, env = "STELLWERK_SYNCTHING")]
        syncthing: Option<PathBuf>,
        /// The device name shown in the Zentrale; the host name by default.
        #[arg(long)]
        name: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let cli = Cli::parse();
    let data_dir = match cli.data_dir {
        Some(dir) => dir,
        None => dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("no data directory; pass --data-dir"))?
            .join("stellwerk-agent"),
    };
    match cli.command {
        Command::Join { server, code } => {
            stellwerk_agent::join(&data_dir, &server, &code)?;
            println!("Saved. Start the agent with: stellwerk-agent run");
            Ok(())
        }
        Command::Run { syncthing, name } => {
            let options = Options {
                data_dir,
                syncthing_binary: syncthing,
                network: Network::Lan,
                name,
            };
            stellwerk_agent::run(options, shutdown_signal()).await
        }
    }
}
