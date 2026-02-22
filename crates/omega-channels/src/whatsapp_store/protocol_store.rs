//! ProtocolStore trait implementation for SqlxWhatsAppStore.
//!
//! Handles SKDM recipients, LID/PN mappings, base keys, device lists,
//! and forget-sender-key marks.

use async_trait::async_trait;
use wacore::store::error::{db_err, StoreError};
use wacore::store::traits::{DeviceListRecord, LidPnMappingEntry, ProtocolStore};

use super::SqlxWhatsAppStore;

type Result<T> = wacore::store::error::Result<T>;

#[async_trait]
impl ProtocolStore for SqlxWhatsAppStore {
    async fn get_skdm_recipients(&self, group_jid: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT device_jid FROM wa_skdm_recipients WHERE group_jid = ?")
                .bind(group_jid)
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?;
        Ok(rows.into_iter().map(|(s,)| s).collect())
    }

    async fn add_skdm_recipients(&self, group_jid: &str, device_jids: &[String]) -> Result<()> {
        for device in device_jids {
            sqlx::query(
                "INSERT OR IGNORE INTO wa_skdm_recipients (group_jid, device_jid) VALUES (?, ?)",
            )
            .bind(group_jid)
            .bind(device)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        }
        Ok(())
    }

    async fn clear_skdm_recipients(&self, group_jid: &str) -> Result<()> {
        sqlx::query("DELETE FROM wa_skdm_recipients WHERE group_jid = ?")
            .bind(group_jid)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn get_lid_mapping(&self, lid: &str) -> Result<Option<LidPnMappingEntry>> {
        let row: Option<(String, i64, i64, String)> = sqlx::query_as(
            "SELECT phone_number, created_at, updated_at, learning_source FROM wa_lid_mappings WHERE lid = ?",
        )
        .bind(lid)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(row.map(
            |(phone_number, created_at, updated_at, learning_source)| LidPnMappingEntry {
                lid: lid.to_string(),
                phone_number,
                created_at,
                updated_at,
                learning_source,
            },
        ))
    }

    async fn get_pn_mapping(&self, phone: &str) -> Result<Option<LidPnMappingEntry>> {
        let row: Option<(String, i64, i64, String)> = sqlx::query_as(
            "SELECT lid, created_at, updated_at, learning_source FROM wa_lid_mappings WHERE phone_number = ?",
        )
        .bind(phone)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(row.map(
            |(lid, created_at, updated_at, learning_source)| LidPnMappingEntry {
                lid,
                phone_number: phone.to_string(),
                created_at,
                updated_at,
                learning_source,
            },
        ))
    }

    async fn put_lid_mapping(&self, entry: &LidPnMappingEntry) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO wa_lid_mappings (lid, phone_number, created_at, updated_at, learning_source) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&entry.lid)
        .bind(&entry.phone_number)
        .bind(entry.created_at)
        .bind(entry.updated_at)
        .bind(&entry.learning_source)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn get_all_lid_mappings(&self) -> Result<Vec<LidPnMappingEntry>> {
        let rows: Vec<(String, String, i64, i64, String)> = sqlx::query_as(
            "SELECT lid, phone_number, created_at, updated_at, learning_source FROM wa_lid_mappings",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(rows
            .into_iter()
            .map(
                |(lid, phone_number, created_at, updated_at, learning_source)| LidPnMappingEntry {
                    lid,
                    phone_number,
                    created_at,
                    updated_at,
                    learning_source,
                },
            )
            .collect())
    }

    async fn save_base_key(&self, address: &str, message_id: &str, base_key: &[u8]) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO wa_base_keys (address, message_id, base_key) VALUES (?, ?, ?)",
        )
        .bind(address)
        .bind(message_id)
        .bind(base_key)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn has_same_base_key(
        &self,
        address: &str,
        message_id: &str,
        current_base_key: &[u8],
    ) -> Result<bool> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as(
            "SELECT base_key FROM wa_base_keys WHERE address = ? AND message_id = ?",
        )
        .bind(address)
        .bind(message_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(|(k,)| k == current_base_key).unwrap_or(false))
    }

    async fn delete_base_key(&self, address: &str, message_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM wa_base_keys WHERE address = ? AND message_id = ?")
            .bind(address)
            .bind(message_id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn update_device_list(&self, record: DeviceListRecord) -> Result<()> {
        let data =
            serde_json::to_string(&record).map_err(|e| StoreError::Serialization(e.to_string()))?;
        sqlx::query("INSERT OR REPLACE INTO wa_device_lists (user, data) VALUES (?, ?)")
            .bind(&record.user)
            .bind(&data)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn get_devices(&self, user: &str) -> Result<Option<DeviceListRecord>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT data FROM wa_device_lists WHERE user = ?")
                .bind(user)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;

        match row {
            Some((data,)) => {
                let record = serde_json::from_str(&data)
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    async fn mark_forget_sender_key(&self, group_jid: &str, participant: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO wa_forget_sender_keys (group_jid, participant) VALUES (?, ?)",
        )
        .bind(group_jid)
        .bind(participant)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn consume_forget_marks(&self, group_jid: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT participant FROM wa_forget_sender_keys WHERE group_jid = ?")
                .bind(group_jid)
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?;

        sqlx::query("DELETE FROM wa_forget_sender_keys WHERE group_jid = ?")
            .bind(group_jid)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;

        Ok(rows.into_iter().map(|(s,)| s).collect())
    }
}
