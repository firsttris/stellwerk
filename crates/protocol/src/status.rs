//! How far a device is with its folders: what the agent reports and the Zentrale shows.

use serde::{Deserialize, Serialize};

/// What a folder is doing right now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncState {
    Idle,
    Scanning,
    Syncing,
    /// Queued for scanning or syncing.
    Waiting,
    Error,
    #[default]
    Unknown,
}

/// One folder on one device.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderStatus {
    /// The folder's ID in the engine, as in the desired state.
    pub folder: String,
    pub state: SyncState,
    /// Everything in the folder as all devices together know it.
    pub global_files: u64,
    pub global_bytes: u64,
    /// What this device still lacks.
    pub need_files: u64,
    pub need_bytes: u64,
    /// Files that failed to sync.
    pub errors: u64,
    /// Why the folder as a whole stopped, if it did (e.g. path missing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// How much of a folder another device has, as this device sees it from that device's index.
/// Known even while the other device is offline.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Completion {
    pub folder: String,
    /// The other device's engine ID.
    pub device: String,
    /// 0 to 100.
    pub percent: f64,
    pub need_bytes: u64,
    pub need_items: u64,
}

/// A file that changed on another device and arrived here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteChange {
    pub folder: String,
    /// The start of the changing device's engine ID.
    pub modified_by: String,
    pub path: String,
    pub deleted: bool,
}
