//! What the Zentrale stores: devices, folders and who holds which folder how.

use serde::{Deserialize, Serialize};
use stellwerk_protocol::{FolderMode, Versioning};

/// What kind of machine a device is; only for the icon in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceKind {
    Desktop,
    Laptop,
    Server,
    Phone,
}

impl DeviceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Laptop => "laptop",
            Self::Server => "server",
            Self::Phone => "phone",
        }
    }

    pub fn parse(text: &str) -> Self {
        match text {
            "laptop" => Self::Laptop,
            "server" => Self::Server,
            "phone" => Self::Phone,
            _ => Self::Desktop,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub os: String,
    #[serde(skip)]
    pub public_key: String,
    pub syncthing_id: String,
    pub agent_version: String,
    pub syncthing_version: String,
    /// `false`: blocked, removed from every folder and turned away when it connects.
    pub approved: bool,
    pub created_at: i64,
    pub last_seen: Option<i64>,
    pub applied_rev: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub label: String,
    /// The folder ID inside Syncthing, the same on every device.
    pub syncthing_folder_id: String,
    pub versioning: Versioning,
    pub created_at: i64,
}

/// A device holding a folder: mode and local path. No share means "off".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    pub folder_id: String,
    pub device_id: String,
    pub mode: FolderMode,
    pub path: String,
}

/// The whole configuration at one revision.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub revision: u64,
    pub devices: Vec<Device>,
    pub folders: Vec<Folder>,
    pub shares: Vec<Share>,
}

/// The server's own Syncthing, member of every folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hub {
    pub syncthing_id: String,
    /// Where devices reach the hub, e.g. `tcp://sync.example.org:22001`.
    pub address: String,
    /// Directory under which the hub keeps one subdirectory per folder.
    pub folders_dir: String,
}

pub fn mode_to_str(mode: FolderMode) -> &'static str {
    match mode {
        FolderMode::SendReceive => "sendReceive",
        FolderMode::SendOnly => "sendOnly",
        FolderMode::ReceiveOnly => "receiveOnly",
    }
}

pub fn mode_from_str(text: &str) -> Option<FolderMode> {
    match text {
        "sendReceive" => Some(FolderMode::SendReceive),
        "sendOnly" => Some(FolderMode::SendOnly),
        "receiveOnly" => Some(FolderMode::ReceiveOnly),
        _ => None,
    }
}

/// The hub keeps versions for a year unless a folder says otherwise.
pub const DEFAULT_HUB_VERSIONING: Versioning = Versioning::Staggered { max_age_days: 365 };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_round_trip_as_text() {
        for mode in [
            FolderMode::SendReceive,
            FolderMode::SendOnly,
            FolderMode::ReceiveOnly,
        ] {
            assert_eq!(mode_from_str(mode_to_str(mode)), Some(mode));
        }
        assert_eq!(mode_from_str("off"), None);
    }
}
