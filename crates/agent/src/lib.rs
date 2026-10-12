//! The agent on each device: brings its own Syncthing and keeps it at the desired state the
//! Zentrale sends.

pub mod config;
pub mod discover;
pub mod identity;
pub mod run;
pub mod service;
pub mod session;

pub use config::AgentConfig;
pub use run::{Network, Options, run};

/// The data directory when none is given: `~/.local/share/stellwerk-agent` on Linux, the
/// platform's local application data elsewhere.
pub fn default_data_dir() -> anyhow::Result<std::path::PathBuf> {
    Ok(dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("no data directory; pass --data-dir"))?
        .join("stellwerk-agent"))
}

/// Runs until Ctrl-C, SIGTERM or the stop file of `service uninstall`.
pub async fn run_until_stopped(options: Options) -> anyhow::Result<()> {
    let stop = service::stop_requested(options.data_dir.clone(), std::time::SystemTime::now());
    run(options, async {
        tokio::select! {
            () = stellwerk_engine::process::shutdown_signal() => {}
            () = stop => {}
        }
    })
    .await
}

/// Stores what pairing needs: the Zentrale's URL, the device's key (kept if it has one) and
/// the one-time code, used on the first connection. Without a code the device waits for
/// approval in the UI. Returns the key's fingerprint, which the UI shows for the device.
pub fn join(
    data_dir: &std::path::Path,
    server: &str,
    code: Option<&str>,
) -> anyhow::Result<String> {
    config::ws_url(server)?;
    let secret_key = match config::load(data_dir) {
        Ok(existing) => existing.secret_key,
        Err(_) => identity::new_secret(),
    };
    let public_key = identity::public_key(&identity::signing_key(&secret_key)?);
    config::save(
        data_dir,
        &AgentConfig {
            server: server.trim_end_matches('/').to_string(),
            secret_key,
            enrollment_code: code.map(|code| code.trim().to_string()),
        },
    )?;
    Ok(stellwerk_protocol::messages::fingerprint(&public_key))
}
