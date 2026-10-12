//! What agent and Zentrale say to each other, and the desired state of a device.
//!
//! The types describe devices, folders, modes and paths only, nothing that is specific to
//! Syncthing beyond the device ID, so a different engine can take over behind `engine` later.

use serde::{Deserialize, Serialize};

pub mod messages;

/// A Syncthing device ID in its canonical form (`XXXXXXX-XXXXXXX-…`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(pub String);

impl DeviceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// How a device takes part in a folder. "Off" is no entry at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FolderMode {
    SendReceive,
    SendOnly,
    ReceiveOnly,
}

/// How old file versions are kept when a peer changes or deletes a file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Versioning {
    #[default]
    None,
    /// Fewer versions the older they get, none older than `max_age_days`.
    Staggered { max_age_days: u32 },
}

/// Another device this device talks to directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    pub device_id: DeviceId,
    pub name: String,
    /// Where to reach the peer, e.g. `tcp://hub.example.org:22000`; empty means "discover it".
    #[serde(default)]
    pub addresses: Vec<String>,
}

/// A folder as it should exist on this device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub label: String,
    pub path: String,
    pub mode: FolderMode,
    /// The peers this folder is shared with; each must be listed in [`DesiredState::peers`].
    pub shared_with: Vec<DeviceId>,
    #[serde(default)]
    pub versioning: Versioning,
    /// Syncthing ignore patterns, one per entry, the same on every device.
    #[serde(default)]
    pub ignore_patterns: Vec<String>,
}

/// The complete configuration a device should have, never a diff.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesiredState {
    pub revision: u64,
    pub peers: Vec<Peer>,
    pub folders: Vec<Folder>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_state_round_trips_through_json() {
        let state = DesiredState {
            revision: 42,
            peers: vec![Peer {
                device_id: DeviceId::new("AAAAAAA"),
                name: "hub".into(),
                addresses: vec!["tcp://127.0.0.1:42000".into()],
            }],
            folders: vec![Folder {
                id: "fotos".into(),
                label: "Fotos".into(),
                path: "/srv/fotos".into(),
                mode: FolderMode::ReceiveOnly,
                shared_with: vec![DeviceId::new("AAAAAAA")],
                versioning: Versioning::Staggered { max_age_days: 30 },
                ignore_patterns: vec!["*.tmp".into()],
            }],
        };

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"mode\":\"receiveOnly\""));
        assert!(json.contains("\"versioning\":{\"type\":\"staggered\",\"max_age_days\":30}"));
        assert_eq!(serde_json::from_str::<DesiredState>(&json).unwrap(), state);
    }
}
