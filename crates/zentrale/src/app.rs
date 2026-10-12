//! Shared state of a running Zentrale: the store, the hub, the current revision and who is
//! connected right now.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::watch;

use crate::model::Hub;
use crate::store::{Result, Store};

#[derive(Debug, Clone)]
pub struct App {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    store: Store,
    hub: Hub,
    revision: watch::Sender<u64>,
    /// Connected agents: device ID → (connection, connected since).
    online: Mutex<BTreeMap<String, (u64, i64)>>,
    connections: AtomicU64,
}

impl App {
    pub async fn new(store: Store, hub: Hub) -> Result<Self> {
        let revision = store.revision().await?;
        Ok(Self {
            inner: Arc::new(Inner {
                store,
                hub,
                revision: watch::Sender::new(revision),
                online: Mutex::default(),
                connections: AtomicU64::new(0),
            }),
        })
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    pub fn hub(&self) -> &Hub {
        &self.inner.hub
    }

    /// Call after every change: agents and hub get the new desired state.
    pub async fn changed(&self) -> Result<()> {
        let revision = self.inner.store.revision().await?;
        self.inner.revision.send_if_modified(|current| {
            let modified = *current != revision;
            *current = revision;
            modified
        });
        Ok(())
    }

    /// Wakes up on every new revision.
    pub fn revisions(&self) -> watch::Receiver<u64> {
        self.inner.revision.subscribe()
    }

    /// Marks a device connected and returns the connection's number for [`Self::set_offline`].
    pub fn set_online(&self, device_id: &str, since: i64) -> u64 {
        let connection = self.inner.connections.fetch_add(1, Ordering::Relaxed);
        self.online()
            .insert(device_id.to_string(), (connection, since));
        connection
    }

    /// Marks the device offline unless a newer connection of it took over meanwhile.
    pub fn set_offline(&self, device_id: &str, connection: u64) {
        let mut online = self.online();
        if online.get(device_id).is_some_and(|(c, _)| *c == connection) {
            online.remove(device_id);
        }
    }

    /// Device ID → connected since, for the UI.
    pub fn online_devices(&self) -> BTreeMap<String, i64> {
        self.online()
            .iter()
            .map(|(id, (_, since))| (id.clone(), *since))
            .collect()
    }

    fn online(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, (u64, i64)>> {
        // A panic while holding the lock leaves a plain map behind; keep using it.
        self.inner
            .online
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
