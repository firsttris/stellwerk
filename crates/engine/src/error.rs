use std::path::PathBuf;

use crate::reconcile::PlanError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Syncthing answered {method} {path} with {status}: {body}")]
    Api {
        method: reqwest::Method,
        path: String,
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("no pinned Syncthing build for {os}/{arch}")]
    UnsupportedPlatform {
        os: &'static str,
        arch: &'static str,
    },

    #[error("checksum mismatch for {asset}: expected {expected}, got {actual}")]
    Checksum {
        asset: String,
        expected: String,
        actual: String,
    },

    #[error("archive {asset} has no entry {entry}")]
    MissingEntry { asset: String, entry: String },

    #[error("archive error: {0}")]
    Archive(String),

    #[error("Syncthing did not start: {0}")]
    Startup(String),

    #[error("invalid desired state: {0}")]
    Plan(#[from] PlanError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn io_at(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.into();
    move |source| Error::Io { path, source }
}
