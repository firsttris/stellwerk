//! The agent on each device: brings its own Syncthing and keeps it at the desired state the
//! Zentrale sends.

pub mod ca;
pub mod config;
pub mod discover;
pub mod identity;
pub mod run;
pub mod service;
pub mod session;

use anyhow::Context;

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

/// Stores what pairing needs: the Zentrale's URL, the device's key (kept if it has one), the
/// one-time code, used on the first connection, and the Zentrale's private CA if it has one
/// (checked by [`ca::fetch`]; a stored one is kept when none is given). Without a code the
/// device waits for approval in the UI. Returns the key's fingerprint, which the UI shows for
/// the device.
pub fn join(
    data_dir: &std::path::Path,
    server: &str,
    code: Option<&str>,
    ca: Option<String>,
) -> anyhow::Result<String> {
    let server = server.trim().trim_end_matches('/');
    // An empty code would be refused as a wrong one; without one the device waits for approval.
    let code = code.map(str::trim).filter(|code| !code.is_empty());
    config::ws_url(server)?;
    // A key only for a device without one: an unreadable agent.json must not silently turn the
    // device into a new one.
    let path = config::path(data_dir);
    let existing = if path.exists() {
        Some(config::load(data_dir).with_context(|| {
            format!(
                "keeping the device's key; to pair it as a new device, delete {}",
                path.display()
            )
        })?)
    } else {
        None
    };
    let (secret_key, kept_ca) = match existing {
        // A CA belongs to its Zentrale: kept for the same server only.
        Some(config) => (
            config.secret_key,
            config.ca.filter(|_| config.server == server),
        ),
        None => (identity::new_secret(), None),
    };
    let public_key = identity::public_key(&identity::signing_key(&secret_key)?);
    config::save(
        data_dir,
        &AgentConfig {
            server: server.to_string(),
            secret_key,
            enrollment_code: code.map(str::to_string),
            ca: ca.or(kept_ca),
        },
    )?;
    Ok(stellwerk_protocol::messages::fingerprint(&public_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERVER: &str = "https://sync.example.org";

    #[test]
    fn joining_again_keeps_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let first = join(dir.path(), SERVER, Some("K7Q2-MX9P-41TZ"), None).unwrap();
        let again = join(dir.path(), "https://other.example.org", None, None).unwrap();
        assert_eq!(first, again);
        let config = config::load(dir.path()).unwrap();
        assert_eq!(config.server, "https://other.example.org");
        assert_eq!(config.enrollment_code, None);
    }

    #[test]
    fn server_and_code_are_trimmed() {
        let dir = tempfile::tempdir().unwrap();
        join(dir.path(), " https://sync.example.org/ ", Some(" "), None).unwrap();
        let config = config::load(dir.path()).unwrap();
        assert_eq!(config.server, SERVER);
        assert_eq!(config.enrollment_code, None);
        join(dir.path(), SERVER, Some(" K7Q2-MX9P-41TZ\n"), None).unwrap();
        let config = config::load(dir.path()).unwrap();
        assert_eq!(config.enrollment_code.as_deref(), Some("K7Q2-MX9P-41TZ"));
    }

    #[test]
    fn a_ca_is_stored_and_kept_for_the_same_server_only() {
        let dir = tempfile::tempdir().unwrap();
        let ca = Some("-----BEGIN CERTIFICATE-----".to_string());
        join(dir.path(), SERVER, Some("K7Q2-MX9P-41TZ"), ca.clone()).unwrap();
        join(dir.path(), SERVER, None, None).unwrap();
        assert_eq!(config::load(dir.path()).unwrap().ca, ca);
        join(dir.path(), "https://other.example.org", None, None).unwrap();
        assert_eq!(config::load(dir.path()).unwrap().ca, None);
    }

    #[test]
    fn an_unreadable_config_is_not_replaced() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(config::path(dir.path()), "{ truncated").unwrap();
        let err = join(dir.path(), SERVER, None, None).unwrap_err();
        assert!(format!("{err:#}").contains("delete"), "{err:#}");
        let left = std::fs::read_to_string(config::path(dir.path())).unwrap();
        assert_eq!(left, "{ truncated");
    }
}
