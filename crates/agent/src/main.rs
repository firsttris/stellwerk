use std::path::PathBuf;

use clap::{Parser, Subcommand};
use stellwerk_agent::discover::discover;
use stellwerk_agent::service::{self, Service};
use stellwerk_agent::{Network, Options, default_data_dir, run_until_stopped};

/// How long `join` listens for a Zentrale in the LAN.
const DISCOVERY_TIME: std::time::Duration = std::time::Duration::from_secs(4);

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
    /// Pair this device with a Zentrale: with the code from "Pair device", or without one by
    /// approving the device in the UI.
    Join {
        /// The Zentrale's address, e.g. https://sync.example.org; found in the LAN if left out.
        #[arg(long)]
        server: Option<String>,
        /// The one-time code, e.g. K7Q2-MX9P-41TZ
        code: Option<String>,
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
    /// Run in the background from login on: a systemd user service, a launchd agent or a
    /// task at logon. Needs no admin rights.
    #[command(subcommand)]
    Service(ServiceCommand),
}

#[derive(Subcommand)]
enum ServiceCommand {
    /// Set up the service for this binary and data directory, and start it.
    Install,
    /// Stop the agent and remove the service; the data directory stays.
    Uninstall,
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
        None => default_data_dir()?,
    };
    match cli.command {
        Command::Join { server, code } => {
            let server = match server {
                Some(server) => server,
                None => find_zentrale().await?,
            };
            let fingerprint = stellwerk_agent::join(&data_dir, &server, code.as_deref())?;
            println!("Saved. Start the agent with: stellwerk-agent service install");
            if code.is_none() {
                println!(
                    "Then approve this device in the Zentrale's UI under Devices; it shows the fingerprint {fingerprint}."
                );
            }
            Ok(())
        }
        Command::Run { syncthing, name } => {
            let options = Options {
                data_dir,
                syncthing_binary: syncthing,
                network: Network::Lan,
                name,
            };
            run_until_stopped(options).await
        }
        Command::Service(ServiceCommand::Install) => {
            let service = Service {
                exe: std::env::current_exe()?,
                data_dir: std::path::absolute(&data_dir)?,
            };
            println!("{}", service::install(&service)?);
            Ok(())
        }
        Command::Service(ServiceCommand::Uninstall) => {
            service::uninstall(&data_dir)?;
            println!("Stopped the agent and removed the service.");
            Ok(())
        }
    }
}

/// The one Zentrale announced in the LAN.
async fn find_zentrale() -> anyhow::Result<String> {
    println!("Looking for a Zentrale in the LAN …");
    let found = discover(DISCOVERY_TIME).await?;
    match found.as_slice() {
        [one] => {
            println!("Found {} at {}", one.name, one.url);
            Ok(one.url.clone())
        }
        [] => anyhow::bail!("no Zentrale found in the LAN; pass its address with --server"),
        several => {
            let list: Vec<_> = several
                .iter()
                .map(|f| format!("  {} at {}", f.name, f.url))
                .collect();
            anyhow::bail!(
                "several Zentralen found; pick one with --server:\n{}",
                list.join("\n")
            )
        }
    }
}
