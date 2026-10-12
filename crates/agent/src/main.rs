//! The agent on each device. Phase 1: connects to the Zentrale over WebSocket, receives its
//! `DesiredState` and applies it with `stellwerk_engine::reconcile`.

fn main() {
    println!(
        "stellwerk-agent {} (Syncthing {}): not usable yet, see the roadmap",
        env!("CARGO_PKG_VERSION"),
        stellwerk_engine::pin::SYNCTHING_VERSION
    );
}
