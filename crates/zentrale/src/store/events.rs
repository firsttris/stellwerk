//! The activity feed.

use serde::Serialize;
use sqlx::FromRow;

use super::{Result, Store};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: i64,
    pub at: i64,
    pub device_id: Option<String>,
    pub folder_id: Option<String>,
    pub kind: String,
    pub text: String,
}

impl Store {
    pub async fn add_event(
        &self,
        at: i64,
        device_id: Option<&str>,
        folder_id: Option<&str>,
        kind: &str,
        text: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO events (at, device_id, folder_id, kind, text) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(at)
        .bind(device_id)
        .bind(folder_id)
        .bind(kind)
        .bind(text)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The newest events first.
    pub async fn events(&self, limit: u32) -> Result<Vec<Event>> {
        Ok(sqlx::query_as(
            "SELECT id, at, device_id, folder_id, kind, text FROM events ORDER BY id DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }
}
