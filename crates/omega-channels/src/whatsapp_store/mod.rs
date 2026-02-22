//! SQLx-based storage backend for `whatsapp-rust`.
//!
//! Implements the `Backend` trait (SignalStore + AppSyncStore + ProtocolStore + DeviceStore)
//! using sqlx with SQLite, avoiding the `libsqlite3-sys` version conflict between
//! sqlx and diesel (used by `whatsapp-rust-sqlite-storage`).

mod app_sync_store;
mod device_store;
mod protocol_store;
mod signal_store;

use sqlx::{Pool, Sqlite, SqlitePool};

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
