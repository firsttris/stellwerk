//! A Zentrale with its hub and agents in one test process; every Syncthing on 127.0.0.1 with
//! random ports.

#![allow(dead_code)]

use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use stellwerk_agent::{Network, Options as AgentOptions};
use stellwerk_engine::download::ensure_syncthing;
use stellwerk_engine::process::free_port;
use stellwerk_zentrale::pairing::{CODE_LIFETIME_SECS, code_hash, new_code};
use stellwerk_zentrale::server::{Options, Running, start};
use stellwerk_zentrale::store::now;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

pub const TIMEOUT: Duration = Duration::from_secs(180);

pub async fn syncthing_binary() -> PathBuf {
    let cache = std::env::var_os("STELLWERK_SYNCTHING_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.syncthing-cache"));
    ensure_syncthing(&cache).await.unwrap()
}

pub async fn start_zentrale(dir: &Path) -> Running {
    let hub = format!("tcp://127.0.0.1:{}", free_port().unwrap());
    let options = Options {
        data_dir: dir.join("zentrale"),
        syncthing_binary: Some(syncthing_binary().await),
        hub_listen: hub.clone(),
        hub_address: hub,
        activity_window: Duration::from_secs(1),
        lan_pairing: true,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    start(&options, listener).await.unwrap()
}

/// An agent running in this process, stopped with [`Agent::stop`].
pub struct Agent {
    pub data_dir: PathBuf,
    stop: oneshot::Sender<()>,
    task: JoinHandle<anyhow::Result<()>>,
}

impl Agent {
    pub async fn stop(self) {
        let _ = self.stop.send(());
        self.task.await.unwrap().unwrap();
    }
}

/// A new pairing code at the Zentrale, like "Pair device" in the UI.
pub async fn new_pairing_code(zentrale: &Running) -> String {
    let code = new_code();
    zentrale
        .app
        .store()
        .create_enrollment(&code_hash(&code), now() + CODE_LIFETIME_SECS)
        .await
        .unwrap();
    code
}

/// Creates a pairing code at the Zentrale and joins with it.
pub async fn pair_agent(zentrale: &Running, data_dir: PathBuf) -> Agent {
    let code = new_pairing_code(zentrale).await;
    stellwerk_agent::join(&data_dir, &format!("http://{}", zentrale.addr), Some(&code)).unwrap();
    start_agent(data_dir).await
}

/// Starts an agent that was paired before.
pub async fn start_agent(data_dir: PathBuf) -> Agent {
    let name = data_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    let options = AgentOptions {
        data_dir: data_dir.clone(),
        syncthing_binary: Some(syncthing_binary().await),
        network: Network::Isolated,
        name,
    };
    let (stop, stopped) = oneshot::channel::<()>();
    let task = tokio::spawn(stellwerk_agent::run(options, async {
        let _ = stopped.await;
    }));
    Agent {
        data_dir,
        stop,
        task,
    }
}

/// Polls `check` until it returns `Some`, failing the test after [`TIMEOUT`].
pub async fn wait_for<T, F, Fut>(what: &str, mut check: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        if let Some(value) = check().await {
            return value;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

pub async fn wait_for_file(path: &Path, contents: &str) {
    wait_for(&format!("{} with {contents:?}", path.display()), || async {
        std::fs::read_to_string(path)
            .ok()
            .filter(|found| found == contents)
    })
    .await;
}
