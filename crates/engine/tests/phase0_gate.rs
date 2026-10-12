//! Gate of phase 0: two local Syncthing instances, coupled only by the reconciler; a file from
//! A arrives in B, and applying the same desired state a second time changes nothing.
//!
//! Downloads the pinned Syncthing on first run into `.syncthing-cache/` at the repository root
//! (or `$STELLWERK_SYNCTHING_CACHE`). All ports are random on 127.0.0.1.

use std::path::{Path, PathBuf};
use std::time::Duration;

use stellwerk_engine::download::ensure_syncthing;
use stellwerk_engine::monitor::{Options, Snapshot, remote_changes, snapshot};
use stellwerk_engine::process::{Instance, SpawnOptions, start};
use stellwerk_engine::syncthing_config::FolderType;
use stellwerk_engine::{Client, reconcile};
use stellwerk_protocol::{DesiredState, DeviceId, Folder, FolderMode, Peer, Versioning};

const FOLDER: &str = "stellwerk-phase0";
const SYNC_TIMEOUT: Duration = Duration::from_secs(120);

fn cache_dir() -> PathBuf {
    std::env::var_os("STELLWERK_SYNCTHING_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.syncthing-cache"))
}

async fn start_instance(binary: &Path, home: PathBuf) -> Instance {
    let options = SpawnOptions::isolated(binary.to_path_buf(), home).unwrap();
    start(&options).await.unwrap()
}

/// The state one device should have: the other as its only peer, the folder shared with it.
fn desired(
    other: &DeviceId,
    other_address: &str,
    path: &Path,
    mode: FolderMode,
    versioning: Versioning,
) -> DesiredState {
    DesiredState {
        revision: 1,
        peers: vec![Peer {
            device_id: other.clone(),
            name: "other".into(),
            addresses: vec![other_address.to_string()],
        }],
        folders: vec![Folder {
            id: FOLDER.into(),
            label: "Phase 0".into(),
            path: path.to_string_lossy().into_owned(),
            mode,
            shared_with: vec![other.clone()],
            versioning,
            ignore_patterns: vec!["*.tmp".into(), "build/".into()],
        }],
    }
}

/// Reconciles and returns how many calls it took; every call must have gone through.
async fn calls_made(client: &Client, desired: &DesiredState) -> usize {
    let outcome = reconcile(client, desired).await.unwrap();
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    outcome.calls.len()
}

async fn wait_for_snapshot(
    client: &Client,
    options: Options,
    done: impl Fn(&Snapshot) -> bool,
) -> Snapshot {
    let deadline = tokio::time::Instant::now() + SYNC_TIMEOUT;
    loop {
        let found = snapshot(client, options).await.unwrap();
        if done(&found) {
            return found;
        }
        assert!(tokio::time::Instant::now() < deadline, "{found:?}");
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn folder_type(client: &Client) -> FolderType {
    let config = client.config().await.unwrap();
    let folder = config.folders.iter().find(|f| f.id == FOLDER).unwrap();
    folder.folder_type
}

async fn wait_for_file(path: &Path, contents: &str) {
    let deadline = tokio::time::Instant::now() + SYNC_TIMEOUT;
    loop {
        if std::fs::read_to_string(path).is_ok_and(|found| found == contents) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{} did not arrive within {SYNC_TIMEOUT:?}",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn file_from_a_arrives_in_b_and_reapplying_changes_nothing() {
    let binary = ensure_syncthing(&cache_dir()).await.unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let a = start_instance(&binary, tmp.path().join("a/home")).await;
    let b = start_instance(&binary, tmp.path().join("b/home")).await;
    let a_id = DeviceId::new(a.client().my_id().await.unwrap());
    let b_id = DeviceId::new(b.client().my_id().await.unwrap());
    let a_data = tmp.path().join("a/data");
    let b_data = tmp.path().join("b/data");

    let want_a = desired(
        &b_id,
        b.listen_address(),
        &a_data,
        FolderMode::SendOnly,
        Versioning::None,
    );
    let want_b = desired(
        &a_id,
        a.listen_address(),
        &b_data,
        FolderMode::ReceiveOnly,
        Versioning::Staggered { max_age_days: 30 },
    );

    // First apply: device and folder are created, folder type and ignores are set.
    assert_eq!(calls_made(a.client(), &want_a).await, 3);
    assert_eq!(calls_made(b.client(), &want_b).await, 3);
    assert_eq!(
        a.client().ignores(FOLDER).await.unwrap(),
        ["*.tmp", "build/"]
    );
    assert_eq!(folder_type(a.client()).await, FolderType::SendOnly);
    assert_eq!(folder_type(b.client()).await, FolderType::ReceiveOnly);

    // Second apply of the same state: nothing to do, configuration untouched.
    let before = (
        a.client().config_raw().await.unwrap(),
        b.client().config_raw().await.unwrap(),
    );
    assert_eq!(calls_made(a.client(), &want_a).await, 0);
    assert_eq!(calls_made(b.client(), &want_b).await, 0);
    let after = (
        a.client().config_raw().await.unwrap(),
        b.client().config_raw().await.unwrap(),
    );
    assert_eq!(before, after);

    // Syncthing buffers events per mask only from the first request on (lib/api/api.go,
    // getEventSub), so subscribe before anything happens.
    b.client()
        .events(0, "RemoteChangeDetected", 0)
        .await
        .unwrap();

    // A file written on A arrives on B.
    std::fs::create_dir_all(&a_data).unwrap();
    let contents = "Stellwerk phase 0\n";
    std::fs::write(a_data.join("hello.txt"), contents).unwrap();
    std::fs::write(a_data.join("scratch.tmp"), "ignored").unwrap();
    a.client().scan(FOLDER).await.unwrap();
    wait_for_file(&b_data.join("hello.txt"), contents).await;
    assert!(
        !b_data.join("scratch.tmp").exists(),
        "ignored files stay where they are"
    );

    // The monitor sees the result: B has the file, A knows B is complete, and B saw the
    // change arrive from A.
    let b_snapshot = wait_for_snapshot(b.client(), Options::default(), |s| {
        s.folders
            .iter()
            .any(|f| f.folder == FOLDER && f.global_files == 1 && f.need_files == 0)
    })
    .await;
    assert_eq!(b_snapshot.completion, []);
    let a_snapshot = wait_for_snapshot(a.client(), Options { completion: true }, |s| {
        s.completion
            .iter()
            .any(|c| c.device == b_id.as_str() && c.percent >= 100.0)
    })
    .await;
    assert_eq!(a_snapshot.completion.len(), 1);
    let events = b
        .client()
        .events(0, "RemoteChangeDetected", 0)
        .await
        .unwrap();
    let changes = remote_changes(&events);
    assert!(
        changes
            .iter()
            .any(|c| c.path == "hello.txt" && a_id.as_str().starts_with(&c.modified_by)),
        "{changes:?}"
    );

    // A Syncthing that exits on its own (it runs with --no-restart) is noticed.
    let mut b = b;
    b.client().shutdown().await.unwrap();
    tokio::time::timeout(SYNC_TIMEOUT, b.exited())
        .await
        .expect("Syncthing exits after a shutdown")
        .unwrap();

    a.stop().await.unwrap();
}
