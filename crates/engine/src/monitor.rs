//! Watching a running Syncthing: folder summaries, how far other devices are, and changes that
//! arrive from them. Long-polls `/rest/events` and reads the state afresh whenever something
//! happened, at most once per [`THROTTLE`].

use std::time::Duration;

use serde::Deserialize;
use stellwerk_protocol::status::{Completion, FolderStatus, RemoteChange, SyncState};
use tokio::sync::{mpsc, watch};

use crate::client::Client;
use crate::error::Result;

/// At most one fresh read per second, as the plan's status messages need.
pub const THROTTLE: Duration = Duration::from_secs(1);
/// Read again after this long even without events.
const IDLE_REFRESH_SECS: u32 = 30;
const RETRY: Duration = Duration::from_secs(5);

/// The events that change what a snapshot shows (lib/events/events.go).
const EVENTS: &str = "StateChanged,FolderSummary,FolderErrors,FolderCompletion,\
                      LocalIndexUpdated,RemoteIndexUpdated,DeviceConnected,\
                      DeviceDisconnected,RemoteChangeDetected,ConfigSaved";

/// Everything the monitor knows at one moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub folders: Vec<FolderStatus>,
    /// Only with [`Options::completion`]: every folder × every other device in it.
    pub completion: Vec<Completion>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Also read how far every other device is (the hub does, agents do not).
    pub completion: bool,
}

/// `GET /rest/db/status`, the part that is used (lib/model/folder_summary.go).
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct FolderSummary {
    pub state: String,
    pub error: String,
    pub errors: u64,
    pub global_files: u64,
    pub global_bytes: u64,
    pub need_files: u64,
    pub need_bytes: u64,
}

/// `GET /rest/db/completion` (lib/model/model.go, `FolderCompletion.Map`).
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CompletionReply {
    pub completion: f64,
    pub need_bytes: u64,
    pub need_items: u64,
}

/// One entry of `GET /rest/events`.
#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    pub id: u64,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// Syncthing's folder state names (lib/model/folderstate.go) in the protocol's terms.
pub fn sync_state(state: &str) -> SyncState {
    match state {
        "idle" => SyncState::Idle,
        "scanning" => SyncState::Scanning,
        "syncing" | "sync-preparing" | "cleaning" => SyncState::Syncing,
        "scan-waiting" | "sync-waiting" | "clean-waiting" => SyncState::Waiting,
        "error" => SyncState::Error,
        _ => SyncState::Unknown,
    }
}

pub fn folder_status(folder: &str, summary: &FolderSummary) -> FolderStatus {
    FolderStatus {
        folder: folder.to_string(),
        state: sync_state(&summary.state),
        global_files: summary.global_files,
        global_bytes: summary.global_bytes,
        need_files: summary.need_files,
        need_bytes: summary.need_bytes,
        errors: summary.errors,
        error: (!summary.error.is_empty()).then(|| summary.error.clone()),
    }
}

/// `RemoteChangeDetected` events as changes; other events are skipped.
pub fn remote_changes(events: &[Event]) -> Vec<RemoteChange> {
    events
        .iter()
        .filter(|event| event.kind == "RemoteChangeDetected")
        .filter_map(|event| {
            let data = &event.data;
            Some(RemoteChange {
                folder: data.get("folder")?.as_str()?.to_string(),
                modified_by: data.get("modifiedBy")?.as_str()?.to_string(),
                path: data.get("path")?.as_str()?.to_string(),
                deleted: data.get("action").and_then(|a| a.as_str()) == Some("deleted"),
            })
        })
        .collect()
}

/// Reads the state of every configured folder, and with `completion` how far each other
/// device in it is.
pub async fn snapshot(client: &Client, options: Options) -> Result<Snapshot> {
    let config = client.config().await?;
    let my_id = if options.completion {
        client.my_id().await?
    } else {
        String::new()
    };
    let mut snapshot = Snapshot::default();
    for folder in &config.folders {
        let summary = client.folder_summary(&folder.id).await?;
        snapshot.folders.push(folder_status(&folder.id, &summary));
        if !options.completion {
            continue;
        }
        for device in folder.devices.iter().filter(|d| d.device_id != my_id) {
            let reply = client.completion(&folder.id, &device.device_id).await?;
            snapshot.completion.push(Completion {
                folder: folder.id.clone(),
                device: device.device_id.clone(),
                percent: reply.completion,
                need_bytes: reply.need_bytes,
                need_items: reply.need_items,
            });
        }
    }
    Ok(snapshot)
}

/// Runs until the receivers are gone: publishes a new snapshot whenever it changed and sends
/// arriving remote changes to `changes`. Syncthing keeps events for a mask only from its first
/// request on, so start this together with Syncthing.
pub async fn run(
    client: Client,
    options: Options,
    snapshots: watch::Sender<Snapshot>,
    changes: Option<mpsc::UnboundedSender<Vec<RemoteChange>>>,
) {
    let mut since = 0;
    loop {
        match snapshot(&client, options).await {
            Ok(fresh) => {
                snapshots.send_if_modified(|current| {
                    let modified = *current != fresh;
                    *current = fresh;
                    modified
                });
            }
            Err(err) => tracing::debug!(%err, "could not read Syncthing's state"),
        }
        if snapshots.is_closed() {
            return;
        }
        match client.events(since, EVENTS, IDLE_REFRESH_SECS).await {
            Ok(events) => {
                since = events.last().map_or(since, |event| event.id);
                let arrived = remote_changes(&events);
                if let Some(changes) = &changes
                    && !arrived.is_empty()
                {
                    let _ = changes.send(arrived);
                }
                tokio::time::sleep(THROTTLE).await;
            }
            Err(err) => {
                tracing::debug!(%err, "event stream interrupted");
                tokio::time::sleep(RETRY).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syncthing_states_map_to_protocol_states() {
        assert_eq!(sync_state("idle"), SyncState::Idle);
        assert_eq!(sync_state("sync-preparing"), SyncState::Syncing);
        assert_eq!(sync_state("scan-waiting"), SyncState::Waiting);
        assert_eq!(sync_state("error"), SyncState::Error);
        assert_eq!(sync_state("something new"), SyncState::Unknown);
    }

    #[test]
    fn a_summary_becomes_a_status() {
        let summary: FolderSummary = serde_json::from_str(
            r#"{"state":"syncing","error":"","errors":2,"globalFiles":10,"globalBytes":2048,
                "needFiles":3,"needBytes":512,"inSyncBytes":1536,"sequence":7}"#,
        )
        .unwrap();
        let status = folder_status("f", &summary);
        assert_eq!(status.state, SyncState::Syncing);
        assert_eq!(
            (status.need_files, status.need_bytes, status.errors),
            (3, 512, 2)
        );
        assert_eq!(status.error, None);
    }

    #[test]
    fn only_remote_changes_are_picked_from_events() {
        let events: Vec<Event> = serde_json::from_str(
            r#"[{"id":1,"type":"StateChanged","data":{"folder":"f","to":"idle"}},
                {"id":2,"type":"RemoteChangeDetected","data":{"folder":"f","path":"a.txt",
                 "action":"modified","type":"file","modifiedBy":"ABCDEFG"}},
                {"id":3,"type":"RemoteChangeDetected","data":{"folder":"f","path":"b.txt",
                 "action":"deleted","type":"file","modifiedBy":"ABCDEFG"}}]"#,
        )
        .unwrap();
        let changes = remote_changes(&events);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].modified_by, "ABCDEFG");
        assert!(!changes[0].deleted && changes[1].deleted);
    }
}
