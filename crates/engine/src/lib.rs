//! The only crate that knows Syncthing.
//!
//! - [`download`] fetches the pinned Syncthing release and checks its SHA-256.
//! - [`process`] starts Syncthing with its own home, on 127.0.0.1 with random ports.
//! - [`client`] talks to its REST API (<https://docs.syncthing.net/dev/rest.html>).
//! - [`reconcile`] is the pure part: actual config + desired state → list of API calls.
//! - [`apply`] runs those calls; the only place where reconciling has side effects.
//! - [`monitor`] follows the event stream: folder states, completion, remote changes.
//! - [`files`] finds and resolves conflict copies and archives versions in a folder on disk.

pub mod apply;
pub mod client;
pub mod download;
pub mod error;
pub mod files;
pub mod monitor;
pub mod pin;
pub mod process;
pub mod reconcile;
pub mod syncthing_config;

pub use apply::{apply, reconcile};
pub use client::Client;
pub use error::{Error, Result};
pub use reconcile::{ApiCall, PlanError, plan};
