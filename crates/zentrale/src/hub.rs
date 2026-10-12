//! The hub Syncthing: started by the Zentrale and kept at its desired state with the same
//! reconciler the agents use.

use std::time::Duration;

use stellwerk_engine::Client;
use stellwerk_engine::monitor::{self, Snapshot};
use stellwerk_protocol::status::RemoteChange;
use tokio::sync::{mpsc, watch};

use crate::app::App;
use crate::live::summarize_changes;
use crate::{desired, store};

/// Changes arriving at the hub are collected this long before one activity entry per device
/// and folder is written ("laptop changed 18 files in Projekte").
pub const ACTIVITY_WINDOW: Duration = Duration::from_secs(30);

/// Reconciles the hub now and after every new revision, until the app goes away.
pub async fn run(app: App, client: Client) {
    let mut revisions = app.revisions();
    revisions.mark_changed();
    while revisions.changed().await.is_ok() {
        reconcile(&app, &client).await;
    }
}

async fn reconcile(app: &App, client: &Client) {
    let config = match app.store().config().await {
        Ok(config) => config,
        Err(err) => return tracing::error!(%err, "could not read the configuration"),
    };
    let state = desired::for_hub(&config, app.hub());
    match stellwerk_engine::reconcile(client, &state).await {
        Ok(outcome) => {
            for failure in &outcome.failures {
                tracing::error!(call = ?failure.call, error = %failure.error, "hub refused a change");
            }
            tracing::debug!(
                revision = state.revision,
                calls = outcome.calls.len(),
                "hub reconciled"
            );
        }
        Err(err) => tracing::error!(%err, "could not reconcile the hub"),
    }
}

/// Follows the hub's Syncthing: its folder states and every device's completion go into the
/// live state, changes that arrive from devices into the activity feed.
pub async fn watch(app: App, client: Client, activity_window: Duration) {
    let (snapshots, mut latest) = watch::channel(Snapshot::default());
    let (changes, arrived) = mpsc::unbounded_channel();
    let monitor = tokio::spawn(monitor::run(
        client,
        monitor::Options { completion: true },
        snapshots,
        Some(changes),
    ));
    let activity = tokio::spawn(record_activity(app.clone(), arrived, activity_window));
    while latest.changed().await.is_ok() {
        let snapshot = latest.borrow_and_update().clone();
        app.set_hub_snapshot(snapshot);
    }
    monitor.abort();
    activity.abort();
}

async fn record_activity(
    app: App,
    mut arrived: mpsc::UnboundedReceiver<Vec<RemoteChange>>,
    window: Duration,
) {
    while let Some(first) = arrived.recv().await {
        let mut batch = first;
        let window = tokio::time::sleep(window);
        tokio::pin!(window);
        loop {
            tokio::select! {
                () = &mut window => break,
                more = arrived.recv() => match more {
                    Some(more) => batch.extend(more),
                    None => break,
                },
            }
        }
        let Ok(config) = app.store().config().await else {
            continue;
        };
        let at = store::now();
        for (device, folder, count) in summarize_changes(&config, &batch) {
            let _ = app
                .store()
                .add_event(
                    at,
                    Some(&device),
                    Some(&folder),
                    "changes",
                    &count.to_string(),
                )
                .await;
        }
        app.tick();
    }
}
