//! SQLite-backed persistent memory store.

use omega_core::{
    config::MemoryConfig,
    context::{Context, ContextEntry},
    error::OmegaError,
    message::{IncomingMessage, OutgoingMessage},
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use tracing::info;
use uuid::Uuid;

/// Persistent memory store backed by SQLite.
pub struct Store {
    pool: SqlitePool,
    max_context_messages: usize,
}

impl Store {
    /// Create a new store, running migrations on first use.
    pub async fn new(config: &MemoryConfig) -> Result<Self, OmegaError> {
        let db_path = shellexpand(&config.db_path);

        // Ensure parent directory exists.
        if let Some(parent) = std::path::Path::new(&db_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| OmegaError::Memory(format!("failed to create data dir: {e}")))?;
        }

        let opts = SqliteConnectOptions::from_str(&format!("sqlite:{db_path}"))
            .map_err(|e| OmegaError::Memory(format!("invalid db path: {e}")))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await
            .map_err(|e| OmegaError::Memory(format!("failed to connect to sqlite: {e}")))?;

        // Run migrations.
        Self::run_migrations(&pool).await?;

        info!("Memory store initialized at {db_path}");

        Ok(Self {
            pool,
            max_context_messages: config.max_context_messages,
        })
    }

    /// Get a reference to the underlying connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Run SQL migrations.
    async fn run_migrations(pool: &SqlitePool) -> Result<(), OmegaError> {
        for migration in &[
            include_str!("../migrations/001_init.sql"),
            include_str!("../migrations/002_audit_log.sql"),
        ] {
            sqlx::raw_sql(migration)
                .execute(pool)
                .await
                .map_err(|e| OmegaError::Memory(format!("migration failed: {e}")))?;
        }
        Ok(())
    }

    /// Get or create a conversation for a given channel + sender.
    async fn get_or_create_conversation(
        &self,
        channel: &str,
        sender_id: &str,
    ) -> Result<String, OmegaError> {
        // Try to find existing conversation.
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM conversations WHERE channel = ? AND sender_id = ? ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(channel)
        .bind(sender_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        if let Some((id,)) = row {
            // Update timestamp.
            sqlx::query("UPDATE conversations SET updated_at = datetime('now') WHERE id = ?")
                .bind(&id)
                .execute(&self.pool)
                .await
                .map_err(|e| OmegaError::Memory(format!("update failed: {e}")))?;
            return Ok(id);
        }

        // Create new conversation.
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO conversations (id, channel, sender_id) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(channel)
            .bind(sender_id)
            .execute(&self.pool)
            .await
            .map_err(|e| OmegaError::Memory(format!("insert failed: {e}")))?;

        Ok(id)
    }

    /// Build a conversation context from memory for the provider.
    pub async fn build_context(&self, incoming: &IncomingMessage) -> Result<Context, OmegaError> {
        let conv_id = self
            .get_or_create_conversation(&incoming.channel, &incoming.sender_id)
            .await?;

        // Load recent messages from this conversation.
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT role, content FROM messages WHERE conversation_id = ? ORDER BY timestamp DESC LIMIT ?",
        )
        .bind(&conv_id)
        .bind(self.max_context_messages as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        // Rows come newest-first, reverse for chronological order.
        let history: Vec<ContextEntry> = rows
            .into_iter()
            .rev()
            .map(|(role, content)| ContextEntry { role, content })
            .collect();

        Ok(Context {
            system_prompt: default_system_prompt(),
            history,
            current_message: incoming.text.clone(),
        })
    }

    /// Store a user message and assistant response.
    pub async fn store_exchange(
        &self,
        incoming: &IncomingMessage,
        response: &OutgoingMessage,
    ) -> Result<(), OmegaError> {
        let conv_id = self
            .get_or_create_conversation(&incoming.channel, &incoming.sender_id)
            .await?;

        // Store user message.
        let user_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO messages (id, conversation_id, role, content) VALUES (?, ?, 'user', ?)",
        )
        .bind(&user_id)
        .bind(&conv_id)
        .bind(&incoming.text)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("insert failed: {e}")))?;

        // Store assistant response.
        let asst_id = Uuid::new_v4().to_string();
        let metadata_json = serde_json::to_string(&response.metadata)
            .map_err(|e| OmegaError::Memory(format!("serialize failed: {e}")))?;

        sqlx::query(
            "INSERT INTO messages (id, conversation_id, role, content, metadata_json) VALUES (?, ?, 'assistant', ?, ?)",
        )
        .bind(&asst_id)
        .bind(&conv_id)
        .bind(&response.text)
        .bind(&metadata_json)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("insert failed: {e}")))?;

        Ok(())
    }
}

/// Expand `~` to home directory.
fn shellexpand(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return format!("{}/{rest}", home.to_string_lossy());
        }
    }
    path.to_string()
}

/// Default system prompt.
fn default_system_prompt() -> String {
    "You are Omega, a personal AI assistant running on the user's own server. \
     You are helpful, concise, and action-oriented. \
     When the user asks you to do something, DO IT — don't just explain how."
        .to_string()
}
