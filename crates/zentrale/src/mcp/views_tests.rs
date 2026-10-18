use std::collections::BTreeMap;

use stellwerk_protocol::status::SyncState;
use stellwerk_protocol::{FolderMode, Versioning};

use super::*;
use crate::live::FolderView;
use crate::model::{DeviceKind, Share};

fn device(id: &str, name: &str) -> Device {
    Device {
        id: id.into(),
        name: name.into(),
        kind: DeviceKind::Desktop,
        os: "linux".into(),
        public_key: format!("key-{id}"),
        syncthing_id: format!("ST-{id}"),
        agent_version: "0.1.9".into(),
        syncthing_version: "v2".into(),
        approved: true,
        created_at: 0,
        last_seen: Some(5),
        applied_rev: Some(1),
    }
}

fn folder(id: &str, label: &str) -> Folder {
    Folder {
        id: id.into(),
        label: label.into(),
        syncthing_folder_id: format!("st-{id}"),
        versioning: Versioning::default(),
        ignore_patterns: vec![],
        created_at: 0,
    }
}

/// A NAS that holds "Serien" and sends it, a laptop without it, and a blocked old PC.
fn config() -> Config {
    let mut old = device("old", "old-pc");
    old.approved = false;
    Config {
        revision: 3,
        devices: vec![device("nas", "NAS"), device("laptop", "laptop"), old],
        folders: vec![folder("serien", "Serien"), folder("games", "Games")],
        shares: vec![Share {
            folder_id: "serien".into(),
            device_id: "nas".into(),
            mode: FolderMode::SendOnly,
            path: "/mnt/media/Serien".into(),
        }],
    }
}

fn online() -> BTreeMap<String, i64> {
    BTreeMap::from([("nas".to_string(), 1)])
}

fn change(folder: &str, device: &str, mode: Wanted, path: Option<&str>) -> DraftChange {
    DraftChange {
        folder: folder.into(),
        device: device.into(),
        mode,
        path: path.map(str::to_string),
    }
}

fn live_with_size(folder: &str, bytes: u64) -> LiveView {
    let mut live = LiveView::default();
    live.hub.folders.insert(
        folder.into(),
        FolderView {
            state: SyncState::Idle,
            global_files: 10,
            global_bytes: bytes,
            need_files: 0,
            need_bytes: 0,
            errors: 0,
            error: None,
        },
    );
    live
}

#[test]
fn devices_and_folders_are_found_by_id_or_name() {
    let config = config();
    assert_eq!(find_device(&config, "laptop").unwrap().id, "laptop");
    assert_eq!(find_device(&config, " nas ").unwrap().id, "nas");
    assert_eq!(find_folder(&config, "serien").unwrap().label, "Serien");
    assert!(find_device(&config, "phone").unwrap_err().contains("phone"));

    let mut twins = config.clone();
    twins.devices.push(device("nas2", "nas"));
    assert!(find_device(&twins, "NAS").unwrap_err().contains("its ID"));
    assert_eq!(find_device(&twins, "nas2").unwrap().name, "nas");
}

#[test]
fn a_plan_fills_in_paths_like_the_store() {
    let config = config();
    let planned = plan(
        &config,
        &online(),
        &[
            change("Serien", "laptop", Wanted::ReceiveOnly, None),
            change("Serien", "NAS", Wanted::SendOnly, Some(" ")),
            change("Games", "laptop", Wanted::SendReceive, Some("D:\\Games")),
        ],
    )
    .unwrap();

    assert_eq!(planned[0].before, None);
    let laptop = planned[0].after.as_ref().unwrap();
    assert_eq!(laptop.mode, FolderMode::ReceiveOnly);
    assert_eq!(laptop.path, "~/Stellwerk/Serien");
    assert!(!planned[0].device_online);
    assert_eq!(
        planned[0].change,
        ShareChange {
            folder_id: "serien".into(),
            device_id: "laptop".into(),
            mode: Some(FolderMode::ReceiveOnly),
            path: Some("~/Stellwerk/Serien".into()),
        }
    );
    // An empty path keeps the current one: nothing changes.
    assert!(!planned[1].changes_something());
    assert_eq!(planned[2].after.as_ref().unwrap().path, "D:\\Games");
}

#[test]
fn a_plan_refuses_what_cannot_be_applied() {
    let config = config();
    let refused = |draft: &[DraftChange]| plan(&config, &online(), draft).unwrap_err();
    assert!(refused(&[]).contains("no changes"));
    assert!(refused(&[change("Filme", "laptop", Wanted::Off, None)]).contains("Filme"));
    assert!(refused(&[change("Games", "old-pc", Wanted::SendOnly, None)]).contains("blocked"));
    let twice = [
        change("Games", "laptop", Wanted::SendOnly, None),
        change("games", "LAPTOP", Wanted::Off, None),
    ];
    assert!(refused(&twice).contains("twice"));
}

#[test]
fn a_description_says_what_happens_and_what_it_costs() {
    let config = config();
    let planned = plan(
        &config,
        &online(),
        &[
            change("Serien", "laptop", Wanted::ReceiveOnly, None),
            change("Serien", "NAS", Wanted::Off, None),
            change("Games", "NAS", Wanted::SendOnly, Some("/srv/games")),
        ],
    )
    .unwrap();
    let lines = describe(
        &planned,
        &live_with_size("serien", 412_000_000_000),
        &config,
    );
    assert_eq!(
        lines[0],
        "Serien on laptop: only receives at ~/Stellwerk/Serien – offline, applies when it is back"
    );
    assert_eq!(lines[1], "Serien on NAS: stops syncing it (its files stay)");
    assert_eq!(lines[2], "Games on NAS: only sends at /srv/games");
    assert!(
        lines[3].starts_with("Serien: about 412.0 GB now"),
        "{}",
        lines[3]
    );
    assert!(
        lines[4].starts_with("Games: size not known yet"),
        "{}",
        lines[4]
    );
    assert_eq!(lines.len(), 5);
}

#[test]
fn folders_show_size_shares_and_state() {
    let config = config();
    let mut live = live_with_size("serien", 2_000);
    live.devices.insert(
        "nas".into(),
        crate::live::DeviceLiveView {
            at: 1,
            folders: BTreeMap::from([("serien".to_string(), live.hub.folders["serien"].clone())]),
        },
    );
    let summary = folders(&config, &live);
    assert_eq!(summary[0].size_bytes, Some(2_000));
    assert_eq!(summary[0].shares[0].device, "NAS");
    assert_eq!(summary[0].shares[0].state, Some(SyncState::Idle));
    assert_eq!(summary[1].size_bytes, None);
    assert!(summary[1].shares.is_empty());

    let devices = devices(&config, &online());
    assert!(devices[0].online);
    assert!(!devices[1].online);
    assert!(devices[2].blocked);
}

#[test]
fn sizes_read_like_a_file_manager() {
    assert_eq!(human_bytes(999), "999 B");
    assert_eq!(human_bytes(1_500), "1.5 KB");
    assert_eq!(human_bytes(412_000_000_000), "412.0 GB");
    assert_eq!(human_bytes(3_200_000_000_000_000), "3200.0 TB");
}
