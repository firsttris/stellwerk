//! The Zentrale on the server. Phase 1: axum server with SQLite, agent hub and the web UI;
//! steers its own hub Syncthing with the same `stellwerk_engine` as the agents.

fn main() {
    println!(
        "stellwerk {} (Syncthing {}): not usable yet, see the roadmap",
        env!("CARGO_PKG_VERSION"),
        stellwerk_engine::pin::SYNCTHING_VERSION
    );
}
