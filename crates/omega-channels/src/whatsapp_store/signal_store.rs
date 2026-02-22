//! SignalStore trait implementation for SqlxWhatsAppStore.
//!
//! Handles identities, sessions, prekeys, signed prekeys, and sender keys.

use async_trait::async_trait;
use wacore::store::error::db_err;
use wacore::store::traits::SignalStore;

use super::SqlxWhatsAppStore;

type Result<T> = wacore::store::error::Result<T>;

#[async_trait]
impl SignalStore for SqlxWhatsAppStore {
    async fn put_identity(&self, address: &str, key: [u8; 32]) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO wa_identities (address, key_data) VALUES (?, ?)")
            .bind(address)
            .bind(key.as_slice())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn load_identity(&self, address: &str) -> Result<Option<Vec<u8>>> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT key_data FROM wa_identities WHERE address = ?")
                .bind(address)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        Ok(row.map(|(d,)| d))
    }

    async fn delete_identity(&self, address: &str) -> Result<()> {
        sqlx::query("DELETE FROM wa_identities WHERE address = ?")
            .bind(address)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn get_session(&self, address: &str) -> Result<Option<Vec<u8>>> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT session_data FROM wa_sessions WHERE address = ?")
                .bind(address)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        Ok(row.map(|(d,)| d))
    }

    async fn put_session(&self, address: &str, session: &[u8]) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO wa_sessions (address, session_data) VALUES (?, ?)")
            .bind(address)
            .bind(session)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn delete_session(&self, address: &str) -> Result<()> {
        sqlx::query("DELETE FROM wa_sessions WHERE address = ?")
            .bind(address)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn store_prekey(&self, id: u32, record: &[u8], uploaded: bool) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO wa_prekeys (id, record, uploaded) VALUES (?, ?, ?)")
            .bind(id as i64)
            .bind(record)
            .bind(uploaded as i32)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn load_prekey(&self, id: u32) -> Result<Option<Vec<u8>>> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as("SELECT record FROM wa_prekeys WHERE id = ?")
            .bind(id as i64)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(row.map(|(d,)| d))
    }

    async fn remove_prekey(&self, id: u32) -> Result<()> {
        sqlx::query("DELETE FROM wa_prekeys WHERE id = ?")
            .bind(id as i64)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn store_signed_prekey(&self, id: u32, record: &[u8]) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO wa_signed_prekeys (id, record) VALUES (?, ?)")
            .bind(id as i64)
            .bind(record)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn load_signed_prekey(&self, id: u32) -> Result<Option<Vec<u8>>> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT record FROM wa_signed_prekeys WHERE id = ?")
                .bind(id as i64)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        Ok(row.map(|(d,)| d))
    }

    async fn load_all_signed_prekeys(&self) -> Result<Vec<(u32, Vec<u8>)>> {
        let rows: Vec<(i64, Vec<u8>)> = sqlx::query_as("SELECT id, record FROM wa_signed_prekeys")
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(rows.into_iter().map(|(id, r)| (id as u32, r)).collect())
    }

    async fn remove_signed_prekey(&self, id: u32) -> Result<()> {
        sqlx::query("DELETE FROM wa_signed_prekeys WHERE id = ?")
            .bind(id as i64)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn put_sender_key(&self, address: &str, record: &[u8]) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO wa_sender_keys (address, record) VALUES (?, ?)")
            .bind(address)
            .bind(record)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn get_sender_key(&self, address: &str) -> Result<Option<Vec<u8>>> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT record FROM wa_sender_keys WHERE address = ?")
                .bind(address)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        Ok(row.map(|(d,)| d))
    }

    async fn delete_sender_key(&self, address: &str) -> Result<()> {
        sqlx::query("DELETE FROM wa_sender_keys WHERE address = ?")
            .bind(address)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }
}
