//! Runs the reconciler's calls against a Syncthing: the side-effect half of reconciling.

use stellwerk_protocol::DesiredState;

use crate::client::Client;
use crate::error::Result;
use crate::reconcile::{ApiCall, plan};

/// Runs the calls in order and stops at the first that fails.
pub async fn apply(client: &Client, calls: &[ApiCall]) -> Result<()> {
    for call in calls {
        tracing::debug!(?call, "applying");
        match call {
            ApiCall::PutDevice(device) => client.put_device(device).await?,
            ApiCall::PutFolder(folder) => client.put_folder(folder).await?,
            ApiCall::DeleteFolder(id) => client.delete_folder(id).await?,
            ApiCall::DeleteDevice(id) => client.delete_device(id).await?,
        }
    }
    Ok(())
}

/// Reads the actual configuration, plans and applies; returns the calls it made.
pub async fn reconcile(client: &Client, desired: &DesiredState) -> Result<Vec<ApiCall>> {
    let calls = plan(&client.actual().await?, desired)?;
    apply(client, &calls).await?;
    Ok(calls)
}
