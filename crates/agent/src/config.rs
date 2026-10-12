//! What the agent keeps between runs: where the Zentrale is, its key and, until it is paired,
//! the one-time code. Stored as `agent.json` in the data directory, readable only by its owner.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

const FILE: &str = "agent.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Base URL of the Zentrale, e.g. `https://sync.example.org`.
    pub server: String,
    /// Ed25519 secret key, base64.
    pub secret_key: String,
    /// The pairing code until the Zentrale has accepted it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrollment_code: Option<String>,
}

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE)
}

pub fn load(data_dir: &Path) -> anyhow::Result<AgentConfig> {
    let path = path(data_dir);
    let text = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "{} not found; pair the device first with `stellwerk-agent join`",
            path.display()
        )
    })?;
    serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))
}

pub fn save(data_dir: &Path, config: &AgentConfig) -> anyhow::Result<()> {
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("creating {}", data_dir.display()))?;
    let path = path(data_dir);
    let tmp = path.with_extension("json.tmp");
    write_private(&tmp, serde_json::to_string_pretty(config)?.as_bytes())
        .with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, &path).with_context(|| format!("writing {}", path.display()))
}

/// Writes a file only its owner can read; on Unix it is created with mode 600, so the key is
/// never readable by others, not even for a moment. Left over from a crash, it is replaced.
fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let _ = std::fs::remove_file(path);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(contents)
}

/// The WebSocket URL for a Zentrale base URL: `https://x` → `wss://x/api/agent/ws`.
pub fn ws_url(server: &str) -> anyhow::Result<String> {
    let server = server.trim_end_matches('/');
    let rest = if let Some(rest) = server.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = server.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        anyhow::bail!("the server URL must start with https:// or http://: {server}");
    };
    Ok(format!(
        "{rest}{}",
        stellwerk_protocol::messages::AGENT_WS_PATH
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_config_loads_again() {
        let dir = tempfile::tempdir().unwrap();
        let config = AgentConfig {
            server: "https://sync.example.org".into(),
            secret_key: "secret".into(),
            enrollment_code: Some("K7Q2-MX9P-41TZ".into()),
        };
        save(dir.path(), &config).unwrap();
        assert_eq!(load(dir.path()).unwrap(), config);
        save(dir.path(), &config).unwrap();
        assert_eq!(load(dir.path()).unwrap(), config);
    }

    #[cfg(unix)]
    #[test]
    fn only_the_owner_can_read_the_config() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let config = AgentConfig {
            server: "https://sync.example.org".into(),
            secret_key: "secret".into(),
            enrollment_code: None,
        };
        std::fs::write(path(dir.path()).with_extension("json.tmp"), "left over").unwrap();
        save(dir.path(), &config).unwrap();
        let mode = std::fs::metadata(path(dir.path()))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn ws_urls_follow_the_scheme() {
        assert_eq!(
            ws_url("https://sync.example.org/").unwrap(),
            "wss://sync.example.org/api/agent/ws"
        );
        assert_eq!(
            ws_url("http://127.0.0.1:8080").unwrap(),
            "ws://127.0.0.1:8080/api/agent/ws"
        );
        assert!(ws_url("sync.example.org").is_err());
    }
}
