use stellwerk_protocol::{DesiredState, DeviceId, Folder, FolderMode, Peer};

use super::*;
use crate::syncthing_config::{Config, FolderType};

const ME: &str = "MEMEMEM";
const HUB: &str = "HUBHUBH";
const LAPTOP: &str = "LAPTOPL";

fn peer(id: &str) -> Peer {
    Peer {
        device_id: DeviceId::new(id),
        name: id.to_lowercase(),
        addresses: vec![],
    }
}

fn folder(id: &str, mode: FolderMode, shared_with: &[&str]) -> Folder {
    Folder {
        id: id.into(),
        label: id.to_uppercase(),
        path: format!("/data/{id}"),
        mode,
        shared_with: shared_with.iter().map(|id| DeviceId::new(*id)).collect(),
    }
}

fn desired(peers: &[&str], folders: Vec<Folder>) -> DesiredState {
    DesiredState {
        revision: 1,
        peers: peers.iter().map(|id| peer(id)).collect(),
        folders,
    }
}

/// A fresh Syncthing: only its own device in the list.
fn fresh() -> Actual {
    Actual {
        my_id: ME.into(),
        config: Config {
            devices: vec![DeviceConfig {
                device_id: ME.into(),
                name: "host".into(),
                addresses: vec!["dynamic".into()],
            }],
            folders: vec![],
        },
    }
}

/// What Syncthing does with the calls, as far as the reconciler looks at it:
/// lib/config/config.go `SetDevice`/`SetFolder`/`RemoveFolder`/`RemoveDevice` and `prepare`.
fn simulate(mut actual: Actual, calls: &[ApiCall]) -> Actual {
    let config = &mut actual.config;
    for call in calls {
        match call {
            ApiCall::PutDevice(device) => {
                config.devices.retain(|d| d.device_id != device.device_id);
                config.devices.push(device.clone());
            }
            ApiCall::PutFolder(folder) => {
                let mut folder = folder.clone();
                folder.devices.retain(|d| {
                    config
                        .devices
                        .iter()
                        .any(|known| known.device_id == d.device_id)
                });
                config.folders.retain(|f| f.id != folder.id);
                config.folders.push(folder);
            }
            ApiCall::DeleteFolder(id) => config.folders.retain(|f| &f.id != id),
            ApiCall::DeleteDevice(id) => {
                config.devices.retain(|d| &d.device_id != id);
                for folder in &mut config.folders {
                    folder.devices.retain(|d| &d.device_id != id);
                }
            }
        }
    }
    config.folders.sort_by(|a, b| a.id.cmp(&b.id));
    actual
}

fn apply_twice(actual: Actual, desired: &DesiredState) -> (Vec<ApiCall>, Vec<ApiCall>) {
    let first = plan(&actual, desired).unwrap();
    let after = simulate(actual, &first);
    let second = plan(&after, desired).unwrap();
    (first, second)
}

#[test]
fn fresh_syncthing_gets_devices_then_folders() {
    let state = desired(
        &[HUB],
        vec![folder("fotos", FolderMode::SendReceive, &[HUB])],
    );

    let calls = plan(&fresh(), &state).unwrap();

    assert_eq!(calls.len(), 2);
    let ApiCall::PutDevice(device) = &calls[0] else {
        panic!("expected a device first, got {calls:?}");
    };
    assert_eq!(device.device_id, HUB);
    assert_eq!(device.addresses, ["dynamic"]);
    let ApiCall::PutFolder(folder) = &calls[1] else {
        panic!("expected the folder second, got {calls:?}");
    };
    assert_eq!(folder.folder_type, FolderType::SendReceive);
    let ids: Vec<_> = folder
        .devices
        .iter()
        .map(|d| d.device_id.as_str())
        .collect();
    assert_eq!(ids, [HUB, ME]);
}

#[test]
fn applying_the_same_state_twice_changes_nothing_the_second_time() {
    let state = desired(
        &[HUB, LAPTOP],
        vec![
            folder("fotos", FolderMode::ReceiveOnly, &[HUB]),
            folder("docs", FolderMode::SendOnly, &[HUB, LAPTOP]),
        ],
    );

    let (first, second) = apply_twice(fresh(), &state);

    assert_eq!(first.len(), 4);
    assert_eq!(second, []);
}

#[test]
fn a_changed_mode_is_one_folder_put() {
    let before = desired(
        &[HUB],
        vec![folder("fotos", FolderMode::SendReceive, &[HUB])],
    );
    let after = desired(
        &[HUB],
        vec![folder("fotos", FolderMode::ReceiveOnly, &[HUB])],
    );
    let actual = simulate(fresh(), &plan(&fresh(), &before).unwrap());

    let calls = plan(&actual, &after).unwrap();

    assert!(
        matches!(&calls[..], [ApiCall::PutFolder(f)] if f.folder_type == FolderType::ReceiveOnly),
        "{calls:?}"
    );
}

#[test]
fn a_changed_address_is_one_device_put() {
    let state = desired(&[HUB], vec![]);
    let actual = simulate(fresh(), &plan(&fresh(), &state).unwrap());
    let mut moved = state.clone();
    moved.peers[0].addresses = vec!["tcp://10.0.0.2:42000".into()];

    let calls = plan(&actual, &moved).unwrap();

    assert!(
        matches!(&calls[..], [ApiCall::PutDevice(d)] if d.addresses == ["tcp://10.0.0.2:42000"]),
        "{calls:?}"
    );
}

#[test]
fn a_new_path_removes_the_folder_and_adds_it_again() {
    let state = desired(
        &[HUB],
        vec![folder("fotos", FolderMode::SendReceive, &[HUB])],
    );
    let actual = simulate(fresh(), &plan(&fresh(), &state).unwrap());
    let mut moved = state.clone();
    moved.folders[0].path = "/elsewhere/fotos".into();

    let calls = plan(&actual, &moved).unwrap();

    assert!(
        matches!(&calls[..], [ApiCall::DeleteFolder(id), ApiCall::PutFolder(f)]
            if id == "fotos" && f.path == "/elsewhere/fotos"),
        "{calls:?}"
    );
}

#[test]
fn folders_and_devices_no_longer_wanted_are_removed_but_never_the_own_device() {
    let state = desired(
        &[HUB, LAPTOP],
        vec![
            folder("fotos", FolderMode::SendReceive, &[HUB]),
            folder("docs", FolderMode::SendReceive, &[LAPTOP]),
        ],
    );
    let actual = simulate(fresh(), &plan(&fresh(), &state).unwrap());
    let smaller = desired(
        &[HUB],
        vec![folder("fotos", FolderMode::SendReceive, &[HUB])],
    );

    let calls = plan(&actual, &smaller).unwrap();

    assert_eq!(
        calls,
        [
            ApiCall::DeleteFolder("docs".into()),
            ApiCall::DeleteDevice(LAPTOP.into())
        ]
    );
    let empty = plan(&simulate(actual, &calls), &DesiredState::default()).unwrap();
    assert!(
        !empty.contains(&ApiCall::DeleteDevice(ME.into())),
        "{empty:?}"
    );
}

#[test]
fn the_own_device_in_a_folder_and_device_order_do_not_count_as_a_difference() {
    let state = desired(
        &[LAPTOP, HUB],
        vec![folder("docs", FolderMode::SendReceive, &[LAPTOP, HUB])],
    );
    let mut actual = simulate(fresh(), &plan(&fresh(), &state).unwrap());
    actual.config.folders[0].devices.reverse();

    assert_eq!(plan(&actual, &state).unwrap(), []);
}

#[test]
fn invalid_states_are_errors_not_calls() {
    let unknown = desired(
        &[HUB],
        vec![folder("docs", FolderMode::SendReceive, &[LAPTOP])],
    );
    assert_eq!(
        plan(&fresh(), &unknown),
        Err(PlanError::UnknownPeer {
            folder: "docs".into(),
            peer: LAPTOP.into()
        })
    );

    let me = desired(&[ME], vec![]);
    assert_eq!(plan(&fresh(), &me), Err(PlanError::SelfAsPeer(ME.into())));

    let twice = desired(&[HUB, HUB], vec![]);
    assert_eq!(
        plan(&fresh(), &twice),
        Err(PlanError::DuplicatePeer(HUB.into()))
    );

    let folders = desired(
        &[],
        vec![
            folder("docs", FolderMode::SendReceive, &[]),
            folder("docs", FolderMode::SendOnly, &[]),
        ],
    );
    assert_eq!(
        plan(&fresh(), &folders),
        Err(PlanError::DuplicateFolder("docs".into()))
    );
}
