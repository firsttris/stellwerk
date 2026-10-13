//! An agent pairs with a one-time code, gets its folder, and its files reach the hub.

mod common;

use common::{new_pairing_code, pair_agent, start_agent, start_zentrale, wait_for, wait_for_file};
use stellwerk_protocol::FolderMode;
use stellwerk_zentrale::desired::hub_path;
use stellwerk_zentrale::live::view;
use stellwerk_zentrale::pairing::code_hash;
use stellwerk_zentrale::store::{ShareChange, now};

#[tokio::test(flavor = "multi_thread")]
async fn a_paired_device_syncs_its_folder_with_the_hub() {
    let tmp = tempfile::tempdir().unwrap();
    let zentrale = start_zentrale(tmp.path()).await;
    let store = zentrale.app.store().clone();
    let agent = pair_agent(&zentrale, tmp.path().join("laptop")).await;

    let device = wait_for("the device to pair", || async {
        store.config().await.ok()?.devices.into_iter().next()
    })
    .await;
    assert_eq!(device.name, "laptop");
    wait_for("the device to be online", || async {
        zentrale
            .app
            .online_devices()
            .contains_key(&device.id)
            .then_some(())
    })
    .await;
    // The code is used up: the stored config no longer carries it. The agent drops it on
    // `welcome`, which the Zentrale sends after marking the device online, so wait for it.
    wait_for_code_dropped(&agent.data_dir).await;

    let folder = store.create_folder("Docs", now()).await.unwrap();
    let docs = tmp.path().join("laptop-docs");
    let revision = store
        .apply_shares(&[ShareChange {
            folder_id: folder.id.clone(),
            device_id: device.id.clone(),
            mode: Some(FolderMode::SendReceive),
            path: Some(docs.to_string_lossy().into_owned()),
        }])
        .await
        .unwrap();
    zentrale.app.changed().await.unwrap();
    wait_for("the agent to apply the new revision", || async {
        let applied = store.device(&device.id).await.ok()?.applied_rev?;
        (applied >= revision).then_some(())
    })
    .await;

    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("hello.txt"), "from the laptop\n").unwrap();
    let on_hub =
        std::path::PathBuf::from(hub_path(zentrale.app.hub(), &folder.syncthing_folder_id));
    wait_for_file(&on_hub.join("hello.txt"), "from the laptop\n").await;

    // Live state: the agent reports the folder in sync, the hub sees the device complete, and
    // the activity feed says who changed what.
    let app = &zentrale.app;
    wait_for("the agent's folder status", || async {
        let view = view(&store.config().await.ok()?, &app.live(), &app.conflicts());
        let docs = view.devices.get(&device.id)?.folders.get(&folder.id)?;
        (docs.global_files == 1 && docs.need_files == 0).then_some(())
    })
    .await;
    wait_for("the hub's completion for the device", || async {
        let view = view(&store.config().await.ok()?, &app.live(), &app.conflicts());
        let done = view.hub.completion.get(&folder.id)?.get(&device.id)?;
        (done.percent >= 100.0).then_some(())
    })
    .await;
    let change = wait_for("an activity entry for the change", || async {
        let events = store.events(20).await.ok()?;
        events.into_iter().find(|e| e.kind == "changes")
    })
    .await;
    assert_eq!(change.device_id.as_deref(), Some(device.id.as_str()));
    assert_eq!(change.folder_id.as_deref(), Some(folder.id.as_str()));
    assert_eq!(change.text, "1");

    agent.stop().await;
    zentrale.stop().await.unwrap();
}

/// `join` again with a fresh code on a paired device keeps its key: it comes back as the same
/// device instead of being turned away, and the pairing dialog sees the code used.
#[tokio::test(flavor = "multi_thread")]
async fn a_paired_device_can_join_again_with_a_new_code() {
    let tmp = tempfile::tempdir().unwrap();
    let zentrale = start_zentrale(tmp.path()).await;
    let store = zentrale.app.store().clone();
    let dir = tmp.path().join("laptop");
    let agent = pair_agent(&zentrale, dir.clone()).await;
    let device = wait_for("the device to pair", || async {
        store.config().await.ok()?.devices.into_iter().next()
    })
    .await;
    agent.stop().await;

    let code = new_pairing_code(&zentrale).await;
    stellwerk_agent::join(&dir, &format!("http://{}", zentrale.addr), Some(&code)).unwrap();
    let agent = start_agent(dir.clone()).await;
    wait_for("the new code to be used by the same device", || async {
        let used = store.enrolled_device(&code_hash(&code)).await.ok()??;
        (used == device.id).then_some(())
    })
    .await;
    wait_for("the device to be online again", || async {
        zentrale
            .app
            .online_devices()
            .contains_key(&device.id)
            .then_some(())
    })
    .await;
    assert_eq!(store.config().await.unwrap().devices.len(), 1);
    wait_for_code_dropped(&dir).await;

    agent.stop().await;
    zentrale.stop().await.unwrap();
}

async fn wait_for_code_dropped(data_dir: &std::path::Path) {
    wait_for("the agent to drop the used code", || async {
        let config = stellwerk_agent::config::load(data_dir).ok()?;
        config.enrollment_code.is_none().then_some(())
    })
    .await;
}
