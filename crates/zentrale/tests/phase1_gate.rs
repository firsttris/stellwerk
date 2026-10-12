//! Gate of phase 1: Zentrale, hub and two agents. A file from A arrives on B through the hub;
//! B goes offline, A changes files and the Zentrale changes the configuration; when B comes
//! back it gets both.

mod common;

use std::path::Path;

use common::{pair_agent, start_agent, start_zentrale, wait_for, wait_for_file};
use stellwerk_protocol::FolderMode;
use stellwerk_zentrale::app::App;
use stellwerk_zentrale::model::Device;
use stellwerk_zentrale::store::{ShareChange, now};

fn share(folder: &str, device: &Device, mode: FolderMode, path: &Path) -> ShareChange {
    ShareChange {
        folder_id: folder.into(),
        device_id: device.id.clone(),
        mode: Some(mode),
        path: Some(path.to_string_lossy().into_owned()),
    }
}

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

async fn wait_applied(app: &App, device: &Device, revision: u64) {
    wait_for(
        &format!("{} to apply revision {revision}", device.name),
        || async {
            let applied = app.store().device(&device.id).await.ok()?.applied_rev?;
            (applied >= revision).then_some(())
        },
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_offline_device_catches_up_on_files_and_configuration() {
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

    // Both devices hold "Projekte"; a file from A arrives on B through the hub.
    let projekte = store.create_folder("Projekte", now()).await.unwrap();
    let a_projekte = tmp.path().join("a-projekte");
    let b_projekte = tmp.path().join("b-projekte");
    let revision = store
        .apply_shares(&[
            share(&projekte.id, &a, FolderMode::SendReceive, &a_projekte),
            share(&projekte.id, &b, FolderMode::SendReceive, &b_projekte),
        ])
        .await
        .unwrap();
    app.changed().await.unwrap();
    wait_applied(&app, &a, revision).await;
    wait_applied(&app, &b, revision).await;
    std::fs::create_dir_all(&a_projekte).unwrap();
    std::fs::write(a_projekte.join("plan.txt"), "v1\n").unwrap();
    wait_for_file(&b_projekte.join("plan.txt"), "v1\n").await;

    // B goes offline.
    agent_b.stop().await;
    wait_online(&app, &b, false).await;

    // Meanwhile A changes a file and adds one, and the admin adds a folder for both.
    std::fs::write(a_projekte.join("plan.txt"), "v2\n").unwrap();
    std::fs::write(a_projekte.join("neu.txt"), "new\n").unwrap();
    let fotos = store.create_folder("Fotos", now()).await.unwrap();
    let a_fotos = tmp.path().join("a-fotos");
    let b_fotos = tmp.path().join("b-fotos");
    let revision = store
        .apply_shares(&[
            share(&fotos.id, &a, FolderMode::SendOnly, &a_fotos),
            share(&fotos.id, &b, FolderMode::ReceiveOnly, &b_fotos),
        ])
        .await
        .unwrap();
    app.changed().await.unwrap();
    wait_applied(&app, &a, revision).await;
    std::fs::create_dir_all(&a_fotos).unwrap();
    std::fs::write(a_fotos.join("bild.txt"), "photo\n").unwrap();

    // The hub has everything while B is away.
    let hub_projekte = Path::new(&app.hub().folders_dir).join(&projekte.syncthing_folder_id);
    let hub_fotos = Path::new(&app.hub().folders_dir).join(&fotos.syncthing_folder_id);
    wait_for_file(&hub_projekte.join("plan.txt"), "v2\n").await;
    wait_for_file(&hub_fotos.join("bild.txt"), "photo\n").await;
    let b_before = store.device(&b.id).await.unwrap().applied_rev.unwrap();
    assert!(
        b_before < revision,
        "B was offline and has not applied the change"
    );

    // B comes back: new configuration and all changes arrive.
    let agent_b = start_agent(tmp.path().join("b")).await;
    wait_online(&app, &b, true).await;
    wait_applied(&app, &b, revision).await;
    wait_for_file(&b_projekte.join("plan.txt"), "v2\n").await;
    wait_for_file(&b_projekte.join("neu.txt"), "new\n").await;
    wait_for_file(&b_fotos.join("bild.txt"), "photo\n").await;

    agent_a.stop().await;
    agent_b.stop().await;
    zentrale.stop().await.unwrap();
}
