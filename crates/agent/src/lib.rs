//! The agent on each device: brings its own Syncthing and keeps it at the desired state the
//! Zentrale sends.

pub mod config;
pub mod identity;
pub mod run;
pub mod session;

pub use config::AgentConfig;
pub use run::{Network, Options, run};

/// Stores what pairing needs: the Zentrale's URL, a new key and the one-time code. The code is
/// used on the first connection.
pub fn join(data_dir: &std::path::Path, server: &str, code: &str) -> anyhow::Result<()> {
    config::ws_url(server)?;
    let secret_key = match config::load(data_dir) {
        Ok(existing) => existing.secret_key,
        Err(_) => identity::new_secret(),
    };
    config::save(
        data_dir,
        &AgentConfig {
            server: server.trim_end_matches('/').to_string(),
            secret_key,
            enrollment_code: Some(code.trim().to_string()),
        },
    )
}
