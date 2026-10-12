//! The hub Syncthing: started by the Zentrale and kept at its desired state with the same
//! reconciler the agents use.

use stellwerk_engine::Client;

use crate::app::App;
use crate::desired;

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
