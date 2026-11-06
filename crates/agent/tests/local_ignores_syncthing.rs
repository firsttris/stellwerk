//! `local_ignores::merge` against the pinned Syncthing: what is listed stays out of the index
//! whatever the Zentrale's patterns say.
//!
//! Downloads the pinned Syncthing on first run into `.syncthing-cache/`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use stellwerk_agent::local_ignores::merge;
use stellwerk_engine::download::ensure_syncthing;
use stellwerk_engine::process::{SpawnOptions, start};
use stellwerk_engine::reconcile;
use stellwerk_protocol::{DesiredState, Folder, FolderMode, Versioning};

const FOLDER: &str = "stellwerk-listed";

fn cache_dir() -> PathBuf {
    std::env::var_os("STELLWERK_SYNCTHING_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.syncthing-cache"))
}

/// A folder that keeps only saves: the usual use of a `!` pattern.
#[tokio::test(flavor = "multi_thread")]
async fn a_listed_file_stays_out_of_a_folder_that_keeps_its_kind() {
    let binary = ensure_syncthing(&cache_dir()).await.unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let instance = start(&SpawnOptions::isolated(binary, tmp.path().join("home")).unwrap())
        .await
        .unwrap();
    let client = instance.client();
    let data = tmp.path().join("Games");
    std::fs::create_dir_all(&data).unwrap();
    for name in ["listed.sav", "kept.sav", "other.txt"] {
        std::fs::write(data.join(name), name).unwrap();
    }
    let desired = DesiredState {
        revision: 1,
        peers: vec![],
        folders: vec![Folder {
            id: FOLDER.into(),
            label: "Games".into(),
            path: data.to_string_lossy().into_owned(),
            mode: FolderMode::SendOnly,
            shared_with: vec![],
            versioning: Versioning::None,
            ignore_patterns: vec!["!*.sav".into(), "*".into()],
        }],
    };
    let merged = merge(&desired, &[data.join("listed.sav")], None);
    let outcome = reconcile(client, &merged).await.unwrap();
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    client.scan(FOLDER).await.unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let indexed = loop {
        let summary = client.folder_summary(FOLDER).await.unwrap();
        if summary.state == "idle" && summary.global_files > 0 {
            break summary.global_files;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Syncthing did not finish scanning: {summary:?}"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    instance.stop().await.unwrap();
    assert_eq!(indexed, 1, "only kept.sav belongs in the index");
}
