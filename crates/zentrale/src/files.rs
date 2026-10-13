//! Conflicts and versions, both on the hub: it holds every folder in full, so they are found
//! and resolved there, and Syncthing carries the result to every device.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use stellwerk_engine::client::FileVersion;
use stellwerk_engine::files::{self, Conflict, Keep, safe_relative};

use crate::app::App;
use crate::desired::hub_path;
use crate::model::{Config, Folder};
use crate::store::{self, now};

/// How often the hub's folders are searched for conflict copies.
pub const SCAN_INTERVAL: Duration = Duration::from_secs(30);
/// Larger files are not shown as text.
const TEXT_LIMIT: u64 = 256 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FilesError {
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Failed(String),
}

impl From<store::StoreError> for FilesError {
    fn from(err: store::StoreError) -> Self {
        match err {
            store::StoreError::NotFound(what) => Self::NotFound(what),
            other => Self::Failed(other.to_string()),
        }
    }
}

impl From<stellwerk_engine::Error> for FilesError {
    fn from(err: stellwerk_engine::Error) -> Self {
        match err {
            stellwerk_engine::Error::Archive(message) => Self::Invalid(message),
            other => Self::Failed(other.to_string()),
        }
    }
}

pub type Result<T, E = FilesError> = std::result::Result<T, E>;

/// A conflict as the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictView {
    pub path: String,
    pub original: String,
    pub at: i64,
    /// The device whose version the copy holds, if known.
    pub device_id: Option<String>,
    pub size: u64,
    pub modified: i64,
    pub original_size: Option<u64>,
    pub original_modified: Option<i64>,
}

/// The device a short ID (the start of a Syncthing device ID) belongs to.
pub fn device_by_short_id<'a>(config: &'a Config, short: &str) -> Option<&'a crate::model::Device> {
    config
        .devices
        .iter()
        .find(|d| !short.is_empty() && d.syncthing_id.starts_with(short))
}

pub fn conflict_view(config: &Config, conflict: &Conflict) -> ConflictView {
    ConflictView {
        path: conflict.path.clone(),
        original: conflict.original.clone(),
        at: conflict.at,
        device_id: device_by_short_id(config, &conflict.modified_by).map(|d| d.id.clone()),
        size: conflict.size,
        modified: conflict.modified,
        original_size: conflict.original_size,
        original_modified: conflict.original_modified,
    }
}

fn folder<'a>(config: &'a Config, id: &str) -> Result<&'a Folder> {
    config
        .folders
        .iter()
        .find(|f| f.id == id)
        .ok_or(FilesError::NotFound("folder"))
}

fn root(app: &App, folder: &Folder) -> PathBuf {
    PathBuf::from(hub_path(app.hub(), &folder.syncthing_folder_id))
}

/// Searches every folder on the hub now; new conflicts go into the activity feed.
pub async fn scan(app: &App) -> Result<()> {
    let config = app.store().config().await?;
    let roots: Vec<(String, PathBuf)> = config
        .folders
        .iter()
        .map(|f| (f.id.clone(), root(app, f)))
        .collect();
    let found = tokio::task::spawn_blocking(move || {
        roots
            .into_iter()
            .map(|(id, root)| (id, files::scan_conflicts(&root)))
            .filter(|(_, conflicts)| !conflicts.is_empty())
            .collect::<BTreeMap<_, _>>()
    })
    .await
    .map_err(|err| FilesError::Failed(err.to_string()))?;

    let known: BTreeSet<(String, String)> = app
        .conflicts()
        .iter()
        .flat_map(|(folder, list)| list.iter().map(move |c| (folder.clone(), c.path.clone())))
        .collect();
    for (folder, list) in &found {
        for conflict in list
            .iter()
            .filter(|c| !known.contains(&(folder.clone(), c.path.clone())))
        {
            let device = device_by_short_id(&config, &conflict.modified_by).map(|d| d.id.as_str());
            let _ = app
                .store()
                .add_event(now(), device, Some(folder), "conflict", &conflict.original)
                .await;
        }
    }
    app.set_conflicts(found);
    Ok(())
}

/// Scans now and then every [`SCAN_INTERVAL`].
pub async fn watch(app: App) {
    loop {
        if let Err(err) = scan(&app).await {
            tracing::warn!(%err, "could not search for conflicts");
        }
        tokio::time::sleep(SCAN_INTERVAL).await;
    }
}

/// A file as the conflict view compares it: text when it is small UTF-8.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    pub size: u64,
    pub modified: i64,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictContent {
    pub original: Option<FileContent>,
    pub copy: FileContent,
}

fn read(root: &Path, path: &str) -> Result<Option<FileContent>> {
    let full = root.join(safe_relative(path)?);
    // A symbolic link is not followed: a device could make one point at any file here.
    let Some(meta) = files::regular_file(&full) else {
        return Ok(None);
    };
    let text = (meta.len() <= TEXT_LIMIT)
        .then(|| std::fs::read(&full).ok())
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok());
    Ok(Some(FileContent {
        size: meta.len(),
        modified: files::modified_secs(&meta),
        text,
    }))
}

/// Both sides of a conflict, for the side-by-side view.
pub async fn content(app: &App, folder_id: &str, copy: &str) -> Result<ConflictContent> {
    let config = app.store().config().await?;
    let folder = folder(&config, folder_id)?;
    let (original, _, _) = files::conflict_original(copy)
        .ok_or_else(|| FilesError::Invalid(format!("not a conflict copy: {copy}")))?;
    let root = root(app, folder);
    Ok(ConflictContent {
        original: read(&root, &original)?,
        copy: read(&root, copy)?.ok_or(FilesError::NotFound("conflict copy"))?,
    })
}

/// Resolves on the hub, rescans the folder there so Syncthing sends the result out at once.
pub async fn resolve(app: &App, folder_id: &str, copy: &str, keep: Keep) -> Result<()> {
    let config = app.store().config().await?;
    let folder = folder(&config, folder_id)?;
    let label = files::conflict_original(copy)
        .and_then(|(_, _, by)| device_by_short_id(&config, &by).map(|d| d.name.clone()))
        .unwrap_or_else(|| "conflict".into());
    let root = root(app, folder);
    let copy_owned = copy.to_string();
    tokio::task::spawn_blocking(move || files::resolve(&root, &copy_owned, keep, &label, now()))
        .await
        .map_err(|err| FilesError::Failed(err.to_string()))??;
    app.hub_client()
        .scan(&folder.syncthing_folder_id)
        .await
        .map_err(FilesError::from)?;
    let text = match keep {
        Keep::Original => "original",
        Keep::Copy => "copy",
        Keep::Both => "both",
    };
    let _ = app
        .store()
        .add_event(now(), None, Some(folder_id), "resolved", text)
        .await;
    scan(app).await
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileVersions {
    pub path: String,
    /// Newest first.
    pub versions: Vec<FileVersion>,
}

/// Every archived version on the hub, by file, newest first.
pub async fn versions(app: &App, folder_id: &str) -> Result<Vec<FileVersions>> {
    let config = app.store().config().await?;
    let folder = folder(&config, folder_id)?;
    let listing = app
        .hub_client()
        .versions(&folder.syncthing_folder_id)
        .await?;
    Ok(listing
        .into_iter()
        .map(|(path, mut versions)| {
            versions.sort_by(|a, b| b.version_time.cmp(&a.version_time));
            FileVersions { path, versions }
        })
        .collect())
}

/// Puts a version back on the hub; Syncthing archives the current file first and then sends
/// the restored one to every device.
pub async fn restore(app: &App, folder_id: &str, path: &str, version_time: &str) -> Result<()> {
    safe_relative(path)?;
    let config = app.store().config().await?;
    let folder = folder(&config, folder_id)?;
    let errors = app
        .hub_client()
        .restore_version(&folder.syncthing_folder_id, path, version_time)
        .await?;
    if let Some(error) = errors.values().next() {
        return Err(FilesError::Invalid(error.clone()));
    }
    let _ = app
        .store()
        .add_event(now(), None, Some(folder_id), "restored", path)
        .await;
    Ok(())
}

// The only test needs symbolic links, which Windows creates only with extra rights.
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn reading_does_not_follow_symbolic_links() {
        let dir = tempfile::tempdir().unwrap();
        let secret = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(secret.path(), "secret").unwrap();
        std::os::unix::fs::symlink(secret.path(), dir.path().join("link.txt")).unwrap();
        std::fs::write(dir.path().join("plain.txt"), "plain").unwrap();

        assert_eq!(read(dir.path(), "link.txt").unwrap(), None);
        let plain = read(dir.path(), "plain.txt").unwrap().unwrap();
        assert_eq!(plain.text.as_deref(), Some("plain"));
    }
}
