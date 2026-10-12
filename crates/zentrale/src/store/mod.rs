//! SQLite storage. Every change that alters a desired state raises the revision in the same
//! transaction, so agents and hub can tell whether they are up to date.

mod auth;
mod devices;
mod events;
mod folders;

use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Sqlite, SqlitePool, Transaction};

pub use auth::Session;
pub use devices::{DeviceUpdate, Enrolled, NewDevice};
pub use events::Event;
pub use folders::ShareChange;

use crate::model::Config;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T, E = StoreError> = std::result::Result<T, E>;

/// The database, cheap to clone.
#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    /// Opens (or creates) the database file and runs the migrations embedded in the binary.
    pub async fn open(path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite:")?
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    /// The whole configuration at its current revision, read in one transaction.
    pub async fn config(&self) -> Result<Config> {
        let mut tx = self.pool.begin().await?;
        let revision = revision(&mut tx).await?;
        let devices = devices::all(&mut *tx).await?;
        let (folders, shares) = folders::all(&mut tx).await?;
        tx.commit().await?;
        Ok(Config {
            revision,
            devices,
            folders,
            shares,
        })
    }

    /// A transaction that will write. `BEGIN IMMEDIATE` takes the write lock up front: a
    /// deferred transaction that reads first and writes later fails at once with "database is
    /// locked" when another connection wrote in between, without waiting for the busy timeout
    /// (https://sqlite.org/lang_transaction.html, "Read transactions versus write
    /// transactions"). Found by the tests on macOS.
    async fn write(&self) -> Result<Transaction<'static, Sqlite>> {
        Ok(self.pool.begin_with("BEGIN IMMEDIATE").await?)
    }

    pub async fn revision(&self) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let revision = revision(&mut tx).await?;
        tx.commit().await?;
        Ok(revision)
    }
}

async fn revision(tx: &mut Transaction<'_, Sqlite>) -> Result<u64> {
    let (revision,): (i64,) = sqlx::query_as("SELECT revision FROM config_rev WHERE id = 1")
        .fetch_one(&mut **tx)
        .await?;
    Ok(to_u64(revision))
}

/// Raises the revision inside the caller's transaction and returns the new one.
async fn bump(tx: &mut Transaction<'_, Sqlite>) -> Result<u64> {
    let (revision,): (i64,) = sqlx::query_as(
        "UPDATE config_rev SET revision = revision + 1 WHERE id = 1 RETURNING revision",
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok(to_u64(revision))
}

fn to_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

/// Seconds since the Unix epoch.
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

/// A new random ID for devices and folders.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
pub(crate) async fn test_device(store: &Store, public_key: &str) -> crate::model::Device {
    let code = format!("code-{public_key}");
    store.create_enrollment(&code, i64::MAX).await.unwrap();
    let device = NewDevice {
        name: "laptop".into(),
        kind: crate::model::DeviceKind::Laptop,
        os: "linux".into(),
        public_key: public_key.into(),
        syncthing_id: format!("ST-{public_key}"),
        agent_version: "0.1.0".into(),
        syncthing_version: "v2.1.6".into(),
    };
    store
        .enroll(&code, device, 1)
        .await
        .unwrap()
        .unwrap()
        .device
}

#[cfg(test)]
pub(crate) async fn test_store() -> (Store, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("stellwerk.db")).await.unwrap();
    (store, dir)
}
