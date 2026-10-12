//! Admin password, login sessions and one-time enrollment codes. Only hashes are stored.

use sqlx::FromRow;

use super::{Result, Store};

/// A logged-in browser: the session cookie's hash and the CSRF token the UI echoes back.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct Session {
    pub token_hash: String,
    pub csrf_token: String,
    pub expires_at: i64,
}

impl Store {
    pub async fn admin_password_hash(&self) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT password_hash FROM admin WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(hash,)| hash))
    }

    /// Sets the password and logs out every session.
    pub async fn set_admin_password_hash(&self, hash: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO admin (id, password_hash) VALUES (1, ?) \
             ON CONFLICT (id) DO UPDATE SET password_hash = excluded.password_hash",
        )
        .bind(hash)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM sessions")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn create_session(&self, session: &Session) -> Result<()> {
        sqlx::query("INSERT INTO sessions (token_hash, csrf_token, expires_at) VALUES (?, ?, ?)")
            .bind(&session.token_hash)
            .bind(&session.csrf_token)
            .bind(session.expires_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The session if it exists and has not expired; expired ones are cleared on the way.
    pub async fn session(&self, token_hash: &str, now: i64) -> Result<Option<Session>> {
        sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(sqlx::query_as(
            "SELECT token_hash, csrf_token, expires_at FROM sessions WHERE token_hash = ?",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn delete_session(&self, token_hash: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(token_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn create_enrollment(&self, code_hash: &str, expires_at: i64) -> Result<()> {
        sqlx::query("INSERT INTO enrollments (code_hash, expires_at) VALUES (?, ?)")
            .bind(code_hash)
            .bind(expires_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The device that used a code, once it has: lets the UI's pairing dialog move on.
    pub async fn enrolled_device(&self, code_hash: &str) -> Result<Option<String>> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT device_id FROM enrollments WHERE code_hash = ?")
                .bind(code_hash)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(device,)| device))
    }
}

#[cfg(test)]
mod tests {
    use crate::store::{Session, test_store};

    #[tokio::test]
    async fn sessions_expire_and_a_new_password_ends_them() {
        let (store, _dir) = test_store().await;
        let session = |hash: &str, expires_at| Session {
            token_hash: hash.into(),
            csrf_token: "csrf".into(),
            expires_at,
        };
        store.create_session(&session("live", 100)).await.unwrap();
        store.create_session(&session("old", 10)).await.unwrap();

        assert_eq!(store.session("old", 50).await.unwrap(), None);
        assert_eq!(
            store.session("live", 50).await.unwrap(),
            Some(session("live", 100))
        );

        store.set_admin_password_hash("hash").await.unwrap();
        assert_eq!(store.session("live", 50).await.unwrap(), None);
        assert_eq!(
            store.admin_password_hash().await.unwrap().as_deref(),
            Some("hash")
        );
    }
}
