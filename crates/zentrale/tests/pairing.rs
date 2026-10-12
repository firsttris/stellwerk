//! An agent pairs with a one-time code, gets its folder, and its files reach the hub.

mod common;

use common::{pair_agent, start_zentrale, wait_for, wait_for_file};
use stellwerk_protocol::FolderMode;
use stellwerk_zentrale::desired::hub_path;
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
    // The code is used up: the stored config no longer carries it.
    let config = stellwerk_agent::config::load(&agent.data_dir).unwrap();
    assert_eq!(config.enrollment_code, None);

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

    agent.stop().await;
    zentrale.stop().await.unwrap();
}
