//! DeviceStore trait implementation for SqlxWhatsAppStore.
//!
//! Handles device identity persistence (save/load/exists/create).

use async_trait::async_trait;
use wacore::store::error::{db_err, StoreError};
use wacore::store::traits::DeviceStore;
use wacore::store::Device;

use super::SqlxWhatsAppStore;

type Result<T> = wacore::store::error::Result<T>;

#[async_trait]
impl DeviceStore for SqlxWhatsAppStore {
    async fn save(&self, device: &Device) -> Result<()> {
        // Device uses custom serde (key_pair_serde, BigArray) that requires
        // a binary format — serde_json cannot handle deserialize_bytes.
        let data =
            bincode::serialize(device).map_err(|e| StoreError::Serialization(e.to_string()))?;
        sqlx::query("INSERT OR REPLACE INTO wa_device_info (id, data) VALUES (1, ?)")
            .bind(&data)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn load(&self) -> Result<Option<Device>> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT data FROM wa_device_info WHERE id = 1")
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;

        match row {
            Some((data,)) => {
                let device = bincode::deserialize(&data)
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                Ok(Some(device))
            }
            None => Ok(None),
        }
    }

    async fn exists(&self) -> Result<bool> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM wa_device_info WHERE id = 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(row.is_some())
    }

    async fn create(&self) -> Result<i32> {
        // Return a device ID. The actual Device data is populated during
        // pairing/key generation and stored via save().
        Ok(1)
    }
}
