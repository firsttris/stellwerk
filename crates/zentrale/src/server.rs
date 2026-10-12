//! Starting and stopping a whole Zentrale: database, hub Syncthing, HTTP server.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use axum::Router;
use axum::routing::get;
use stellwerk_engine::download::ensure_syncthing;
use stellwerk_engine::process::{Instance, SpawnOptions, start as start_syncthing};
use stellwerk_protocol::messages::AGENT_WS_PATH;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::app::App;
use crate::model::Hub;
use crate::store::Store;
use crate::{agents, hub};

#[derive(Debug, Clone)]
pub struct Options {
    /// Database, hub Syncthing home and the hub's folders.
    pub data_dir: PathBuf,
    /// A Syncthing binary to use instead of downloading the pinned one.
    pub syncthing_binary: Option<PathBuf>,
    /// Where the hub accepts sync connections, e.g. `tcp://0.0.0.0:22001`.
    pub hub_listen: String,
    /// Where devices reach the hub, e.g. `tcp://sync.example.org:22001`.
    pub hub_address: String,
}

/// A running Zentrale.
pub struct Running {
    pub app: App,
    pub addr: SocketAddr,
    hub: Instance,
    hub_task: JoinHandle<()>,
    server: JoinHandle<std::io::Result<()>>,
    shutdown: oneshot::Sender<()>,
}

pub fn router(app: App) -> Router {
    Router::new()
        .route(AGENT_WS_PATH, get(agents::ws))
        .with_state(app)
}

/// Opens the database, starts the hub Syncthing and serves HTTP on `listener`.
pub async fn start(options: &Options, listener: TcpListener) -> anyhow::Result<Running> {
    let data_dir = &options.data_dir;
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("creating {}", data_dir.display()))?;
    let store = Store::open(&data_dir.join("stellwerk.db")).await?;

    let binary = match &options.syncthing_binary {
        Some(binary) => binary.clone(),
        None => ensure_syncthing(&data_dir.join("syncthing-bin")).await?,
    };
    let hub = start_syncthing(&SpawnOptions {
        binary,
        home: data_dir.join("hub"),
        listen_address: options.hub_listen.clone(),
        discovery: false,
    })
    .await?;
    let hub_info = Hub {
        syncthing_id: hub.client().my_id().await?,
        address: options.hub_address.clone(),
        folders_dir: data_dir.join("folders").to_string_lossy().into_owned(),
    };
    tracing::info!(id = %hub_info.syncthing_id, address = %hub_info.address, "hub Syncthing running");

    let app = App::new(store, hub_info).await?;
    let hub_task = tokio::spawn(hub::run(app.clone(), hub.client().clone()));

    let addr = listener.local_addr()?;
    let (shutdown, stopped) = oneshot::channel();
    let server = tokio::spawn(
        axum::serve(listener, router(app.clone()))
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .into_future(),
    );
    tracing::info!(%addr, "Zentrale listening");
    Ok(Running {
        app,
        addr,
        hub,
        hub_task,
        server,
        shutdown,
    })
}

impl Running {
    /// Stops serving, then the hub Syncthing.
    pub async fn stop(self) -> anyhow::Result<()> {
        let _ = self.shutdown.send(());
        self.server.abort();
        self.hub_task.abort();
        self.hub.stop().await?;
        Ok(())
    }
}
