//! SQLx-based storage backend for `whatsapp-rust`.
//!
//! Implements the `Backend` trait (SignalStore + AppSyncStore + ProtocolStore + DeviceStore)
//! using sqlx with SQLite, avoiding the `libsqlite3-sys` version conflict between
//! sqlx and diesel (used by `whatsapp-rust-sqlite-storage`).

use async_trait::async_trait;
use sqlx::{Pool, Sqlite, SqlitePool};
use wacore::appstate::hash::HashState;
use wacore::appstate::processor::AppStateMutationMAC;
use wacore::store::error::{db_err, StoreError};
use wacore::store::traits::{
    AppStateSyncKey, AppSyncStore, DeviceListRecord, DeviceStore, LidPnMappingEntry, ProtocolStore,
    SignalStore,
};
use wacore::store::Device;

type Result<T> = wacore::store::error::Result<T>;

/// SQLx-backed WhatsApp session store.
pub struct SqlxWhatsAppStore {
    pool: Pool<Sqlite>,
}

impl SqlxWhatsAppStore {
    /// Create a new store and initialize the schema.
    pub async fn new(db_path: &str) -> std::result::Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(&format!("sqlite:{db_path}?mode=rwc")).await?;
        Self::init_schema(&pool).await?;
        Ok(Self { pool })
    }

    async fn init_schema(pool: &Pool<Sqlite>) -> std::result::Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_identities (
                address TEXT PRIMARY KEY,
                key_data BLOB NOT NULL
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_sessions (
                address TEXT PRIMARY KEY,
                session_data BLOB NOT NULL
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_prekeys (
                id INTEGER PRIMARY KEY,
                record BLOB NOT NULL,
                uploaded INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_signed_prekeys (
                id INTEGER PRIMARY KEY,
                record BLOB NOT NULL
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_sender_keys (
                address TEXT PRIMARY KEY,
                record BLOB NOT NULL
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_app_sync_keys (
                key_id BLOB PRIMARY KEY,
                key_data BLOB NOT NULL,
                timestamp INTEGER NOT NULL DEFAULT 0,
                fingerprint BLOB
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_app_versions (
                collection TEXT PRIMARY KEY,
                data TEXT NOT NULL
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_mutation_macs (
                collection TEXT NOT NULL,
                index_mac BLOB NOT NULL,
                version INTEGER NOT NULL,
                value_mac BLOB NOT NULL,
                PRIMARY KEY (collection, index_mac)
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_skdm_recipients (
                group_jid TEXT NOT NULL,
                device_jid TEXT NOT NULL,
                PRIMARY KEY (group_jid, device_jid)
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_lid_mappings (
                lid TEXT PRIMARY KEY,
                phone_number TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0,
                learning_source TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_base_keys (
                address TEXT NOT NULL,
                message_id TEXT NOT NULL,
                base_key BLOB NOT NULL,
                PRIMARY KEY (address, message_id)
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_device_lists (
                user TEXT PRIMARY KEY,
                data TEXT NOT NULL
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_forget_sender_keys (
                group_jid TEXT NOT NULL,
                participant TEXT NOT NULL,
                PRIMARY KEY (group_jid, participant)
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wa_device_info (
                id INTEGER PRIMARY KEY,
                data BLOB NOT NULL
            )",
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}

// --- SignalStore ---

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

// --- AppSyncStore ---

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
        for m in mutations {
            sqlx::query(
                "INSERT OR REPLACE INTO wa_mutation_macs (collection, index_mac, version, value_mac) VALUES (?, ?, ?, ?)",
            )
            .bind(name)
            .bind(&m.index_mac)
            .bind(version as i64)
            .bind(&m.value_mac)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        }
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
        for mac in index_macs {
            sqlx::query("DELETE FROM wa_mutation_macs WHERE collection = ? AND index_mac = ?")
                .bind(name)
                .bind(mac)
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
        }
        Ok(())
    }
}

// --- ProtocolStore ---

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

// --- DeviceStore ---

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
