//! API tokens for scripts and AI assistants. Only the hash is stored; the token itself is shown
//! once, when it is created.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::{Result, Store, StoreError, new_id};

/// What a token may do. Everything else (devices, pairing, conflicts, versions, tokens) needs
/// the admin's login in the browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    /// Overview, live status, activity, browsing and searching on devices.
    Read,
    /// Also create and rename folders, set ignore patterns, apply shares.
    Manage,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Manage => "manage",
        }
    }

    /// Unknown text reads as the narrower scope.
    pub fn parse(text: &str) -> Self {
        match text {
            "manage" => Self::Manage,
            _ => Self::Read,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiToken {
    pub id: String,
    pub name: String,
    pub scope: Scope,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

#[derive(FromRow)]
struct Row {
    id: String,
    name: String,
    scope: String,
    created_at: i64,
    last_used_at: Option<i64>,
}

impl From<Row> for ApiToken {
    fn from(row: Row) -> Self {
        Self {
            id: row.id,
            name: row.name,
            scope: Scope::parse(&row.scope),
            created_at: row.created_at,
            last_used_at: row.last_used_at,
        }
    }
}

impl Store {
    pub async fn create_api_token(
        &self,
        name: &str,
        scope: Scope,
        token_hash: &str,
        now: i64,
    ) -> Result<ApiToken> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StoreError::Invalid("a token needs a name".into()));
        }
        let token = ApiToken {
            id: new_id(),
            name: name.to_string(),
            scope,
            created_at: now,
            last_used_at: None,
        };
        sqlx::query(
            "INSERT INTO api_tokens (id, name, scope, token_hash, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&token.id)
        .bind(&token.name)
        .bind(scope.as_str())
        .bind(token_hash)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(token)
    }

    /// Newest first.
    pub async fn api_tokens(&self) -> Result<Vec<ApiToken>> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT id, name, scope, created_at, last_used_at FROM api_tokens \
             ORDER BY created_at DESC, name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(ApiToken::from).collect())
    }

    pub async fn delete_api_token(&self, id: &str) -> Result<()> {
        let done = sqlx::query("DELETE FROM api_tokens WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if done.rows_affected() == 0 {
            return Err(StoreError::NotFound("token"));
        }
        Ok(())
    }

    /// The token with this hash, noting that it was used now; `None` for an unknown or revoked
    /// one.
    pub async fn use_api_token(&self, token_hash: &str, now: i64) -> Result<Option<ApiToken>> {
        let row: Option<Row> = sqlx::query_as(
            "UPDATE api_tokens SET last_used_at = ? WHERE token_hash = ? \
             RETURNING id, name, scope, created_at, last_used_at",
        )
        .bind(now)
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(ApiToken::from))
    }
}

#[cfg(test)]
mod tests {
    use super::Scope;
    use crate::store::test_store;

    #[tokio::test]
    async fn tokens_are_found_by_hash_until_revoked() {
        let (store, _dir) = test_store().await;
        let token = store
            .create_api_token(" claude ", Scope::Manage, "hash", 10)
            .await
            .unwrap();
        assert_eq!(token.name, "claude");
        assert_eq!(token.last_used_at, None);

        let used = store.use_api_token("hash", 20).await.unwrap().unwrap();
        assert_eq!(used.scope, Scope::Manage);
        assert_eq!(used.last_used_at, Some(20));
        assert_eq!(store.use_api_token("other", 20).await.unwrap(), None);
        assert_eq!(store.api_tokens().await.unwrap(), vec![used]);

        store.delete_api_token(&token.id).await.unwrap();
        assert_eq!(store.use_api_token("hash", 30).await.unwrap(), None);
        assert!(store.delete_api_token(&token.id).await.is_err());
    }

    #[tokio::test]
    async fn a_token_needs_a_name() {
        let (store, _dir) = test_store().await;
        assert!(
            store
                .create_api_token("  ", Scope::Read, "hash", 10)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_new_password_revokes_every_token() {
        let (store, _dir) = test_store().await;
        store
            .create_api_token("script", Scope::Read, "hash", 10)
            .await
            .unwrap();
        store.set_admin_password_hash("new").await.unwrap();
        assert!(store.api_tokens().await.unwrap().is_empty());
    }

    #[test]
    fn an_unknown_scope_reads_as_read() {
        assert_eq!(Scope::parse("manage"), Scope::Manage);
        assert_eq!(Scope::parse("admin"), Scope::Read);
    }
}
