//! Patterns from `ignore::anchored_pattern` against the pinned Syncthing: each excludes exactly
//! the one path it was made for, even with characters Syncthing's glob reads specially, and
//! nothing next to it. This is what keeps saves another program syncs out of a folder.
//!
//! Downloads the pinned Syncthing on first run into `.syncthing-cache/` (see `phase0_gate.rs`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use stellwerk_engine::download::ensure_syncthing;
use stellwerk_engine::ignore::anchored_pattern;
use stellwerk_engine::process::{SpawnOptions, start};
use stellwerk_engine::reconcile;
use stellwerk_protocol::{DesiredState, Folder, FolderMode, Versioning};

const FOLDER: &str = "stellwerk-anchored";

fn cache_dir() -> PathBuf {
    std::env::var_os("STELLWERK_SYNCTHING_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.syncthing-cache"))
}

fn write(root: &Path, rel: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, rel).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_anchored_pattern_excludes_exactly_its_path() {
    let binary = ensure_syncthing(&cache_dir()).await.unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let instance = start(&SpawnOptions::isolated(binary, tmp.path().join("home")).unwrap())
        .await
        .unwrap();
    let client = instance.client();
    let data = tmp.path().join("Games");

    let excluded = [
        "Hades/prefix/drive_c/users/steamuser/Save [1] {a,b}.sav",
        "Hades/star*.sav",
        r"Hades/back\slash.sav",
    ];
    let kept = [
        "Hades/prefix/drive_c/users/steamuser/Save 1 a.sav",
        "Hades/starX.sav",
        "Hades/other.sav",
        "Other/Hades/star*.sav",
    ];
    for rel in excluded.iter().chain(&kept) {
        write(&data, rel);
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
            ignore_patterns: excluded
                .iter()
                .map(|rel| anchored_pattern(rel).unwrap())
                .collect(),
        }],
    };
    let outcome = reconcile(client, &desired).await.unwrap();
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    client.scan(FOLDER).await.unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let summary = client.folder_summary(FOLDER).await.unwrap();
        if summary.state == "idle" && summary.global_files == kept.len() as u64 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "expected {} files in the index, Syncthing has {summary:?}",
            kept.len()
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    // Scanned again, still only the kept files: the count was not caught half way.
    client.scan(FOLDER).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        client.folder_summary(FOLDER).await.unwrap().global_files,
        kept.len() as u64
    );
    instance.stop().await.unwrap();
}
