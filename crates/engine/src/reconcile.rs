//! The reconciler: actual Syncthing configuration + desired state → API calls.
//!
//! A pure function, so it is tested without Syncthing. Running the calls is [`crate::apply`].
//! It only ever changes configuration; removing a folder from Syncthing leaves its files alone.

use std::collections::{BTreeMap, BTreeSet};

use stellwerk_protocol::{DesiredState, Folder, Peer};

use crate::syncthing_config::{Actual, DeviceConfig, FolderConfig, FolderDevice};

/// One write to Syncthing's REST API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiCall {
    /// `PUT /rest/config/devices/{id}`
    PutDevice(DeviceConfig),
    /// `PUT /rest/config/folders/{id}`
    PutFolder(FolderConfig),
    /// `DELETE /rest/config/folders/{id}`
    DeleteFolder(String),
    /// `DELETE /rest/config/devices/{id}`
    DeleteDevice(String),
    /// `POST /rest/db/ignores?folder={id}`, after the folder exists.
    SetIgnores { folder: String, lines: Vec<String> },
}

impl ApiCall {
    /// The folder this call is about, if any.
    pub fn folder(&self) -> Option<&str> {
        match self {
            Self::PutFolder(folder) => Some(&folder.id),
            Self::DeleteFolder(id) | Self::SetIgnores { folder: id, .. } => Some(id),
            Self::PutDevice(_) | Self::DeleteDevice(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    #[error("the device's own ID {0} is listed as a peer")]
    SelfAsPeer(String),
    #[error("peer {0} is listed twice")]
    DuplicatePeer(String),
    #[error("folder {0} is listed twice")]
    DuplicateFolder(String),
    #[error("folder {folder} is shared with {peer}, which is not a peer")]
    UnknownPeer { folder: String, peer: String },
}

/// The calls that turn `actual` into `desired`, in an order Syncthing accepts: devices before
/// the folders that name them, folders removed before the devices they name. Empty when
/// nothing differs.
pub fn plan(actual: &Actual, desired: &DesiredState) -> Result<Vec<ApiCall>, PlanError> {
    validate(&actual.my_id, desired)?;
    let (put_devices, delete_devices) = plan_devices(actual, desired);
    let folders = plan_folders(actual, desired);
    let set_ignores = plan_ignores(actual, desired, &folders.fresh);
    Ok([
        put_devices,
        folders.delete,
        folders.put,
        set_ignores,
        delete_devices,
    ]
    .concat())
}

/// Devices to put and devices to delete; the own device is never touched.
fn plan_devices(actual: &Actual, desired: &DesiredState) -> (Vec<ApiCall>, Vec<ApiCall>) {
    let want: BTreeMap<_, _> = desired
        .peers
        .iter()
        .map(|peer| (peer.device_id.as_str(), device_config(peer)))
        .collect();
    let have: BTreeMap<_, _> = actual
        .config
        .devices
        .iter()
        .filter(|device| device.device_id != actual.my_id)
        .map(|device| (device.device_id.as_str(), device))
        .collect();
    let put = want
        .iter()
        .filter(|(id, want)| have.get(*id).is_none_or(|have| *have != *want))
        .map(|(_, want)| ApiCall::PutDevice(want.clone()))
        .collect();
    let delete = have
        .keys()
        .filter(|id| !want.contains_key(*id))
        .map(|id| ApiCall::DeleteDevice(id.to_string()))
        .collect();
    (put, delete)
}

struct FolderCalls<'a> {
    delete: Vec<ApiCall>,
    put: Vec<ApiCall>,
    /// Folders that are new to Syncthing after the plan: added, or removed and added again.
    fresh: BTreeSet<&'a str>,
}

fn plan_folders<'a>(actual: &Actual, desired: &'a DesiredState) -> FolderCalls<'a> {
    let want: BTreeMap<_, _> = desired
        .folders
        .iter()
        .map(|folder| (folder.id.as_str(), folder_config(&actual.my_id, folder)))
        .collect();
    let have: BTreeMap<_, _> = actual
        .config
        .folders
        .iter()
        .map(|folder| (folder.id.as_str(), folder))
        .collect();
    // A new path is a new folder: Syncthing would otherwise compare the new directory with the
    // index of the old one and announce every file as deleted.
    let fresh: BTreeSet<&str> = want
        .iter()
        .filter(|(id, want)| have.get(*id).is_none_or(|have| have.path != want.path))
        .map(|(id, _)| *id)
        .collect();
    let delete = have
        .iter()
        .filter(|(id, _)| !want.contains_key(*id) || fresh.contains(*id))
        .map(|(id, _)| ApiCall::DeleteFolder(id.to_string()))
        .collect();
    let put = want
        .iter()
        .filter(|(id, want)| {
            fresh.contains(*id) || have.get(*id).is_some_and(|have| !same_folder(have, want))
        })
        .map(|(_, want)| ApiCall::PutFolder(want.clone()))
        .collect();
    FolderCalls { delete, put, fresh }
}

/// Ignores live in the folder (.stignore), so a new or re-created folder always gets them set
/// after it exists: its directory may already hold a `.stignore` (a folder added again, a
/// directory left from a removed one) that Syncthing would load, and an empty list removes it
/// (lib/ignore/ignore.go, `WriteIgnores`).
fn plan_ignores(actual: &Actual, desired: &DesiredState, fresh: &BTreeSet<&str>) -> Vec<ApiCall> {
    desired
        .folders
        .iter()
        .filter_map(|folder| {
            let want = ignore_lines(&folder.ignore_patterns);
            let have = actual
                .ignores
                .get(&folder.id)
                .map_or(&[][..], Vec::as_slice);
            let differs = fresh.contains(folder.id.as_str()) || want != ignore_lines(have);
            differs.then(|| ApiCall::SetIgnores {
                folder: folder.id.clone(),
                lines: want,
            })
        })
        .collect()
}

fn validate(my_id: &str, desired: &DesiredState) -> Result<(), PlanError> {
    let mut peers = BTreeSet::new();
    for peer in &desired.peers {
        let id = peer.device_id.as_str();
        if id == my_id {
            return Err(PlanError::SelfAsPeer(id.to_string()));
        }
        if !peers.insert(id) {
            return Err(PlanError::DuplicatePeer(id.to_string()));
        }
    }

    let mut folders = BTreeSet::new();
    for folder in &desired.folders {
        if !folders.insert(folder.id.as_str()) {
            return Err(PlanError::DuplicateFolder(folder.id.clone()));
        }
        if let Some(peer) = folder
            .shared_with
            .iter()
            .find(|peer| !peers.contains(peer.as_str()))
        {
            return Err(PlanError::UnknownPeer {
                folder: folder.id.clone(),
                peer: peer.to_string(),
            });
        }
    }
    Ok(())
}

fn device_config(peer: &Peer) -> DeviceConfig {
    let addresses = if peer.addresses.is_empty() {
        vec!["dynamic".to_string()]
    } else {
        peer.addresses.clone()
    };
    DeviceConfig {
        device_id: peer.device_id.to_string(),
        name: peer.name.clone(),
        addresses,
    }
}

/// The folder as Syncthing stores it: shared with the device itself too, devices sorted
/// (lib/config/folderconfiguration.go, `prepare`).
fn folder_config(my_id: &str, folder: &Folder) -> FolderConfig {
    let devices: BTreeSet<&str> = folder
        .shared_with
        .iter()
        .map(|peer| peer.as_str())
        .chain([my_id])
        .collect();
    FolderConfig {
        id: folder.id.clone(),
        label: folder.label.clone(),
        path: folder.path.clone(),
        folder_type: folder.mode.into(),
        versioning: folder.versioning.into(),
        devices: devices
            .into_iter()
            .map(|id| FolderDevice {
                device_id: id.to_string(),
            })
            .collect(),
    }
}

/// Ignore lines as compared and written: trimmed and without empty lines, as Syncthing reads
/// `.stignore` and returns it from `GET /rest/db/ignores` (lib/ignore/ignore.go,
/// `parseIgnoreFile`: `strings.TrimSpace`).
fn ignore_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn same_folder(have: &FolderConfig, want: &FolderConfig) -> bool {
    let device_ids = |folder: &FolderConfig| -> BTreeSet<String> {
        folder
            .devices
            .iter()
            .map(|device| device.device_id.clone())
            .collect()
    };
    have.label == want.label
        && have.path == want.path
        && have.folder_type == want.folder_type
        && have.versioning == want.versioning
        && device_ids(have) == device_ids(want)
}

#[cfg(test)]
#[path = "reconcile_tests.rs"]
mod tests;
