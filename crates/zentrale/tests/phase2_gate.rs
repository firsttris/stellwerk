//! Gate of phase 2 (integration part): a real conflict between two devices is found on the hub
//! and resolved there, the result reaches both devices; an older version is restored from the
//! hub's versions and reaches them too.

mod common;

use std::path::Path;

use common::{pair_agent, start_agent, start_zentrale, wait_for, wait_for_file};
use stellwerk_engine::files::Keep;
use stellwerk_protocol::FolderMode;
use stellwerk_zentrale::app::App;
use stellwerk_zentrale::files;
use stellwerk_zentrale::live::view;
use stellwerk_zentrale::model::Device;
use stellwerk_zentrale::store::{ShareChange, now};

async fn device_named(app: &App, name: &str) -> Device {
    wait_for(&format!("{name} to pair"), || async {
        let config = app.store().config().await.ok()?;
        config.devices.into_iter().find(|d| d.name == name)
    })
    .await
}

async fn wait_online(app: &App, device: &Device, online: bool) {
    wait_for(&format!("{} online = {online}", device.name), || async {
        (app.online_devices().contains_key(&device.id) == online).then_some(())
    })
    .await;
}

/// Names and contents of the files in a folder, without Syncthing's own.
fn files_in(dir: &Path) -> Vec<(String, String)> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| !name.starts_with(".st"))
                .map(|name| {
                    let content = std::fs::read_to_string(dir.join(&name)).unwrap_or_default();
                    (name, content)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Waits until hub and both devices hold the same files and the hub sees both devices
/// complete: nothing is in flight any more.
async fn wait_converged(app: &App, folder_id: &str, devices: [&Device; 2], dirs: [&Path; 3]) {
    wait_for("the folder to converge", || async {
        let config = app.store().config().await.ok()?;
        let view = view(&config, &app.live(), &app.conflicts());
        let done = view.hub.completion.get(folder_id)?;
        let complete = devices
            .iter()
            .all(|d| done.get(&d.id).is_some_and(|c| c.percent >= 100.0));
        let [first, rest @ ..] = dirs.map(files_in);
        (complete && rest.iter().all(|other| *other == first)).then_some(())
    })
    .await;
}

fn conflict_copies(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| name.contains(".sync-conflict-"))
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_conflict_is_resolved_on_the_hub_and_a_version_restored() {
    let tmp = tempfile::tempdir().unwrap();
    let zentrale = start_zentrale(tmp.path()).await;
    let app = zentrale.app.clone();
    let store = app.store().clone();
    let agent_a = pair_agent(&zentrale, tmp.path().join("a")).await;
    let agent_b = pair_agent(&zentrale, tmp.path().join("b")).await;
    let a = device_named(&app, "a").await;
    let b = device_named(&app, "b").await;
    wait_online(&app, &a, true).await;
    wait_online(&app, &b, true).await;

    let folder = store.create_folder("Projekte", now()).await.unwrap();
    let a_dir = tmp.path().join("a-projekte");
    let b_dir = tmp.path().join("b-projekte");
    let share = |device: &Device, path: &Path| ShareChange {
        folder_id: folder.id.clone(),
        device_id: device.id.clone(),
        mode: Some(FolderMode::SendReceive),
        path: Some(path.to_string_lossy().into_owned()),
    };
    store
        .apply_shares(&[share(&a, &a_dir), share(&b, &b_dir)])
        .await
        .unwrap();
    app.changed().await.unwrap();
    wait_for("both folders to exist", || async {
        (a_dir.join(".stfolder").exists() && b_dir.join(".stfolder").exists()).then_some(())
    })
    .await;

    std::fs::write(a_dir.join("plan.txt"), "v1\n").unwrap();
    wait_for_file(&b_dir.join("plan.txt"), "v1\n").await;

    // A conflict: B edits offline while A edits online.
    agent_b.stop().await;
    wait_online(&app, &b, false).await;
    std::fs::write(a_dir.join("plan.txt"), "from A\n").unwrap();
    let hub_dir = Path::new(&app.hub().folders_dir).join(&folder.syncthing_folder_id);
    wait_for_file(&hub_dir.join("plan.txt"), "from A\n").await;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    std::fs::write(b_dir.join("plan.txt"), "edited on device B\n").unwrap();
    let agent_b = start_agent(tmp.path().join("b")).await;

    // The hub finds the conflict copy, once everyone has seen everything.
    wait_converged(&app, &folder.id, [&a, &b], [&hub_dir, &a_dir, &b_dir]).await;
    let conflict = wait_for("a conflict on the hub", || async {
        files::scan(&app).await.ok()?;
        app.conflicts().get(&folder.id)?.first().cloned()
    })
    .await;
    assert_eq!(conflict.original, "plan.txt");
    let content = files::content(&app, &folder.id, &conflict.path)
        .await
        .unwrap();
    let copy_text = content.copy.text.clone().unwrap();
    let original_text = content.original.clone().unwrap().text.unwrap();
    assert_ne!(copy_text, original_text);
    let events = store.events(50).await.unwrap();
    assert!(events.iter().any(|e| e.kind == "conflict"), "{events:?}");

    // Keep the copy: it becomes plan.txt everywhere. Syncthing may have made more than one
    // copy of the same version (it retries a failed pull); those are discarded.
    files::resolve(&app, &folder.id, &conflict.path, Keep::Copy)
        .await
        .unwrap();
    wait_for("every conflict copy to be resolved", || async {
        files::scan(&app).await.ok()?;
        let Some(next) = app
            .conflicts()
            .get(&folder.id)
            .and_then(|l| l.first().cloned())
        else {
            return Some(());
        };
        files::resolve(&app, &folder.id, &next.path, Keep::Original)
            .await
            .ok()?;
        None
    })
    .await;
    for dir in [&hub_dir, &a_dir, &b_dir] {
        wait_for_file(&dir.join("plan.txt"), &copy_text).await;
        wait_for(
            &format!("no conflict copies in {}", dir.display()),
            || async { conflict_copies(dir).is_empty().then_some(()) },
        )
        .await;
    }

    wait_converged(&app, &folder.id, [&a, &b], [&hub_dir, &a_dir, &b_dir]).await;

    // The discarded side is a version on the hub; restoring it brings it back everywhere. The
    // two sides have different lengths, so the size tells the versions apart.
    let version = wait_for("the discarded version to be listed", || async {
        let listing = files::versions(&app, &folder.id).await.ok()?;
        let plan = listing.into_iter().find(|f| f.path == "plan.txt")?;
        plan.versions
            .into_iter()
            .find(|v| v.size == original_text.len() as u64)
    })
    .await;
    files::restore(&app, &folder.id, "plan.txt", &version.version_time)
        .await
        .unwrap();
    for dir in [&a_dir, &b_dir] {
        wait_for_file(&dir.join("plan.txt"), &original_text).await;
    }

    agent_a.stop().await;
    agent_b.stop().await;
    zentrale.stop().await.unwrap();
}
