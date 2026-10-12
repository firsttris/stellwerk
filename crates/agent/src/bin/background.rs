//! The agent without a console window, for the task at logon on Windows (a console program
//! would open one). Logs into `agent.log` in the data directory instead.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;

use clap::Parser;
use stellwerk_agent::{Network, Options, default_data_dir, run_until_stopped};

/// Stellwerk agent in the background; started by `stellwerk-agent service install`.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[arg(long, env = "STELLWERK_AGENT_DATA")]
    data_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let data_dir = match cli.data_dir {
        Some(dir) => dir,
        None => default_data_dir()?,
    };
    std::fs::create_dir_all(&data_dir)?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir.join("agent.log"))?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_ansi(false)
        .with_writer(Mutex::new(log))
        .init();
    let options = Options {
        data_dir,
        syncthing_binary: None,
        network: Network::Lan,
        name: None,
    };
    let result = run_until_stopped(options).await;
    if let Err(err) = &result {
        tracing::error!(%err, "the agent stopped");
    }
    result
}
