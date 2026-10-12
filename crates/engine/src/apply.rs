//! Runs the reconciler's calls against a Syncthing: the side-effect half of reconciling.

use stellwerk_protocol::DesiredState;

use crate::client::Client;
use crate::error::{Error, Result};
use crate::reconcile::{ApiCall, plan};

/// A call Syncthing refused, and why.
#[derive(Debug)]
pub struct Failure {
    pub call: ApiCall,
    pub error: Error,
}

/// What one reconcile did.
#[derive(Debug, Default)]
pub struct Outcome {
    pub calls: Vec<ApiCall>,
    pub failures: Vec<Failure>,
}

/// Runs every call in order; one that fails does not stop the others (a folder with a bad
/// path must not keep the other folders from being set up).
pub async fn apply(client: &Client, calls: &[ApiCall]) -> Vec<Failure> {
    let mut failures = Vec::new();
    for call in calls {
        tracing::debug!(?call, "applying");
        let result = match call {
            ApiCall::PutDevice(device) => client.put_device(device).await,
            ApiCall::PutFolder(folder) => client.put_folder(folder).await,
            ApiCall::DeleteFolder(id) => client.delete_folder(id).await,
            ApiCall::DeleteDevice(id) => client.delete_device(id).await,
            ApiCall::SetIgnores { folder, lines } => client.set_ignores(folder, lines).await,
        };
        if let Err(error) = result {
            tracing::warn!(?call, %error, "Syncthing refused a change");
            failures.push(Failure {
                call: call.clone(),
                error,
            });
        }
    }
    failures
}

/// Reads the actual configuration, plans and applies. Errors are only reading and planning;
/// refused calls are in [`Outcome::failures`].
pub async fn reconcile(client: &Client, desired: &DesiredState) -> Result<Outcome> {
    let calls = plan(&client.actual().await?, desired)?;
    let failures = apply(client, &calls).await;
    Ok(Outcome { calls, failures })
}
