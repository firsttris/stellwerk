//! Folders and shares: the rows and columns of the matrix.

use std::collections::HashSet;

use rand::Rng;
use serde::Deserialize;
use sqlx::{FromRow, Sqlite, Transaction};
use stellwerk_protocol::{FolderMode, Versioning};

use super::{Result, Store, StoreError, bump, new_id};
use crate::model::{DEFAULT_HUB_VERSIONING, Folder, Share, mode_from_str, mode_to_str};

/// One cell of the matrix as the UI sends it: `mode: None` is "off". `path` is only needed
/// to change it; a new share without one gets [`default_path`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareChange {
    pub folder_id: String,
    pub device_id: String,
    pub mode: Option<FolderMode>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(FromRow)]
struct FolderRow {
    id: String,
    label: String,
    syncthing_folder_id: String,
    versioning: String,
    ignore_patterns: String,
    created_at: i64,
}

#[derive(FromRow)]
struct ShareRow {
    folder_id: String,
    device_id: String,
    mode: String,
    path: String,
}

impl From<FolderRow> for Folder {
    fn from(row: FolderRow) -> Self {
        Self {
            id: row.id,
            label: row.label,
            syncthing_folder_id: row.syncthing_folder_id,
            versioning: serde_json::from_str(&row.versioning).unwrap_or(DEFAULT_HUB_VERSIONING),
            ignore_patterns: row.ignore_patterns.lines().map(str::to_string).collect(),
            created_at: row.created_at,
        }
    }
}

pub(super) async fn all(tx: &mut Transaction<'_, Sqlite>) -> Result<(Vec<Folder>, Vec<Share>)> {
    let folders: Vec<FolderRow> = sqlx::query_as(
        "SELECT id, label, syncthing_folder_id, versioning, ignore_patterns, created_at \
         FROM folders ORDER BY label COLLATE NOCASE, id",
    )
    .fetch_all(&mut **tx)
    .await?;
    let shares: Vec<ShareRow> = sqlx::query_as(
        "SELECT folder_id, device_id, mode, path FROM shares ORDER BY folder_id, device_id",
    )
    .fetch_all(&mut **tx)
    .await?;
    let shares = shares
        .into_iter()
        .filter_map(|row| {
            Some(Share {
                mode: mode_from_str(&row.mode)?,
                folder_id: row.folder_id,
                device_id: row.device_id,
                path: row.path,
            })
        })
        .collect();
    Ok((folders.into_iter().map(Folder::from).collect(), shares))
}

/// Where a folder lands on a device unless the admin says otherwise; Syncthing expands `~`.
pub fn default_path(label: &str) -> String {
    let name: String = label
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':') {
                '-'
            } else {
                c
            }
        })
        .collect();
    format!("~/Stellwerk/{}", name.trim())
}

/// A Syncthing-style folder ID such as `k3j9x-2mwqa`, unique enough per installation.
fn syncthing_folder_id() -> String {
    const CHARS: &[u8] = b"abcdefghijkmnopqrstuvwxyz23456789";
    let mut rng = rand::rng();
    let mut part = || -> String {
        (0..5)
            .map(|_| char::from(CHARS[rng.random_range(0..CHARS.len())]))
            .collect()
    };
    format!("{}-{}", part(), part())
}

fn versioning_json(versioning: Versioning) -> String {
    serde_json::to_string(&versioning).unwrap_or_default()
}

impl Store {
    pub async fn create_folder(&self, label: &str, now: i64) -> Result<Folder> {
        let label = label.trim();
        if label.is_empty() {
            return Err(StoreError::Invalid("a folder needs a name".into()));
        }
        let folder = Folder {
            id: new_id(),
            label: label.to_string(),
            syncthing_folder_id: syncthing_folder_id(),
            versioning: DEFAULT_HUB_VERSIONING,
            ignore_patterns: Vec::new(),
            created_at: now,
        };
        let mut tx = self.write().await?;
        sqlx::query(
            "INSERT INTO folders (id, label, syncthing_folder_id, versioning, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&folder.id)
        .bind(&folder.label)
        .bind(&folder.syncthing_folder_id)
        .bind(versioning_json(folder.versioning))
        .bind(folder.created_at)
        .execute(&mut *tx)
        .await?;
        bump(&mut tx).await?;
        tx.commit().await?;
        Ok(folder)
    }

    /// Renames and/or sets the ignore patterns; `None` leaves a field as it is.
    pub async fn update_folder(
        &self,
        id: &str,
        label: Option<&str>,
        ignore_patterns: Option<&[String]>,
    ) -> Result<()> {
        let label = label.map(str::trim);
        if label.is_some_and(str::is_empty) {
            return Err(StoreError::Invalid("a folder needs a name".into()));
        }
        let patterns = ignore_patterns.map(|lines| {
            lines
                .iter()
                .map(|line| line.trim_end())
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        });
        let mut tx = self.write().await?;
        let found = sqlx::query(
            "UPDATE folders SET label = COALESCE(?, label), \
             ignore_patterns = COALESCE(?, ignore_patterns) WHERE id = ?",
        )
        .bind(label)
        .bind(patterns)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if found.rows_affected() == 0 {
            return Err(StoreError::NotFound("folder"));
        }
        bump(&mut tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Stops the folder everywhere; the files on the devices and the hub stay.
    pub async fn delete_folder(&self, id: &str) -> Result<()> {
        let mut tx = self.write().await?;
        let deleted = sqlx::query("DELETE FROM folders WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() == 0 {
            return Err(StoreError::NotFound("folder"));
        }
        bump(&mut tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Applies a draft from the matrix as one new revision; nothing changes when the draft
    /// changes nothing. Returns the revision after it.
    pub async fn apply_shares(&self, changes: &[ShareChange]) -> Result<u64> {
        let mut tx = self.write().await?;
        let mut seen = HashSet::new();
        let mut changed = false;
        for change in changes {
            if !seen.insert((&change.folder_id, &change.device_id)) {
                return Err(StoreError::Invalid(format!(
                    "folder {} and device {} appear twice",
                    change.folder_id, change.device_id
                )));
            }
            changed |= apply_share(&mut tx, change).await?;
        }
        let revision = if changed {
            bump(&mut tx).await?
        } else {
            super::revision(&mut tx).await?
        };
        tx.commit().await?;
        Ok(revision)
    }
}

/// Returns whether the share actually changed.
async fn apply_share(tx: &mut Transaction<'_, Sqlite>, change: &ShareChange) -> Result<bool> {
    let current: Option<ShareRow> = sqlx::query_as(
        "SELECT folder_id, device_id, mode, path FROM shares WHERE folder_id = ? AND device_id = ?",
    )
    .bind(&change.folder_id)
    .bind(&change.device_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(mode) = change.mode else {
        sqlx::query("DELETE FROM shares WHERE folder_id = ? AND device_id = ?")
            .bind(&change.folder_id)
            .bind(&change.device_id)
            .execute(&mut **tx)
            .await?;
        return Ok(current.is_some());
    };

    let path = match (&change.path, &current) {
        (Some(path), _) if !path.trim().is_empty() => path.trim().to_string(),
        (_, Some(current)) => current.path.clone(),
        (_, None) => {
            let (label,): (String,) = sqlx::query_as("SELECT label FROM folders WHERE id = ?")
                .bind(&change.folder_id)
                .fetch_optional(&mut **tx)
                .await?
                .ok_or(StoreError::NotFound("folder"))?;
            default_path(&label)
        }
    };
    let mode = mode_to_str(mode);
    if current
        .as_ref()
        .is_some_and(|c| c.mode == mode && c.path == path)
    {
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO shares (folder_id, device_id, mode, path) VALUES (?, ?, ?, ?) \
         ON CONFLICT (folder_id, device_id) DO UPDATE SET mode = excluded.mode, path = excluded.path",
    )
    .bind(&change.folder_id)
    .bind(&change.device_id)
    .bind(mode)
    .bind(&path)
    .execute(&mut **tx)
    .await
    .map_err(|err| match err {
        sqlx::Error::Database(db) if db.is_foreign_key_violation() => {
            StoreError::NotFound("folder or device")
        }
        other => StoreError::Db(other),
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{test_device, test_store};

    fn change(folder: &str, device: &str, mode: Option<FolderMode>) -> ShareChange {
        ShareChange {
            folder_id: folder.into(),
            device_id: device.into(),
            mode,
            path: None,
        }
    }

    #[test]
    fn default_paths_keep_the_name_but_no_separators() {
        assert_eq!(default_path(" Fotos "), "~/Stellwerk/Fotos");
        assert_eq!(default_path("a/b:c"), "~/Stellwerk/a-b-c");
    }

    #[test]
    fn folder_ids_look_like_syncthings() {
        let id = syncthing_folder_id();
        assert_eq!(id.len(), 11);
        assert_eq!(&id[5..6], "-");
        assert_ne!(id, syncthing_folder_id());
    }

    #[tokio::test]
    async fn a_draft_is_one_revision_and_an_empty_one_none() {
        let (store, _dir) = test_store().await;
        let device = test_device(&store, "KEY").await;
        let folder = store.create_folder("Fotos", 1).await.unwrap();
        let start = store.revision().await.unwrap();

        let both = change(&folder.id, &device.id, Some(FolderMode::SendReceive));
        assert_eq!(
            store
                .apply_shares(std::slice::from_ref(&both))
                .await
                .unwrap(),
            start + 1
        );
        assert_eq!(store.apply_shares(&[both]).await.unwrap(), start + 1);

        let config = store.config().await.unwrap();
        assert_eq!(config.shares.len(), 1);
        assert_eq!(config.shares[0].path, "~/Stellwerk/Fotos");

        let moved = ShareChange {
            mode: Some(FolderMode::ReceiveOnly),
            path: Some("/data/fotos".into()),
            ..change(&folder.id, &device.id, None)
        };
        assert_eq!(store.apply_shares(&[moved]).await.unwrap(), start + 2);
        let off = change(&folder.id, &device.id, None);
        assert_eq!(
            store
                .apply_shares(std::slice::from_ref(&off))
                .await
                .unwrap(),
            start + 3
        );
        assert_eq!(store.apply_shares(&[off]).await.unwrap(), start + 3);
        assert!(store.config().await.unwrap().shares.is_empty());
    }

    #[tokio::test]
    async fn unknown_folders_and_duplicates_are_refused_without_a_revision() {
        let (store, _dir) = test_store().await;
        let device = test_device(&store, "KEY").await;
        let folder = store.create_folder("Docs", 1).await.unwrap();
        let start = store.revision().await.unwrap();

        let unknown = change("nope", &device.id, Some(FolderMode::SendOnly));
        assert!(matches!(
            store.apply_shares(&[unknown]).await,
            Err(StoreError::NotFound(_))
        ));
        let twice = change(&folder.id, &device.id, Some(FolderMode::SendOnly));
        assert!(matches!(
            store.apply_shares(&[twice.clone(), twice]).await,
            Err(StoreError::Invalid(_))
        ));
        assert_eq!(store.revision().await.unwrap(), start);
    }

    #[tokio::test]
    async fn folders_keep_their_ignore_patterns() {
        let (store, _dir) = test_store().await;
        let folder = store.create_folder("Code", 1).await.unwrap();
        let lines = vec!["target/".to_string(), "".to_string(), "*.log ".to_string()];
        store
            .update_folder(&folder.id, None, Some(&lines))
            .await
            .unwrap();
        store
            .update_folder(&folder.id, Some("Quellcode"), None)
            .await
            .unwrap();
        let config = store.config().await.unwrap();
        assert_eq!(config.folders[0].label, "Quellcode");
        assert_eq!(config.folders[0].ignore_patterns, ["target/", "*.log"]);
        assert!(matches!(
            store.update_folder(&folder.id, Some(" "), None).await,
            Err(StoreError::Invalid(_))
        ));
    }

    #[tokio::test]
    async fn deleting_a_folder_removes_its_shares() {
        let (store, _dir) = test_store().await;
        let device = test_device(&store, "KEY").await;
        let folder = store.create_folder("Docs", 1).await.unwrap();
        store
            .apply_shares(&[change(
                &folder.id,
                &device.id,
                Some(FolderMode::SendReceive),
            )])
            .await
            .unwrap();

        store.delete_folder(&folder.id).await.unwrap();

        let config = store.config().await.unwrap();
        assert!(config.folders.is_empty() && config.shares.is_empty());
        assert!(matches!(
            store.delete_folder(&folder.id).await,
            Err(StoreError::NotFound(_))
        ));
    }
}
