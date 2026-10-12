//! From the stored configuration to the desired state of each device and of the hub.
//!
//! Star topology: a device shares each of its folders with the hub only; the hub holds every
//! folder and shares it with every device that has it. Pure functions, tested without a
//! database or Syncthing.

use std::collections::BTreeSet;

use stellwerk_protocol::{DesiredState, DeviceId, Folder, FolderMode, Peer, Versioning};

use crate::model::{Config, Device, Hub};

/// The desired state of one device, `None` when it is unknown or blocked.
pub fn for_device(config: &Config, hub: &Hub, device_id: &str) -> Option<DesiredState> {
    let device = config
        .devices
        .iter()
        .find(|device| device.id == device_id && device.approved)?;
    let hub_id = DeviceId::new(&hub.syncthing_id);
    let folders = config
        .shares
        .iter()
        .filter(|share| share.device_id == device.id)
        .filter_map(|share| {
            let folder = config.folders.iter().find(|f| f.id == share.folder_id)?;
            Some(Folder {
                id: folder.syncthing_folder_id.clone(),
                label: folder.label.clone(),
                path: share.path.clone(),
                mode: share.mode,
                shared_with: vec![hub_id.clone()],
                versioning: Versioning::None,
                ignore_patterns: folder.ignore_patterns.clone(),
            })
        })
        .collect();
    Some(DesiredState {
        revision: config.revision,
        peers: vec![Peer {
            device_id: hub_id,
            name: "Stellwerk hub".into(),
            addresses: vec![hub.address.clone()],
        }],
        folders,
    })
}

/// The desired state of the hub: every approved device a peer, every folder present.
pub fn for_hub(config: &Config, hub: &Hub) -> DesiredState {
    let approved: Vec<&Device> = config
        .devices
        .iter()
        .filter(|device| device.approved && device.syncthing_id != hub.syncthing_id)
        .collect();
    let peers = approved
        .iter()
        .map(|device| Peer {
            device_id: DeviceId::new(&device.syncthing_id),
            name: device.name.clone(),
            addresses: vec![],
        })
        .collect();
    let folders = config
        .folders
        .iter()
        .map(|folder| {
            let shared_with: BTreeSet<&str> = config
                .shares
                .iter()
                .filter(|share| share.folder_id == folder.id)
                .filter_map(|share| approved.iter().find(|d| d.id == share.device_id))
                .map(|device| device.syncthing_id.as_str())
                .collect();
            Folder {
                id: folder.syncthing_folder_id.clone(),
                label: folder.label.clone(),
                path: hub_path(hub, &folder.syncthing_folder_id),
                mode: FolderMode::SendReceive,
                shared_with: shared_with.into_iter().map(DeviceId::new).collect(),
                versioning: folder.versioning,
                ignore_patterns: folder.ignore_patterns.clone(),
            }
        })
        .collect();
    DesiredState {
        revision: config.revision,
        peers,
        folders,
    }
}

/// Where the hub keeps a folder: by its Syncthing ID, so renaming a folder moves nothing.
pub fn hub_path(hub: &Hub, syncthing_folder_id: &str) -> String {
    let dir = hub.folders_dir.trim_end_matches(['/', '\\']);
    format!("{dir}/{syncthing_folder_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DEFAULT_HUB_VERSIONING, DeviceKind, Folder as StoredFolder, Share};

    fn device(id: &str, approved: bool) -> Device {
        Device {
            id: id.into(),
            name: id.into(),
            kind: DeviceKind::Desktop,
            os: "linux".into(),
            public_key: format!("key-{id}"),
            syncthing_id: format!("ST-{id}"),
            agent_version: "0.1.0".into(),
            syncthing_version: "v2.1.6".into(),
            approved,
            created_at: 0,
            last_seen: None,
            applied_rev: None,
        }
    }

    fn folder(id: &str) -> StoredFolder {
        StoredFolder {
            id: id.into(),
            label: id.to_uppercase(),
            syncthing_folder_id: format!("st-{id}"),
            versioning: DEFAULT_HUB_VERSIONING,
            ignore_patterns: vec!["*.tmp".into()],
            created_at: 0,
        }
    }

    fn share(folder: &str, device: &str, mode: FolderMode) -> Share {
        Share {
            folder_id: folder.into(),
            device_id: device.into(),
            mode,
            path: format!("~/{folder}"),
        }
    }

    fn hub() -> Hub {
        Hub {
            syncthing_id: "ST-HUB".into(),
            address: "tcp://hub:22001".into(),
            folders_dir: "/data/folders/".into(),
        }
    }

    fn config() -> Config {
        Config {
            revision: 7,
            devices: vec![
                device("a", true),
                device("b", true),
                device("blocked", false),
            ],
            folders: vec![folder("docs"), folder("fotos"), folder("empty")],
            shares: vec![
                share("docs", "a", FolderMode::SendReceive),
                share("docs", "b", FolderMode::ReceiveOnly),
                share("fotos", "a", FolderMode::SendOnly),
                share("fotos", "blocked", FolderMode::SendReceive),
            ],
        }
    }

    #[test]
    fn a_device_shares_its_folders_with_the_hub_only() {
        let state = for_device(&config(), &hub(), "b").unwrap();

        assert_eq!(state.revision, 7);
        assert_eq!(state.peers.len(), 1);
        assert_eq!(state.peers[0].device_id.as_str(), "ST-HUB");
        assert_eq!(state.peers[0].addresses, ["tcp://hub:22001"]);
        assert_eq!(state.folders.len(), 1);
        let docs = &state.folders[0];
        assert_eq!(docs.id, "st-docs");
        assert_eq!(docs.path, "~/docs");
        assert_eq!(docs.mode, FolderMode::ReceiveOnly);
        assert_eq!(docs.shared_with, [DeviceId::new("ST-HUB")]);
        assert_eq!(docs.versioning, Versioning::None);
        assert_eq!(docs.ignore_patterns, ["*.tmp"]);
    }

    #[test]
    fn blocked_and_unknown_devices_get_nothing() {
        assert_eq!(for_device(&config(), &hub(), "blocked"), None);
        assert_eq!(for_device(&config(), &hub(), "nope"), None);
    }

    #[test]
    fn the_hub_holds_every_folder_with_versions_and_knows_approved_devices() {
        let state = for_hub(&config(), &hub());

        let peers: Vec<_> = state.peers.iter().map(|p| p.device_id.as_str()).collect();
        assert_eq!(peers, ["ST-a", "ST-b"]);
        let ids: Vec<_> = state.folders.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["st-docs", "st-fotos", "st-empty"]);

        let docs = &state.folders[0];
        assert_eq!(docs.path, "/data/folders/st-docs");
        assert_eq!(docs.mode, FolderMode::SendReceive);
        assert_eq!(docs.versioning, DEFAULT_HUB_VERSIONING);
        let shared: Vec<_> = docs.shared_with.iter().map(|d| d.as_str()).collect();
        assert_eq!(shared, ["ST-a", "ST-b"]);

        // The blocked device is not in fotos, the empty folder still lives on the hub.
        assert_eq!(state.folders[1].shared_with, [DeviceId::new("ST-a")]);
        assert!(state.folders[2].shared_with.is_empty());
    }

    #[test]
    fn both_states_pass_the_reconcilers_checks() {
        use stellwerk_engine::syncthing_config::{Actual, Config as StConfig};
        let config = config();
        let empty = |id: &str| Actual {
            my_id: id.into(),
            config: StConfig::default(),
            ignores: Default::default(),
        };
        assert!(stellwerk_engine::plan(&empty("ST-HUB"), &for_hub(&config, &hub())).is_ok());
        let device = for_device(&config, &hub(), "a").unwrap();
        assert!(stellwerk_engine::plan(&empty("ST-a"), &device).is_ok());
    }
}
