//! LAN pairing: an agent without a code waits until the UI approves or rejects it.

mod common;

use common::{start_agent, start_zentrale, wait_for};
use stellwerk_zentrale::agents::decide;
use stellwerk_zentrale::waiting::Decision;

#[tokio::test(flavor = "multi_thread")]
async fn a_device_without_a_code_waits_for_approval() {
    let tmp = tempfile::tempdir().unwrap();
    let zentrale = start_zentrale(tmp.path()).await;
    let app = zentrale.app.clone();
    let url = format!("http://{}", zentrale.addr);

    let desk_dir = tmp.path().join("desk");
    let stranger_dir = tmp.path().join("stranger");
    let desk_print = stellwerk_agent::join(&desk_dir, &url, None, None).unwrap();
    let stranger_print = stellwerk_agent::join(&stranger_dir, &url, None, None).unwrap();
    let desk = start_agent(desk_dir).await;
    let stranger = start_agent(stranger_dir).await;

    // Both wait, with the fingerprints the agents printed; neither is a device yet.
    wait_for("both agents to wait", || async {
        let waiting = app.waiting();
        let prints: Vec<_> = waiting.iter().map(|w| w.fingerprint.as_str()).collect();
        (prints.contains(&desk_print.as_str()) && prints.contains(&stranger_print.as_str()))
            .then_some(())
    })
    .await;
    assert!(app.store().config().await.unwrap().devices.is_empty());

    assert!(
        decide(&app, &stranger_print, Decision::Reject)
            .await
            .unwrap()
    );
    assert!(decide(&app, &desk_print, Decision::Approve).await.unwrap());
    assert!(!decide(&app, &desk_print, Decision::Approve).await.unwrap());

    // The approved one becomes a device and connects on the same socket.
    let device = wait_for("the approved device", || async {
        let config = app.store().config().await.ok()?;
        config.devices.into_iter().find(|d| d.name == "desk")
    })
    .await;
    wait_for("the device to be online", || async {
        app.online_devices().contains_key(&device.id).then_some(())
    })
    .await;
    let events = app.store().events(20).await.unwrap();
    assert!(events.iter().any(|e| e.kind == "paired"), "{events:?}");

    // The rejected one is not a device, and does not come back to the list on its retry.
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let config = app.store().config().await.unwrap();
    assert_eq!(config.devices.len(), 1);
    assert!(app.waiting().is_empty(), "{:?}", app.waiting());

    desk.stop().await;
    stranger.stop().await;
    zentrale.stop().await.unwrap();
}
