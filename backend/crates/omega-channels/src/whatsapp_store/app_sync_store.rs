//! AppSyncStore trait implementation for SqlxWhatsAppStore.
//!
//! Handles app state sync keys, collection versions, and mutation MACs.

use async_trait::async_trait;
use wacore::appstate::hash::HashState;
use wacore::appstate::processor::AppStateMutationMAC;
use wacore::store::error::{db_err, StoreError};
use wacore::store::traits::{AppStateSyncKey, AppSyncStore};

use super::SqlxWhatsAppStore;

type Result<T> = wacore::store::error::Result<T>;

#[async_trait]
impl AppSyncStore for SqlxWhatsAppStore {
    async fn get_sync_key(&self, key_id: &[u8]) -> Result<Option<AppStateSyncKey>> {
        let row: Option<(Vec<u8>, i64, Option<Vec<u8>>)> = sqlx::query_as(
            "SELECT key_data, timestamp, fingerprint FROM wa_app_sync_keys WHERE key_id = ?",
        )
        .bind(key_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(
            row.map(|(key_data, timestamp, fingerprint)| AppStateSyncKey {
                key_data,
                timestamp,
                fingerprint: fingerprint.unwrap_or_default(),
            }),
        )
    }

    async fn set_sync_key(&self, key_id: &[u8], key: AppStateSyncKey) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO wa_app_sync_keys (key_id, key_data, timestamp, fingerprint) VALUES (?, ?, ?, ?)",
        )
        .bind(key_id)
        .bind(&key.key_data)
        .bind(key.timestamp)
        .bind(&key.fingerprint)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn get_version(&self, name: &str) -> Result<HashState> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT data FROM wa_app_versions WHERE collection = ?")
                .bind(name)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;

        match row {
            Some((data,)) => {
                serde_json::from_str(&data).map_err(|e| StoreError::Serialization(e.to_string()))
            }
            None => Ok(HashState::default()),
        }
    }

    async fn set_version(&self, name: &str, state: HashState) -> Result<()> {
        let data =
            serde_json::to_string(&state).map_err(|e| StoreError::Serialization(e.to_string()))?;
        sqlx::query("INSERT OR REPLACE INTO wa_app_versions (collection, data) VALUES (?, ?)")
            .bind(name)
            .bind(&data)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn put_mutation_macs(
        &self,
        name: &str,
        version: u64,
        mutations: &[AppStateMutationMAC],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        for m in mutations {
            sqlx::query(
                "INSERT OR REPLACE INTO wa_mutation_macs (collection, index_mac, version, value_mac) VALUES (?, ?, ?, ?)",
            )
            .bind(name)
            .bind(&m.index_mac)
            .bind(version as i64)
            .bind(&m.value_mac)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

    async fn get_mutation_mac(&self, name: &str, index_mac: &[u8]) -> Result<Option<Vec<u8>>> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as(
            "SELECT value_mac FROM wa_mutation_macs WHERE collection = ? AND index_mac = ?",
        )
        .bind(name)
        .bind(index_mac)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(|(d,)| d))
    }

    async fn delete_mutation_macs(&self, name: &str, index_macs: &[Vec<u8>]) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        for mac in index_macs {
            sqlx::query("DELETE FROM wa_mutation_macs WHERE collection = ? AND index_mac = ?")
                .bind(name)
                .bind(mac)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }
}
