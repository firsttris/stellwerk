//! What the MCP tools show and plan, as pure functions of the configuration and the live state:
//! devices and folders by name, and a draft of shares checked and described before it is
//! applied.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use stellwerk_protocol::FolderMode;
use stellwerk_protocol::status::SyncState;

use crate::live::LiveView;
use crate::model::{Config, Device, Folder};
use crate::store::{ShareChange, default_path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSummary {
    pub id: String,
    pub name: String,
    pub os: String,
    pub online: bool,
    pub blocked: bool,
    /// Seconds since the Unix epoch.
    pub last_seen: Option<i64>,
}

pub fn devices(config: &Config, online: &BTreeMap<String, i64>) -> Vec<DeviceSummary> {
    config
        .devices
        .iter()
        .map(|device| DeviceSummary {
            id: device.id.clone(),
            name: device.name.clone(),
            os: device.os.clone(),
            online: online.contains_key(&device.id),
            blocked: !device.approved,
            last_seen: device.last_seen,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareSummary {
    pub device: String,
    pub device_id: String,
    pub mode: FolderMode,
    pub path: String,
    /// As the device last reported it; `None` before it did.
    pub state: Option<SyncState>,
    /// What the device still lacks, as the server sees it.
    pub need_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderSummary {
    pub id: String,
    pub label: String,
    /// The size of the server's full copy, without versions; `None` before it knows.
    pub size_bytes: Option<u64>,
    pub files: Option<u64>,
    pub ignore_patterns: Vec<String>,
    pub shares: Vec<ShareSummary>,
}

pub fn folders(config: &Config, live: &LiveView) -> Vec<FolderSummary> {
    config
        .folders
        .iter()
        .map(|folder| {
            let hub = live.hub.folders.get(&folder.id);
            FolderSummary {
                id: folder.id.clone(),
                label: folder.label.clone(),
                size_bytes: hub.map(|h| h.global_bytes),
                files: hub.map(|h| h.global_files),
                ignore_patterns: folder.ignore_patterns.clone(),
                shares: shares_of(config, live, folder),
            }
        })
        .collect()
}

fn shares_of(config: &Config, live: &LiveView, folder: &Folder) -> Vec<ShareSummary> {
    config
        .shares
        .iter()
        .filter(|share| share.folder_id == folder.id)
        .filter_map(|share| {
            let device = config.devices.iter().find(|d| d.id == share.device_id)?;
            let reported = live
                .devices
                .get(&device.id)
                .and_then(|d| d.folders.get(&folder.id));
            let completion = live
                .hub
                .completion
                .get(&folder.id)
                .and_then(|c| c.get(&device.id));
            Some(ShareSummary {
                device: device.name.clone(),
                device_id: device.id.clone(),
                mode: share.mode,
                path: share.path.clone(),
                state: reported.map(|r| r.state),
                need_bytes: completion.map(|c| c.need_bytes),
            })
        })
        .collect()
}

/// The device with this ID, or else this name (ignoring case).
pub fn find_device<'a>(config: &'a Config, wanted: &str) -> Result<&'a Device, String> {
    find(&config.devices, wanted, |d| &d.id, |d| &d.name, "device")
}

/// The folder with this ID, or else this name (ignoring case).
pub fn find_folder<'a>(config: &'a Config, wanted: &str) -> Result<&'a Folder, String> {
    find(&config.folders, wanted, |f| &f.id, |f| &f.label, "folder")
}

fn find<'a, T>(
    items: &'a [T],
    wanted: &str,
    id: impl Fn(&T) -> &String,
    name: impl Fn(&T) -> &String,
    what: &str,
) -> Result<&'a T, String> {
    let wanted = wanted.trim();
    if let Some(item) = items.iter().find(|item| id(item) == wanted) {
        return Ok(item);
    }
    let named: Vec<&T> = items
        .iter()
        .filter(|item| name(item).eq_ignore_ascii_case(wanted))
        .collect();
    match named.as_slice() {
        [item] => Ok(item),
        [] => Err(format!("no {what} named \"{wanted}\"")),
        _ => Err(format!(
            "more than one {what} is named \"{wanted}\"; use its ID"
        )),
    }
}

/// How a device should hold a folder, as a tool takes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Wanted {
    SendReceive,
    SendOnly,
    ReceiveOnly,
    Off,
}

impl Wanted {
    fn mode(self) -> Option<FolderMode> {
        match self {
            Self::SendReceive => Some(FolderMode::SendReceive),
            Self::SendOnly => Some(FolderMode::SendOnly),
            Self::ReceiveOnly => Some(FolderMode::ReceiveOnly),
            Self::Off => None,
        }
    }
}

/// One cell of a draft as a tool takes it: folder and device by ID or name.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DraftChange {
    pub folder: String,
    pub device: String,
    pub mode: Wanted,
    #[serde(default)]
    pub path: Option<String>,
}

/// A device's part in a folder: mode and path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Holding {
    pub mode: FolderMode,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedChange {
    pub folder: String,
    pub device: String,
    pub before: Option<Holding>,
    pub after: Option<Holding>,
    pub device_online: bool,
    /// What the store gets; not shown.
    #[serde(skip)]
    pub change: ShareChange,
}

impl PlannedChange {
    pub fn changes_something(&self) -> bool {
        self.before != self.after
    }
}

/// Checks a draft against the configuration and works out what each cell becomes, with the
/// same default path the store gives a new share.
pub fn plan(
    config: &Config,
    online: &BTreeMap<String, i64>,
    draft: &[DraftChange],
) -> Result<Vec<PlannedChange>, String> {
    if draft.is_empty() {
        return Err("the draft has no changes".into());
    }
    let planned = draft
        .iter()
        .map(|cell| plan_one(config, online, cell))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, change) in planned.iter().enumerate() {
        let twice = planned[..index].iter().any(|earlier| {
            earlier.change.folder_id == change.change.folder_id
                && earlier.change.device_id == change.change.device_id
        });
        if twice {
            return Err(format!(
                "{} on {} appears twice",
                change.folder, change.device
            ));
        }
    }
    Ok(planned)
}

fn plan_one(
    config: &Config,
    online: &BTreeMap<String, i64>,
    cell: &DraftChange,
) -> Result<PlannedChange, String> {
    let folder = find_folder(config, &cell.folder)?;
    let device = find_device(config, &cell.device)?;
    if !device.approved {
        return Err(format!("{} is blocked", device.name));
    }
    let before = config
        .shares
        .iter()
        .find(|s| s.folder_id == folder.id && s.device_id == device.id)
        .map(|s| Holding {
            mode: s.mode,
            path: s.path.clone(),
        });
    let path = cell
        .path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty());
    let after = cell.mode.mode().map(|mode| Holding {
        mode,
        path: match (path, &before) {
            (Some(path), _) => path.to_string(),
            (None, Some(before)) => before.path.clone(),
            (None, None) => default_path(&folder.label),
        },
    });
    Ok(PlannedChange {
        folder: folder.label.clone(),
        device: device.name.clone(),
        device_online: online.contains_key(&device.id),
        change: ShareChange {
            folder_id: folder.id.clone(),
            device_id: device.id.clone(),
            mode: cell.mode.mode(),
            path: after.as_ref().map(|holding| holding.path.clone()),
        },
        before,
        after,
    })
}

fn mode_words(mode: FolderMode) -> &'static str {
    match mode {
        FolderMode::SendReceive => "sends and receives",
        FolderMode::SendOnly => "only sends",
        FolderMode::ReceiveOnly => "only receives",
    }
}

/// The draft in sentences, with what the person should know before applying it.
pub fn describe(planned: &[PlannedChange], live: &LiveView, config: &Config) -> Vec<String> {
    let mut lines: Vec<String> = planned.iter().map(describe_one).collect();
    for folder in touched_folders(planned, config) {
        let size = live.hub.folders.get(&folder.id).map(|h| h.global_bytes);
        lines.push(match size {
            Some(bytes) => format!(
                "{}: about {} now; the server keeps a full copy plus a year of versions, and \
                 every receiving device needs the same space.",
                folder.label,
                human_bytes(bytes)
            ),
            None => format!(
                "{}: size not known yet (the server learns it once a sending device has \
                 scanned it); the server keeps a full copy plus a year of versions.",
                folder.label
            ),
        });
    }
    lines
}

fn describe_one(change: &PlannedChange) -> String {
    let what = match (&change.before, &change.after) {
        (before, Some(after)) if before.as_ref() == Some(after) => {
            format!("unchanged ({} at {})", mode_words(after.mode), after.path)
        }
        (_, Some(after)) => format!("{} at {}", mode_words(after.mode), after.path),
        (Some(_), None) => "stops syncing it (its files stay)".to_string(),
        (None, None) => "unchanged (not syncing it)".to_string(),
    };
    let offline = if change.device_online || !change.changes_something() {
        ""
    } else {
        " – offline, applies when it is back"
    };
    format!("{} on {}: {what}{offline}", change.folder, change.device)
}

fn touched_folders<'a>(planned: &[PlannedChange], config: &'a Config) -> Vec<&'a Folder> {
    config
        .folders
        .iter()
        .filter(|folder| {
            planned
                .iter()
                .any(|p| p.change.folder_id == folder.id && p.after.is_some())
        })
        .collect()
}

/// "1.4 GB", in powers of 1000 as file managers show sizes.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
#[path = "views_tests.rs"]
mod tests;
