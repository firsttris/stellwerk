//! The folder picker and the search end to end: the request goes through the Zentrale to a
//! real agent and its listing of the device's directories comes back.

mod common;

use common::{pair_agent, start_zentrale, wait_for};
use stellwerk_zentrale::browse::BrowseError;

#[tokio::test(flavor = "multi_thread")]
async fn the_picker_lists_a_directory_on_a_connected_device() {
    let tmp = tempfile::tempdir().unwrap();
    let zentrale = start_zentrale(tmp.path()).await;
    let store = zentrale.app.store().clone();
    let agent = pair_agent(&zentrale, tmp.path().join("laptop")).await;
    let device = wait_for("the device to come online", || async {
        let device = store.config().await.ok()?.devices.into_iter().next()?;
        zentrale
            .app
            .online_devices()
            .contains_key(&device.id)
            .then_some(device)
    })
    .await;

    let pictures = tmp.path().join("Pictures");
    std::fs::create_dir_all(pictures.join("Holidays")).unwrap();
    std::fs::create_dir_all(pictures.join(".thumbnails")).unwrap();
    let listed = zentrale
        .app
        .browse(&device.id, pictures.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(listed.path, pictures.to_string_lossy());
    let names: Vec<&str> = listed.dirs.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Holidays"]);

    let missing = zentrale
        .app
        .browse(
            &device.id,
            pictures.join("gone").to_string_lossy().into_owned(),
        )
        .await;
    assert!(
        matches!(missing, Err(BrowseError::Failed(_))),
        "{missing:?}"
    );

    std::fs::create_dir_all(tmp.path().join("Media/Serien/Game.of.Thrones/S01")).unwrap();
    let found = zentrale
        .app
        .search(
            &device.id,
            tmp.path().to_string_lossy().into_owned(),
            "game of thrones".into(),
        )
        .await
        .unwrap();
    let paths: Vec<&str> = found.dirs.iter().map(|e| e.path.as_str()).collect();
    let thrones = tmp
        .path()
        .join("Media")
        .join("Serien")
        .join("Game.of.Thrones");
    assert_eq!(paths, [thrones.to_string_lossy()]);
    assert!(!found.truncated);

    agent.stop().await;
    wait_for("the device to go offline", || async {
        (!zentrale.app.online_devices().contains_key(&device.id)).then_some(())
    })
    .await;
    assert_eq!(
        zentrale.app.browse(&device.id, String::new()).await,
        Err(BrowseError::Offline)
    );
    zentrale.stop().await.unwrap();
}

/// The way there and back through a real agent: it says it can update, takes the request and
/// refuses its own version, and the reason arrives where the UI shows it.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn an_update_request_reaches_the_agent_and_its_answer_comes_back() {
    let tmp = tempfile::tempdir().unwrap();
    let zentrale = start_zentrale(tmp.path()).await;
    let store = zentrale.app.store().clone();
    let agent = pair_agent(&zentrale, tmp.path().join("laptop")).await;
    let device = wait_for("the agent to report that it can update", || async {
        let device = store.config().await.ok()?.devices.into_iter().next()?;
        let live = zentrale.app.live();
        live.devices
            .get(&device.id)
            .is_some_and(|d| d.can_update)
            .then_some(device)
    })
    .await;

    let refused = zentrale.app.update_agent(&device.id).await.unwrap_err();
    let BrowseError::Failed(reason) = refused else {
        panic!("{refused:?}");
    };
    assert!(reason.contains("not newer"), "{reason}");
    drop(agent);
}
