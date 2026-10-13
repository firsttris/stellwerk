//! Shared state of a running Zentrale: the store, the hub, the current revision and who is
//! connected right now.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::watch;

use stellwerk_engine::Client;
use stellwerk_engine::files::Conflict;
use stellwerk_engine::monitor::Snapshot;
use stellwerk_protocol::status::FolderStatus;

use crate::ca::Ca;
use crate::live::{DeviceLive, Live};
use crate::model::Hub;
use crate::store::{NewDevice, Result, Store};
use crate::waiting::{Decision, Place, Refused, Waiting, WaitingView};

#[derive(Debug, Clone)]
pub struct App {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    store: Store,
    hub: Hub,
    hub_client: Client,
    /// Conflict copies on the hub by folder ID, from the last scan.
    conflicts: Mutex<BTreeMap<String, Vec<Conflict>>>,
    revision: watch::Sender<u64>,
    /// Connected agents: device ID → (connection, connected since).
    online: Mutex<BTreeMap<String, (u64, i64)>>,
    connections: AtomicU64,
    live: Mutex<Live>,
    /// Raised on anything the UI shows live: revision, online states, folder states.
    ticks: watch::Sender<u64>,
    /// Whether agents without a code may wait for approval in the UI.
    lan_pairing: AtomicBool,
    waiting: Mutex<Waiting>,
    /// Failed logins in a row; held while a password is checked, so checks run one at a time.
    login_failures: tokio::sync::Mutex<u32>,
    /// The private CA devices are told to trust, set once at start.
    ca: OnceLock<Ca>,
}

impl App {
    pub async fn new(store: Store, hub: Hub, hub_client: Client) -> Result<Self> {
        let revision = store.revision().await?;
        Ok(Self {
            inner: Arc::new(Inner {
                store,
                hub,
                hub_client,
                conflicts: Mutex::default(),
                revision: watch::Sender::new(revision),
                online: Mutex::default(),
                connections: AtomicU64::new(0),
                live: Mutex::default(),
                ticks: watch::Sender::new(0),
                lan_pairing: AtomicBool::new(false),
                waiting: Mutex::default(),
                login_failures: tokio::sync::Mutex::default(),
                ca: OnceLock::new(),
            }),
        })
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    pub fn hub(&self) -> &Hub {
        &self.inner.hub
    }

    /// The hub's Syncthing API.
    pub fn hub_client(&self) -> &Client {
        &self.inner.hub_client
    }

    pub fn conflicts(&self) -> BTreeMap<String, Vec<Conflict>> {
        lock(&self.inner.conflicts).clone()
    }

    pub fn set_conflicts(&self, conflicts: BTreeMap<String, Vec<Conflict>>) {
        let changed = {
            let mut current = lock(&self.inner.conflicts);
            let changed = *current != conflicts;
            *current = conflicts;
            changed
        };
        if changed {
            self.tick();
        }
    }

    /// Call after every change: agents and hub get the new desired state.
    pub async fn changed(&self) -> Result<()> {
        let revision = self.inner.store.revision().await?;
        self.inner.revision.send_if_modified(|current| {
            let modified = *current != revision;
            *current = revision;
            modified
        });
        self.tick();
        Ok(())
    }

    /// Failed logins in a row. Lock it around checking a password.
    pub fn login_failures(&self) -> &tokio::sync::Mutex<u32> {
        &self.inner.login_failures
    }

    /// Something the UI shows changed.
    pub fn tick(&self) {
        self.inner
            .ticks
            .send_modify(|tick| *tick = tick.wrapping_add(1));
    }

    /// Wakes up whenever the UI should refresh.
    pub fn ticks(&self) -> watch::Receiver<u64> {
        self.inner.ticks.subscribe()
    }

    pub fn set_device_status(&self, device_id: &str, folders: Vec<FolderStatus>, at: i64) {
        lock(&self.inner.live)
            .devices
            .insert(device_id.to_string(), DeviceLive { at, folders });
        self.tick();
    }

    pub fn set_hub_snapshot(&self, snapshot: Snapshot) {
        lock(&self.inner.live).hub = snapshot;
        self.tick();
    }

    pub fn live(&self) -> Live {
        lock(&self.inner.live).clone()
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
        self.tick();
        connection
    }

    /// Marks the device offline unless a newer connection of it took over meanwhile.
    pub fn set_offline(&self, device_id: &str, connection: u64) {
        let mut online = self.online();
        if online.get(device_id).is_some_and(|(c, _)| *c == connection) {
            online.remove(device_id);
            drop(online);
            self.tick();
        }
    }

    /// Device ID → connected since, for the UI.
    pub fn online_devices(&self) -> BTreeMap<String, i64> {
        self.online()
            .iter()
            .map(|(id, (_, since))| (id.clone(), *since))
            .collect()
    }

    pub fn ca(&self) -> Option<&Ca> {
        self.inner.ca.get()
    }

    /// Only the first call counts; the CA is set once at start.
    pub fn set_ca(&self, ca: Ca) {
        let _ = self.inner.ca.set(ca);
    }

    pub fn lan_pairing(&self) -> bool {
        self.inner.lan_pairing.load(Ordering::Relaxed)
    }

    pub fn set_lan_pairing(&self, on: bool) {
        self.inner.lan_pairing.store(on, Ordering::Relaxed);
    }

    /// Puts an agent without a code on the list for approval in the UI.
    pub fn wait_for_approval(&self, device: NewDevice, since: i64) -> Result<Place, Refused> {
        let place = lock(&self.inner.waiting).join(device, since)?;
        self.tick();
        Ok(place)
    }

    /// The agent disconnected while waiting.
    pub fn stop_waiting(&self, fingerprint: &str, ticket: u64) {
        if lock(&self.inner.waiting).leave(fingerprint, ticket) {
            self.tick();
        }
    }

    /// Takes a waiting agent off the list; see [`Waiting::decide`].
    pub fn decide_waiting(
        &self,
        fingerprint: &str,
        decision: Decision,
    ) -> Option<(NewDevice, tokio::sync::oneshot::Sender<Decision>)> {
        let taken = lock(&self.inner.waiting).decide(fingerprint, decision);
        if taken.is_some() {
            self.tick();
        }
        taken
    }

    pub fn waiting(&self) -> Vec<WaitingView> {
        lock(&self.inner.waiting).views()
    }

    fn online(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, (u64, i64)>> {
        lock(&self.inner.online)
    }
}

/// A panic while holding a lock leaves plain data behind; keep using it.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
