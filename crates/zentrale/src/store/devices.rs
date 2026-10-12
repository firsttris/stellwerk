//! Devices: enrolling with a one-time code or by approval in the UI, updates on every hello,
//! renaming, blocking.

use sqlx::{FromRow, Sqlite, SqliteExecutor, Transaction};

use super::{Result, Store, StoreError, bump, new_id, to_u64};
use crate::model::{Device, DeviceKind};

/// What an agent reports about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDevice {
    pub name: String,
    pub kind: DeviceKind,
    pub os: String,
    pub public_key: String,
    pub syncthing_id: String,
    pub agent_version: String,
    pub syncthing_version: String,
}

/// The device a pairing code was used for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enrolled {
    pub device: Device,
    /// `false`: the key was paired before; the code only confirmed it.
    pub new: bool,
}

/// Changes from the UI; `None` leaves a field as it is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceUpdate {
    pub name: Option<String>,
    pub kind: Option<DeviceKind>,
    pub approved: Option<bool>,
}

#[derive(FromRow)]
struct Row {
    id: String,
    name: String,
    kind: String,
    os: String,
    public_key: String,
    syncthing_id: String,
    agent_version: String,
    syncthing_version: String,
    approved: bool,
    created_at: i64,
    last_seen: Option<i64>,
    applied_rev: Option<i64>,
}

impl From<Row> for Device {
    fn from(row: Row) -> Self {
        Self {
            id: row.id,
            name: row.name,
            kind: DeviceKind::parse(&row.kind),
            os: row.os,
            public_key: row.public_key,
            syncthing_id: row.syncthing_id,
            agent_version: row.agent_version,
            syncthing_version: row.syncthing_version,
            approved: row.approved,
            created_at: row.created_at,
            last_seen: row.last_seen,
            applied_rev: row.applied_rev.map(to_u64),
        }
    }
}

/// The column list as a literal, so the queries stay literal SQL.
macro_rules! select_devices {
    ($rest:literal) => {
        concat!(
            "SELECT id, name, kind, os, public_key, syncthing_id, agent_version, ",
            "syncthing_version, approved, created_at, last_seen, applied_rev FROM devices ",
            $rest
        )
    };
}

pub(super) async fn all(db: impl SqliteExecutor<'_>) -> Result<Vec<Device>> {
    let rows: Vec<Row> = sqlx::query_as(select_devices!("ORDER BY created_at, id"))
        .fetch_all(db)
        .await?;
    Ok(rows.into_iter().map(Device::from).collect())
}

impl Store {
    pub async fn device(&self, id: &str) -> Result<Device> {
        let row: Option<Row> = sqlx::query_as(select_devices!("WHERE id = ?"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(Device::from).ok_or(StoreError::NotFound("device"))
    }

    pub async fn device_by_key(&self, public_key: &str) -> Result<Option<Device>> {
        let row: Option<Row> = sqlx::query_as(select_devices!("WHERE public_key = ?"))
            .bind(public_key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(Device::from))
    }

    /// Uses up an unexpired code; `None` when the code is unknown, expired or already used.
    /// An unknown key becomes a new device, a new peer for the hub: new revision. A known key
    /// (`join` again on a paired device, which keeps its key) gets its own device back and the
    /// code is linked to it, so the pairing dialog moves on. A blocked device leaves the code
    /// unused.
    pub async fn enroll(
        &self,
        code_hash: &str,
        device: NewDevice,
        now: i64,
    ) -> Result<Option<Enrolled>> {
        let mut tx = self.write().await?;
        let known: Option<(String, bool)> =
            sqlx::query_as("SELECT id, approved FROM devices WHERE public_key = ?")
                .bind(&device.public_key)
                .fetch_optional(&mut *tx)
                .await?;
        let blocked = known.as_ref().is_some_and(|(_, approved)| !approved);
        if blocked || !take_code(&mut tx, code_hash, now).await? {
            return Ok(None);
        }
        let (id, new) = match known {
            Some((id, _)) => (id, false),
            None => (insert_device(&mut tx, &device, now).await?, true),
        };
        sqlx::query("UPDATE enrollments SET device_id = ? WHERE code_hash = ?")
            .bind(&id)
            .bind(code_hash)
            .execute(&mut *tx)
            .await?;
        if new {
            bump(&mut tx).await?;
        }
        tx.commit().await?;
        let device = self.device(&id).await?;
        Ok(Some(Enrolled { device, new }))
    }

    /// Adds a device approved in the UI without a code (LAN pairing).
    pub async fn add_device(&self, device: NewDevice, now: i64) -> Result<Device> {
        let mut tx = self.write().await?;
        let id = insert_device(&mut tx, &device, now).await?;
        bump(&mut tx).await?;
        tx.commit().await?;
        self.device(&id).await
    }

    /// Records what an agent reported on connecting. A new Syncthing ID (fresh Syncthing home)
    /// changes the hub's peers: new revision. Returns whether the revision was raised.
    pub async fn device_seen(&self, id: &str, report: &NewDevice, now: i64) -> Result<bool> {
        let mut tx = self.write().await?;
        let (old_id,): (String,) = sqlx::query_as("SELECT syncthing_id FROM devices WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(StoreError::NotFound("device"))?;
        sqlx::query(
            "UPDATE devices SET os = ?, syncthing_id = ?, agent_version = ?, \
             syncthing_version = ?, last_seen = ? WHERE id = ?",
        )
        .bind(&report.os)
        .bind(&report.syncthing_id)
        .bind(&report.agent_version)
        .bind(&report.syncthing_version)
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        let changed = old_id != report.syncthing_id;
        if changed {
            bump(&mut tx).await?;
        }
        tx.commit().await?;
        Ok(changed)
    }

    pub async fn touch_device(&self, id: &str, now: i64) -> Result<()> {
        sqlx::query("UPDATE devices SET last_seen = ? WHERE id = ?")
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_applied(&self, id: &str, revision: u64, now: i64) -> Result<()> {
        sqlx::query("UPDATE devices SET applied_rev = ?, last_seen = ? WHERE id = ?")
            .bind(i64::try_from(revision).unwrap_or(i64::MAX))
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Renames, changes the icon or blocks/unblocks. Only blocking changes desired states.
    pub async fn update_device(&self, id: &str, update: &DeviceUpdate) -> Result<Device> {
        let mut tx = self.write().await?;
        let found = sqlx::query(
            "UPDATE devices SET name = COALESCE(?, name), kind = COALESCE(?, kind), \
             approved = COALESCE(?, approved) WHERE id = ?",
        )
        .bind(update.name.as_deref())
        .bind(update.kind.map(DeviceKind::as_str))
        .bind(update.approved)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if found.rows_affected() == 0 {
            return Err(StoreError::NotFound("device"));
        }
        if update.approved.is_some() {
            bump(&mut tx).await?;
        }
        tx.commit().await?;
        self.device(id).await
    }

    /// Removes the device and its shares; its files stay where they are.
    pub async fn delete_device(&self, id: &str) -> Result<()> {
        let mut tx = self.write().await?;
        let deleted = sqlx::query("DELETE FROM devices WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() == 0 {
            return Err(StoreError::NotFound("device"));
        }
        bump(&mut tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

/// Marks an unexpired, unused code as used; `false` when there is no such code.
async fn take_code(tx: &mut Transaction<'_, Sqlite>, code_hash: &str, now: i64) -> Result<bool> {
    let used = sqlx::query(
        "UPDATE enrollments SET used_at = ? \
         WHERE code_hash = ? AND used_at IS NULL AND expires_at > ?",
    )
    .bind(now)
    .bind(code_hash)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(used.rows_affected() > 0)
}

/// A new, approved device; refuses a key that is already known.
async fn insert_device(
    tx: &mut Transaction<'_, Sqlite>,
    device: &NewDevice,
    now: i64,
) -> Result<String> {
    let known: Option<(String,)> = sqlx::query_as("SELECT id FROM devices WHERE public_key = ?")
        .bind(&device.public_key)
        .fetch_optional(&mut **tx)
        .await?;
    if known.is_some() {
        return Err(StoreError::Invalid("this key is already enrolled".into()));
    }
    let id = new_id();
    sqlx::query(
        "INSERT INTO devices (id, name, kind, os, public_key, syncthing_id, agent_version, \
         syncthing_version, approved, created_at, last_seen) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
    )
    .bind(&id)
    .bind(&device.name)
    .bind(device.kind.as_str())
    .bind(&device.os)
    .bind(&device.public_key)
    .bind(&device.syncthing_id)
    .bind(&device.agent_version)
    .bind(&device.syncthing_version)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_store;

    fn laptop() -> NewDevice {
        NewDevice {
            name: "laptop".into(),
            kind: DeviceKind::Laptop,
            os: "linux".into(),
            public_key: "KEY".into(),
            syncthing_id: "ST-1".into(),
            agent_version: "0.1.0".into(),
            syncthing_version: "v2.1.6".into(),
        }
    }

    #[tokio::test]
    async fn a_code_enrolls_once_and_only_before_it_expires() {
        let (store, _dir) = test_store().await;
        store.create_enrollment("fresh", 100).await.unwrap();
        store.create_enrollment("old", 10).await.unwrap();

        assert_eq!(store.enroll("old", laptop(), 50).await.unwrap(), None);
        assert_eq!(store.enroll("unknown", laptop(), 50).await.unwrap(), None);
        let enrolled = store.enroll("fresh", laptop(), 50).await.unwrap().unwrap();
        assert!(enrolled.new);
        let device = enrolled.device;
        assert_eq!(device.name, "laptop");
        assert!(device.approved);
        assert_eq!(store.revision().await.unwrap(), 1);

        let mut again = laptop();
        again.public_key = "OTHER".into();
        assert_eq!(store.enroll("fresh", again, 60).await.unwrap(), None);
        assert_eq!(
            store.device_by_key("KEY").await.unwrap().map(|d| d.id),
            Some(device.id)
        );
    }

    #[tokio::test]
    async fn a_known_key_with_a_fresh_code_gets_its_device_back() {
        let (store, _dir) = test_store().await;
        store.create_enrollment("first", 100).await.unwrap();
        store.create_enrollment("second", 100).await.unwrap();
        let device = store.enroll("first", laptop(), 1).await.unwrap().unwrap();

        let again = store.enroll("second", laptop(), 2).await.unwrap().unwrap();
        assert!(!again.new);
        assert_eq!(again.device.id, device.device.id);
        assert_eq!(store.revision().await.unwrap(), 1, "no new peer");
        assert_eq!(
            store.enrolled_device("second").await.unwrap(),
            Some(device.device.id.clone()),
            "the pairing dialog sees the code used"
        );
        assert_eq!(store.config().await.unwrap().devices.len(), 1);

        // A blocked device does not use up a code.
        let block = DeviceUpdate {
            approved: Some(false),
            ..DeviceUpdate::default()
        };
        store
            .update_device(&device.device.id, &block)
            .await
            .unwrap();
        store.create_enrollment("third", 100).await.unwrap();
        assert_eq!(store.enroll("third", laptop(), 3).await.unwrap(), None);
        assert_eq!(store.enrolled_device("third").await.unwrap(), None);
    }

    #[tokio::test]
    async fn only_a_new_syncthing_id_or_blocking_raises_the_revision() {
        let (store, _dir) = test_store().await;
        store.create_enrollment("code", 100).await.unwrap();
        let device = store
            .enroll("code", laptop(), 1)
            .await
            .unwrap()
            .unwrap()
            .device;

        assert!(!store.device_seen(&device.id, &laptop(), 2).await.unwrap());
        let mut fresh = laptop();
        fresh.syncthing_id = "ST-2".into();
        assert!(store.device_seen(&device.id, &fresh, 3).await.unwrap());
        assert_eq!(store.revision().await.unwrap(), 2);

        let rename = DeviceUpdate {
            name: Some("work".into()),
            ..DeviceUpdate::default()
        };
        assert_eq!(
            store.update_device(&device.id, &rename).await.unwrap().name,
            "work"
        );
        assert_eq!(store.revision().await.unwrap(), 2);

        let block = DeviceUpdate {
            approved: Some(false),
            ..DeviceUpdate::default()
        };
        assert!(
            !store
                .update_device(&device.id, &block)
                .await
                .unwrap()
                .approved
        );
        assert_eq!(store.revision().await.unwrap(), 3);

        store.set_applied(&device.id, 3, 4).await.unwrap();
        assert_eq!(store.device(&device.id).await.unwrap().applied_rev, Some(3));
    }
}
