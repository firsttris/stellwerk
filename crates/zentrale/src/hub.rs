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

/// The first retry after a failed hub reconcile; each further one waits twice as long.
const FIRST_RETRY: Duration = Duration::from_secs(1);
const MAX_RETRY: Duration = Duration::from_secs(60);

/// Reconciles the hub now and after every new revision, until the app goes away. A failed
/// reconcile is retried with a growing delay, so a device added while the hub was briefly
/// unreachable still becomes its peer.
pub async fn run(app: App, client: Client) {
    let mut revisions = app.revisions();
    let mut retry = None;
    loop {
        revisions.borrow_and_update();
        retry = next_retry(retry, reconcile(&app, &client).await);
        let changed = match retry {
            None => revisions.changed().await,
            Some(delay) => tokio::select! {
                changed = revisions.changed() => changed,
                () = tokio::time::sleep(delay) => Ok(()),
            },
        };
        if changed.is_err() {
            return;
        }
    }
}

/// How long to wait before trying again: nothing after a success, otherwise doubling from
/// [`FIRST_RETRY`] up to [`MAX_RETRY`].
fn next_retry(previous: Option<Duration>, succeeded: bool) -> Option<Duration> {
    if succeeded {
        return None;
    }
    Some(previous.map_or(FIRST_RETRY, |delay| (delay * 2).min(MAX_RETRY)))
}

/// Brings the hub to its desired state; `false` when anything failed.
async fn reconcile(app: &App, client: &Client) -> bool {
    let config = match app.store().config().await {
        Ok(config) => config,
        Err(err) => {
            tracing::error!(%err, "could not read the configuration");
            return false;
        }
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
            outcome.failures.is_empty()
        }
        Err(err) => {
            tracing::error!(%err, "could not reconcile the hub");
            false
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_are_retried_with_a_growing_delay_until_one_succeeds() {
        let first = next_retry(None, false);
        assert_eq!(first, Some(FIRST_RETRY));
        assert_eq!(next_retry(first, false), Some(FIRST_RETRY * 2));
        assert_eq!(next_retry(Some(MAX_RETRY), false), Some(MAX_RETRY));
        assert_eq!(next_retry(Some(MAX_RETRY), true), None);
        assert_eq!(next_retry(None, true), None);
    }
}
