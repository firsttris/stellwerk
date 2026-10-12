//! What is happening right now: folder states from the agents, the hub's view of every device
//! (also offline ones), and which files arrived from where. Kept in memory; pure functions turn
//! it into what the UI shows and what goes into the activity feed.

use std::collections::BTreeMap;

use serde::Serialize;
use stellwerk_engine::monitor::Snapshot;
use stellwerk_protocol::status::{FolderStatus, RemoteChange, SyncState};

use stellwerk_engine::files::Conflict;

use crate::files::{ConflictView, conflict_view};
use crate::model::Config;
use crate::waiting::WaitingView;

/// The latest status of one device, as its agent reported it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceLive {
    pub at: i64,
    pub folders: Vec<FolderStatus>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Live {
    /// By device ID.
    pub devices: BTreeMap<String, DeviceLive>,
    pub hub: Snapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderView {
    pub state: SyncState,
    pub global_files: u64,
    pub global_bytes: u64,
    pub need_files: u64,
    pub need_bytes: u64,
    pub errors: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionView {
    pub percent: f64,
    pub need_bytes: u64,
    pub need_items: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLiveView {
    pub at: i64,
    /// By folder ID.
    pub folders: BTreeMap<String, FolderView>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HubLiveView {
    /// By folder ID.
    pub folders: BTreeMap<String, FolderView>,
    /// Folder ID → device ID → how much of the folder the device has.
    pub completion: BTreeMap<String, BTreeMap<String, CompletionView>>,
}

/// The live state in the Zentrale's own IDs, for `GET /api/status` and the UI's WebSocket.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveView {
    pub devices: BTreeMap<String, DeviceLiveView>,
    pub hub: HubLiveView,
    /// Conflict copies on the hub by folder ID.
    pub conflicts: BTreeMap<String, Vec<ConflictView>>,
    /// Agents waiting for approval (LAN pairing); [`view`] leaves it empty, the API fills it
    /// from the running Zentrale.
    pub waiting: Vec<WaitingView>,
}

fn folder_view(status: &FolderStatus) -> FolderView {
    FolderView {
        state: status.state,
        global_files: status.global_files,
        global_bytes: status.global_bytes,
        need_files: status.need_files,
        need_bytes: status.need_bytes,
        errors: status.errors,
        error: status.error.clone(),
    }
}

/// Syncthing folder ID → folder ID, for the folders that still exist.
fn folder_ids(config: &Config) -> BTreeMap<&str, &str> {
    config
        .folders
        .iter()
        .map(|f| (f.syncthing_folder_id.as_str(), f.id.as_str()))
        .collect()
}

fn folder_views(config: &Config, folders: &[FolderStatus]) -> BTreeMap<String, FolderView> {
    let ids = folder_ids(config);
    folders
        .iter()
        .filter_map(|status| {
            Some((
                ids.get(status.folder.as_str())?.to_string(),
                folder_view(status),
            ))
        })
        .collect()
}

/// Maps Syncthing's IDs to the Zentrale's and drops what no longer exists.
pub fn view(config: &Config, live: &Live, conflicts: &BTreeMap<String, Vec<Conflict>>) -> LiveView {
    let devices = live
        .devices
        .iter()
        .filter(|(id, _)| config.devices.iter().any(|d| &d.id == *id))
        .map(|(id, device)| {
            let view = DeviceLiveView {
                at: device.at,
                folders: folder_views(config, &device.folders),
            };
            (id.clone(), view)
        })
        .collect();

    let folders = folder_ids(config);
    let mut completion: BTreeMap<String, BTreeMap<String, CompletionView>> = BTreeMap::new();
    for entry in &live.hub.completion {
        let Some(folder) = folders.get(entry.folder.as_str()) else {
            continue;
        };
        let Some(device) = config
            .devices
            .iter()
            .find(|d| d.syncthing_id == entry.device)
        else {
            continue;
        };
        completion.entry(folder.to_string()).or_default().insert(
            device.id.clone(),
            CompletionView {
                percent: entry.percent,
                need_bytes: entry.need_bytes,
                need_items: entry.need_items,
            },
        );
    }
    let conflicts = conflicts
        .iter()
        .filter(|(folder, _)| config.folders.iter().any(|f| &f.id == *folder))
        .map(|(folder, list)| {
            let views = list.iter().map(|c| conflict_view(config, c)).collect();
            (folder.clone(), views)
        })
        .collect();
    LiveView {
        devices,
        hub: HubLiveView {
            folders: folder_views(config, &live.hub.folders),
            completion,
        },
        conflicts,
        waiting: Vec::new(),
    }
}

/// Changed files per device and folder: (device ID, folder ID, count). `modified_by` is the
/// start of a Syncthing device ID (its short ID); changes from unknown devices are dropped.
pub fn summarize_changes(
    config: &Config,
    changes: &[RemoteChange],
) -> Vec<(String, String, usize)> {
    let folders = folder_ids(config);
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for change in changes {
        let Some(folder) = folders.get(change.folder.as_str()) else {
            continue;
        };
        let Some(device) = config.devices.iter().find(|d| {
            !change.modified_by.is_empty() && d.syncthing_id.starts_with(&change.modified_by)
        }) else {
            continue;
        };
        *counts
            .entry((device.id.clone(), folder.to_string()))
            .or_default() += 1;
    }
    counts
        .into_iter()
        .map(|((device, folder), count)| (device, folder, count))
        .collect()
}

#[cfg(test)]
mod tests {
    use stellwerk_protocol::status::Completion;

    use super::*;
    use crate::model::{DEFAULT_HUB_VERSIONING, Device, DeviceKind, Folder};

    fn config() -> Config {
        let device = |id: &str, st: &str| Device {
            id: id.into(),
            name: id.into(),
            kind: DeviceKind::Desktop,
            os: "linux".into(),
            public_key: String::new(),
            syncthing_id: st.into(),
            agent_version: String::new(),
            syncthing_version: String::new(),
            approved: true,
            created_at: 0,
            last_seen: None,
            applied_rev: None,
        };
        Config {
            revision: 1,
            devices: vec![
                device("laptop", "AAAAAAA-BBBBBBB"),
                device("pc", "CCCCCCC-DDDDDDD"),
            ],
            folders: vec![Folder {
                id: "docs".into(),
                label: "Docs".into(),
                syncthing_folder_id: "st-docs".into(),
                versioning: DEFAULT_HUB_VERSIONING,
                ignore_patterns: vec![],
                created_at: 0,
            }],
            shares: vec![],
        }
    }

    fn status(folder: &str, state: SyncState) -> FolderStatus {
        FolderStatus {
            folder: folder.into(),
            state,
            need_files: 3,
            ..FolderStatus::default()
        }
    }

    #[test]
    fn the_view_uses_the_zentrales_ids_and_drops_the_unknown() {
        let live = Live {
            devices: BTreeMap::from([
                (
                    "laptop".into(),
                    DeviceLive {
                        at: 5,
                        folders: vec![
                            status("st-docs", SyncState::Syncing),
                            status("gone", SyncState::Idle),
                        ],
                    },
                ),
                ("removed".into(), DeviceLive::default()),
            ]),
            hub: Snapshot {
                folders: vec![status("st-docs", SyncState::Idle)],
                completion: vec![
                    Completion {
                        folder: "st-docs".into(),
                        device: "CCCCCCC-DDDDDDD".into(),
                        percent: 40.0,
                        need_bytes: 600,
                        need_items: 2,
                    },
                    Completion {
                        folder: "st-docs".into(),
                        device: "UNKNOWN".into(),
                        ..Completion::default()
                    },
                ],
            },
        };

        let view = view(&config(), &live, &BTreeMap::new());

        assert_eq!(view.devices.keys().collect::<Vec<_>>(), ["laptop"]);
        let laptop = &view.devices["laptop"];
        assert_eq!(laptop.at, 5);
        assert_eq!(laptop.folders.keys().collect::<Vec<_>>(), ["docs"]);
        assert_eq!(laptop.folders["docs"].state, SyncState::Syncing);
        assert_eq!(view.hub.folders["docs"].state, SyncState::Idle);
        let docs = &view.hub.completion["docs"];
        assert_eq!(docs.keys().collect::<Vec<_>>(), ["pc"]);
        assert_eq!(docs["pc"].need_items, 2);
    }

    #[test]
    fn changes_are_counted_per_device_and_folder() {
        let change = |by: &str, folder: &str| RemoteChange {
            folder: folder.into(),
            modified_by: by.into(),
            path: "x".into(),
            deleted: false,
        };
        let changes = [
            change("AAAAAAA", "st-docs"),
            change("AAAAAAA", "st-docs"),
            change("CCCCCCC", "st-docs"),
            change("ZZZZZZZ", "st-docs"),
            change("AAAAAAA", "unknown"),
            change("", "st-docs"),
        ];
        assert_eq!(
            summarize_changes(&config(), &changes),
            [
                ("laptop".to_string(), "docs".to_string(), 2),
                ("pc".to_string(), "docs".to_string(), 1)
            ]
        );
    }
}
