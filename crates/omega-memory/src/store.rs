//! SQLite-backed persistent memory store.

use omega_core::{
    config::MemoryConfig,
    context::{Context, ContextEntry},
    error::OmegaError,
    message::{IncomingMessage, OutgoingMessage},
    shellexpand,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use tracing::info;
use uuid::Uuid;

/// How long (in minutes) before a conversation is considered idle.
const CONVERSATION_TIMEOUT_MINUTES: i64 = 30;

/// Persistent memory store backed by SQLite.
#[derive(Clone)]
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

    /// Run SQL migrations, tracking which have already been applied.
    async fn run_migrations(pool: &SqlitePool) -> Result<(), OmegaError> {
        // Create migration tracking table.
        sqlx::raw_sql(
            "CREATE TABLE IF NOT EXISTS _migrations (
                name TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .execute(pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("failed to create migrations table: {e}")))?;

        // Bootstrap: if _migrations is empty but tables already exist from
        // a pre-tracking era, mark all existing migrations as applied.
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _migrations")
            .fetch_one(pool)
            .await
            .map_err(|e| OmegaError::Memory(format!("failed to count migrations: {e}")))?;

        if count.0 == 0 {
            // Check if the schema already has the Phase 3 columns (summary on conversations).
            let has_summary: bool = sqlx::query_scalar::<_, String>(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='conversations'",
            )
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .map(|sql| sql.contains("summary"))
            .unwrap_or(false);

            if has_summary {
                // All 3 migrations were already applied before tracking existed.
                for name in &["001_init", "002_audit_log", "003_memory_enhancement"] {
                    sqlx::query("INSERT OR IGNORE INTO _migrations (name) VALUES (?)")
                        .bind(name)
                        .execute(pool)
                        .await
                        .map_err(|e| {
                            OmegaError::Memory(format!("failed to bootstrap migration {name}: {e}"))
                        })?;
                }
            }
        }

        let migrations: &[(&str, &str)] = &[
            ("001_init", include_str!("../migrations/001_init.sql")),
            (
                "002_audit_log",
                include_str!("../migrations/002_audit_log.sql"),
            ),
            (
                "003_memory_enhancement",
                include_str!("../migrations/003_memory_enhancement.sql"),
            ),
            (
                "004_fts5_recall",
                include_str!("../migrations/004_fts5_recall.sql"),
            ),
            (
                "005_scheduled_tasks",
                include_str!("../migrations/005_scheduled_tasks.sql"),
            ),
        ];

        for (name, sql) in migrations {
            let applied: Option<(String,)> =
                sqlx::query_as("SELECT name FROM _migrations WHERE name = ?")
                    .bind(name)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| {
                        OmegaError::Memory(format!("failed to check migration {name}: {e}"))
                    })?;

            if applied.is_some() {
                continue;
            }

            sqlx::raw_sql(sql)
                .execute(pool)
                .await
                .map_err(|e| OmegaError::Memory(format!("migration {name} failed: {e}")))?;

            sqlx::query("INSERT INTO _migrations (name) VALUES (?)")
                .bind(name)
                .execute(pool)
                .await
                .map_err(|e| {
                    OmegaError::Memory(format!("failed to record migration {name}: {e}"))
                })?;
        }
        Ok(())
    }

    /// Get or create an active conversation for a given channel + sender.
    ///
    /// Only returns conversations that are `active` AND have `last_activity`
    /// within the timeout window. Otherwise creates a new one.
    async fn get_or_create_conversation(
        &self,
        channel: &str,
        sender_id: &str,
    ) -> Result<String, OmegaError> {
        // Find active conversation within the timeout window.
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM conversations \
             WHERE channel = ? AND sender_id = ? AND status = 'active' \
             AND datetime(last_activity) > datetime('now', ? || ' minutes') \
             ORDER BY last_activity DESC LIMIT 1",
        )
        .bind(channel)
        .bind(sender_id)
        .bind(-CONVERSATION_TIMEOUT_MINUTES)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        if let Some((id,)) = row {
            // Update last_activity timestamp.
            sqlx::query(
                "UPDATE conversations SET last_activity = datetime('now'), updated_at = datetime('now') WHERE id = ?",
            )
            .bind(&id)
            .execute(&self.pool)
            .await
            .map_err(|e| OmegaError::Memory(format!("update failed: {e}")))?;
            return Ok(id);
        }

        // Create new conversation.
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO conversations (id, channel, sender_id, status, last_activity) \
             VALUES (?, ?, ?, 'active', datetime('now'))",
        )
        .bind(&id)
        .bind(channel)
        .bind(sender_id)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("insert failed: {e}")))?;

        Ok(id)
    }

    /// Find active conversations that have been idle beyond the timeout.
    pub async fn find_idle_conversations(
        &self,
    ) -> Result<Vec<(String, String, String)>, OmegaError> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, channel, sender_id FROM conversations \
             WHERE status = 'active' \
             AND datetime(last_activity) <= datetime('now', ? || ' minutes')",
        )
        .bind(-CONVERSATION_TIMEOUT_MINUTES)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        Ok(rows)
    }

    /// Find all active conversations (for shutdown).
    pub async fn find_all_active_conversations(
        &self,
    ) -> Result<Vec<(String, String, String)>, OmegaError> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, channel, sender_id FROM conversations WHERE status = 'active'",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        Ok(rows)
    }

    /// Get all messages for a conversation (for summarization).
    pub async fn get_conversation_messages(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<(String, String)>, OmegaError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT role, content FROM messages \
             WHERE conversation_id = ? ORDER BY timestamp ASC",
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        Ok(rows)
    }

    /// Close a conversation with a summary.
    pub async fn close_conversation(
        &self,
        conversation_id: &str,
        summary: &str,
    ) -> Result<(), OmegaError> {
        sqlx::query(
            "UPDATE conversations SET status = 'closed', summary = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(summary)
        .bind(conversation_id)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("update failed: {e}")))?;

        Ok(())
    }

    /// Store a fact (upsert by sender_id + key).
    pub async fn store_fact(
        &self,
        sender_id: &str,
        key: &str,
        value: &str,
    ) -> Result<(), OmegaError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO facts (id, sender_id, key, value) VALUES (?, ?, ?, ?) \
             ON CONFLICT(sender_id, key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
        )
        .bind(&id)
        .bind(sender_id)
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("upsert fact failed: {e}")))?;

        Ok(())
    }

    /// Get a single fact by sender and key.
    pub async fn get_fact(&self, sender_id: &str, key: &str) -> Result<Option<String>, OmegaError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM facts WHERE sender_id = ? AND key = ?")
                .bind(sender_id)
                .bind(key)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        Ok(row.map(|(v,)| v))
    }

    /// Delete a single fact by sender and key. Returns `true` if a row was deleted.
    pub async fn delete_fact(&self, sender_id: &str, key: &str) -> Result<bool, OmegaError> {
        let result = sqlx::query("DELETE FROM facts WHERE sender_id = ? AND key = ?")
            .bind(sender_id)
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| OmegaError::Memory(format!("delete failed: {e}")))?;

        Ok(result.rows_affected() > 0)
    }

    /// Get all facts for a sender.
    pub async fn get_facts(&self, sender_id: &str) -> Result<Vec<(String, String)>, OmegaError> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM facts WHERE sender_id = ? ORDER BY key")
                .bind(sender_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        Ok(rows)
    }

    /// Get recent closed conversation summaries for a sender.
    pub async fn get_recent_summaries(
        &self,
        channel: &str,
        sender_id: &str,
        limit: i64,
    ) -> Result<Vec<(String, String)>, OmegaError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT summary, updated_at FROM conversations \
             WHERE channel = ? AND sender_id = ? AND status = 'closed' AND summary IS NOT NULL \
             ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(channel)
        .bind(sender_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        Ok(rows)
    }

    /// Get memory statistics for a sender.
    pub async fn get_memory_stats(&self, sender_id: &str) -> Result<(i64, i64, i64), OmegaError> {
        let (conv_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM conversations WHERE sender_id = ?")
                .bind(sender_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        let (msg_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM messages m \
             JOIN conversations c ON m.conversation_id = c.id \
             WHERE c.sender_id = ?",
        )
        .bind(sender_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        let (fact_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM facts WHERE sender_id = ?")
                .bind(sender_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        Ok((conv_count, msg_count, fact_count))
    }

    /// Get conversation history (summaries with timestamps) for a sender.
    pub async fn get_history(
        &self,
        channel: &str,
        sender_id: &str,
        limit: i64,
    ) -> Result<Vec<(String, String)>, OmegaError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT COALESCE(summary, '(no summary)'), updated_at FROM conversations \
             WHERE channel = ? AND sender_id = ? AND status = 'closed' \
             ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(channel)
        .bind(sender_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        Ok(rows)
    }

    /// Delete facts for a sender — all facts if key is None, specific fact if key is Some.
    pub async fn delete_facts(
        &self,
        sender_id: &str,
        key: Option<&str>,
    ) -> Result<u64, OmegaError> {
        let result = if let Some(k) = key {
            sqlx::query("DELETE FROM facts WHERE sender_id = ? AND key = ?")
                .bind(sender_id)
                .bind(k)
                .execute(&self.pool)
                .await
        } else {
            sqlx::query("DELETE FROM facts WHERE sender_id = ?")
                .bind(sender_id)
                .execute(&self.pool)
                .await
        };

        result
            .map(|r| r.rows_affected())
            .map_err(|e| OmegaError::Memory(format!("delete failed: {e}")))
    }

    /// Close the current active conversation for a sender (for /forget).
    pub async fn close_current_conversation(
        &self,
        channel: &str,
        sender_id: &str,
    ) -> Result<bool, OmegaError> {
        let result = sqlx::query(
            "UPDATE conversations SET status = 'closed', updated_at = datetime('now') \
             WHERE channel = ? AND sender_id = ? AND status = 'active'",
        )
        .bind(channel)
        .bind(sender_id)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("update failed: {e}")))?;

        Ok(result.rows_affected() > 0)
    }

    /// Get the database file size in bytes.
    pub async fn db_size(&self) -> Result<u64, OmegaError> {
        let (page_count,): (i64,) = sqlx::query_as("PRAGMA page_count")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| OmegaError::Memory(format!("pragma failed: {e}")))?;

        let (page_size,): (i64,) = sqlx::query_as("PRAGMA page_size")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| OmegaError::Memory(format!("pragma failed: {e}")))?;

        Ok((page_count * page_size) as u64)
    }

    /// Build a conversation context from memory for the provider.
    pub async fn build_context(
        &self,
        incoming: &IncomingMessage,
        base_system_prompt: &str,
    ) -> Result<Context, OmegaError> {
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

        // Fetch facts, summaries, and recalled messages for enriched context.
        let facts = self
            .get_facts(&incoming.sender_id)
            .await
            .unwrap_or_default();
        let summaries = self
            .get_recent_summaries(&incoming.channel, &incoming.sender_id, 3)
            .await
            .unwrap_or_default();
        let recall = self
            .search_messages(&incoming.text, &conv_id, &incoming.sender_id, 5)
            .await
            .unwrap_or_default();
        let pending_tasks = self
            .get_tasks_for_sender(&incoming.sender_id)
            .await
            .unwrap_or_default();

        // Resolve language: stored preference > auto-detect > English.
        let language =
            if let Some((_, lang)) = facts.iter().find(|(k, _)| k == "preferred_language") {
                lang.clone()
            } else {
                let detected = detect_language(&incoming.text).to_string();
                let _ = self
                    .store_fact(&incoming.sender_id, "preferred_language", &detected)
                    .await;
                detected
            };

        let system_prompt = build_system_prompt(
            base_system_prompt,
            &facts,
            &summaries,
            &recall,
            &pending_tasks,
            &language,
        );

        Ok(Context {
            system_prompt,
            history,
            current_message: incoming.text.clone(),
        })
    }

    /// Search past messages across all conversations using FTS5 full-text search.
    pub async fn search_messages(
        &self,
        query: &str,
        exclude_conversation_id: &str,
        sender_id: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, String)>, OmegaError> {
        // Skip short queries — they produce noisy results.
        if query.len() < 3 {
            return Ok(Vec::new());
        }

        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT m.role, m.content, m.timestamp \
             FROM messages_fts fts \
             JOIN messages m ON m.rowid = fts.rowid \
             JOIN conversations c ON c.id = m.conversation_id \
             WHERE messages_fts MATCH ? \
             AND m.conversation_id != ? \
             AND c.sender_id = ? \
             ORDER BY rank \
             LIMIT ?",
        )
        .bind(query)
        .bind(exclude_conversation_id)
        .bind(sender_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("fts search failed: {e}")))?;

        Ok(rows)
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

    // --- Scheduled tasks ---

    /// Create a scheduled task.
    pub async fn create_task(
        &self,
        channel: &str,
        sender_id: &str,
        reply_target: &str,
        description: &str,
        due_at: &str,
        repeat: Option<&str>,
    ) -> Result<String, OmegaError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO scheduled_tasks (id, channel, sender_id, reply_target, description, due_at, repeat) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(channel)
        .bind(sender_id)
        .bind(reply_target)
        .bind(description)
        .bind(due_at)
        .bind(repeat)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("create task failed: {e}")))?;

        Ok(id)
    }

    /// Get tasks that are due for delivery.
    pub async fn get_due_tasks(
        &self,
    ) -> Result<Vec<(String, String, String, String, Option<String>)>, OmegaError> {
        // Returns: (id, channel, reply_target, description, repeat)
        let rows: Vec<(String, String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, channel, reply_target, description, repeat \
             FROM scheduled_tasks \
             WHERE status = 'pending' AND datetime(due_at) <= datetime('now')",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("get due tasks failed: {e}")))?;

        Ok(rows)
    }

    /// Complete a task: one-shot tasks become 'delivered', recurring tasks advance due_at.
    pub async fn complete_task(&self, id: &str, repeat: Option<&str>) -> Result<(), OmegaError> {
        match repeat {
            None | Some("once") => {
                sqlx::query(
                    "UPDATE scheduled_tasks SET status = 'delivered', delivered_at = datetime('now') WHERE id = ?",
                )
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| OmegaError::Memory(format!("complete task failed: {e}")))?;
            }
            Some(interval) => {
                let offset = match interval {
                    "daily" | "weekdays" => "+1 day",
                    "weekly" => "+7 days",
                    "monthly" => "+1 month",
                    _ => "+1 day",
                };

                // Advance due_at by interval.
                sqlx::query(&format!(
                    "UPDATE scheduled_tasks SET due_at = datetime(due_at, '{offset}') WHERE id = ?"
                ))
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| OmegaError::Memory(format!("advance task failed: {e}")))?;

                // For weekdays: skip Saturday (6) and Sunday (0).
                if interval == "weekdays" {
                    // If landed on Saturday (6), skip to Monday (+2 days).
                    sqlx::query(
                        "UPDATE scheduled_tasks SET due_at = datetime(due_at, '+2 days') \
                         WHERE id = ? AND CAST(strftime('%w', due_at) AS INTEGER) = 6",
                    )
                    .bind(id)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| OmegaError::Memory(format!("weekday skip sat failed: {e}")))?;

                    // If landed on Sunday (0), skip to Monday (+1 day).
                    sqlx::query(
                        "UPDATE scheduled_tasks SET due_at = datetime(due_at, '+1 day') \
                         WHERE id = ? AND CAST(strftime('%w', due_at) AS INTEGER) = 0",
                    )
                    .bind(id)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| OmegaError::Memory(format!("weekday skip sun failed: {e}")))?;
                }
            }
        }
        Ok(())
    }

    /// Get pending tasks for a sender (for /tasks command).
    pub async fn get_tasks_for_sender(
        &self,
        sender_id: &str,
    ) -> Result<Vec<(String, String, String, Option<String>)>, OmegaError> {
        // Returns: (id, description, due_at, repeat)
        let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, description, due_at, repeat \
             FROM scheduled_tasks \
             WHERE sender_id = ? AND status = 'pending' \
             ORDER BY due_at ASC",
        )
        .bind(sender_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("get tasks failed: {e}")))?;

        Ok(rows)
    }

    /// Cancel a task by ID prefix (must match sender).
    pub async fn cancel_task(&self, id_prefix: &str, sender_id: &str) -> Result<bool, OmegaError> {
        let result = sqlx::query(
            "UPDATE scheduled_tasks SET status = 'cancelled' \
             WHERE id LIKE ? AND sender_id = ? AND status = 'pending'",
        )
        .bind(format!("{id_prefix}%"))
        .bind(sender_id)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("cancel task failed: {e}")))?;

        Ok(result.rows_affected() > 0)
    }

    /// Get all facts across all users — for heartbeat context enrichment.
    pub async fn get_all_facts(&self) -> Result<Vec<(String, String)>, OmegaError> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM facts WHERE key != 'welcomed' ORDER BY key")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        Ok(rows)
    }

    /// Get recent conversation summaries across all users — for heartbeat context enrichment.
    pub async fn get_all_recent_summaries(
        &self,
        limit: i64,
    ) -> Result<Vec<(String, String)>, OmegaError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT summary, updated_at FROM conversations \
             WHERE status = 'closed' AND summary IS NOT NULL \
             ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        Ok(rows)
    }

    /// Check if a sender has never been welcomed (no `welcomed` fact).
    pub async fn is_new_user(&self, sender_id: &str) -> Result<bool, OmegaError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM facts WHERE sender_id = ? AND key = 'welcomed'")
                .bind(sender_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        Ok(row.is_none())
    }
}

/// Build a dynamic system prompt enriched with facts, conversation history, and recalled messages.
fn build_system_prompt(
    base_rules: &str,
    facts: &[(String, String)],
    summaries: &[(String, String)],
    recall: &[(String, String, String)],
    pending_tasks: &[(String, String, String, Option<String>)],
    language: &str,
) -> String {
    let mut prompt = String::from(base_rules);

    if !facts.is_empty() {
        prompt.push_str("\n\nKnown facts about this user:");
        for (key, value) in facts {
            prompt.push_str(&format!("\n- {key}: {value}"));
        }
    }

    if !summaries.is_empty() {
        prompt.push_str("\n\nRecent conversation history:");
        for (summary, timestamp) in summaries {
            prompt.push_str(&format!("\n- [{timestamp}] {summary}"));
        }
    }

    if !recall.is_empty() {
        prompt.push_str("\n\nRelated past context:");
        for (_role, content, timestamp) in recall {
            let truncated = if content.len() > 200 {
                format!("{}...", &content[..200])
            } else {
                content.clone()
            };
            prompt.push_str(&format!("\n- [{timestamp}] User: {truncated}"));
        }
    }

    if !pending_tasks.is_empty() {
        prompt.push_str("\n\nUser's scheduled tasks:");
        for (id, desc, due_at, repeat) in pending_tasks {
            let r = repeat.as_deref().unwrap_or("once");
            prompt.push_str(&format!(
                "\n- [{id_short}] {desc} (due: {due_at}, {r})",
                id_short = &id[..8.min(id.len())]
            ));
        }
    }

    prompt.push_str(&format!("\n\nIMPORTANT: Always respond in {language}."));

    prompt.push_str(
        "\n\nIf the user explicitly asks you to change language (e.g. 'speak in French'), \
         respond in the requested language. Include LANG_SWITCH: <language> on its own line \
         at the END of your response.",
    );

    prompt.push_str(
        "\n\nWhen the user asks to schedule, remind, or set a recurring task, \
         include this marker on its own line at the END of your response:\n\
         SCHEDULE: <description> | <ISO 8601 datetime> | <once|daily|weekly|monthly|weekdays>\n\
         Example: SCHEDULE: Call John | 2026-02-17T15:00:00 | once\n\
         Only include the marker if the user explicitly asks for a reminder or scheduled task.",
    );

    prompt.push_str(
        "\n\nWhen the user asks you to monitor, watch, keep an eye on, or add something to \
         your periodic checklist, include this marker on its own line at the END of your response:\n\
         HEARTBEAT_ADD: <description>\n\
         When the user asks to stop monitoring, unwatch, or remove something from the checklist:\n\
         HEARTBEAT_REMOVE: <description>\n\
         Only include the marker if the user explicitly asks to add or remove a monitored item.",
    );

    prompt
}

/// Detect the most likely language of a text using stop-word heuristics.
/// Returns a language name like "English", "Spanish", etc.
pub fn detect_language(text: &str) -> &'static str {
    let lower = text.to_lowercase();

    let languages: &[(&str, &[&str])] = &[
        (
            "Spanish",
            &[
                " que ", " por ", " para ", " como ", " con ", " una ", " los ", " las ", " del ",
                " pero ", "hola", "gracias", "necesito", "quiero", "puedes",
            ],
        ),
        (
            "Portuguese",
            &[
                " que ", " com ", " para ", " uma ", " dos ", " das ", " não ", " mais ", " tem ",
                " isso ", "olá", "obrigado", "preciso", "você",
            ],
        ),
        (
            "French",
            &[
                " que ", " les ", " des ", " une ", " est ", " pas ", " pour ", " dans ", " avec ",
                " sur ", "bonjour", "merci", " je ", " nous ",
            ],
        ),
        (
            "German",
            &[
                " und ", " der ", " die ", " das ", " ist ", " nicht ", " ein ", " eine ", " ich ",
                " auf ", " mit ", " für ", " den ", "hallo",
            ],
        ),
        (
            "Italian",
            &[
                " che ", " per ", " con ", " una ", " gli ", " non ", " sono ", " della ", " nel ",
                " questo ", "ciao", "grazie", " io ", " anche ",
            ],
        ),
        (
            "Dutch",
            &[
                " de ", " het ", " een ", " van ", " en ", " niet ", " dat ", " met ", " voor ",
                " zijn ", " ook ", " maar ", "hallo", " ik ",
            ],
        ),
        (
            "Russian",
            &[
                " и ",
                " в ",
                " не ",
                " на ",
                " что ",
                " это ",
                " как ",
                " но ",
                " от ",
                " по ",
                "привет",
                "спасибо",
                " мне ",
                " для ",
            ],
        ),
    ];

    let mut best = "English";
    let mut best_score = 0usize;

    for (lang, words) in languages {
        let score = words.iter().filter(|w| lower.contains(**w)).count();
        if score > best_score {
            best_score = score;
            best = lang;
        }
    }

    // Short messages (≤3 words): 1 match suffices (e.g. "hola", "bonjour").
    // Longer messages: require 2+ to avoid false positives.
    let word_count = lower.split_whitespace().count();
    let threshold = if word_count <= 3 { 1 } else { 2 };
    if best_score >= threshold {
        best
    } else {
        "English"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create an in-memory store for testing.
    async fn test_store() -> Store {
        let config = MemoryConfig {
            backend: "sqlite".to_string(),
            db_path: ":memory:".to_string(),
            max_context_messages: 10,
        };
        // For in-memory, we need to bypass shellexpand.
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        Store::run_migrations(&pool).await.unwrap();
        Store {
            pool,
            max_context_messages: config.max_context_messages,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_tasks() {
        let store = test_store().await;
        let id = store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "Call John",
                "2026-12-31T15:00:00",
                None,
            )
            .await
            .unwrap();
        assert!(!id.is_empty());

        let tasks = store.get_tasks_for_sender("user1").await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].1, "Call John");
        assert_eq!(tasks[0].2, "2026-12-31T15:00:00");
        assert!(tasks[0].3.is_none());
    }

    #[tokio::test]
    async fn test_get_due_tasks() {
        let store = test_store().await;
        // Create a task in the past (due now).
        store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "Past task",
                "2020-01-01T00:00:00",
                None,
            )
            .await
            .unwrap();
        // Create a task in the future.
        store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "Future task",
                "2099-12-31T23:59:59",
                None,
            )
            .await
            .unwrap();

        let due = store.get_due_tasks().await.unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].3, "Past task");
    }

    #[tokio::test]
    async fn test_complete_one_shot() {
        let store = test_store().await;
        let id = store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "One shot",
                "2020-01-01T00:00:00",
                None,
            )
            .await
            .unwrap();

        store.complete_task(&id, None).await.unwrap();

        // Should no longer appear in pending.
        let tasks = store.get_tasks_for_sender("user1").await.unwrap();
        assert!(tasks.is_empty());

        // Should not appear in due tasks either.
        let due = store.get_due_tasks().await.unwrap();
        assert!(due.is_empty());
    }

    #[tokio::test]
    async fn test_complete_recurring() {
        let store = test_store().await;
        let id = store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "Daily standup",
                "2020-01-01T09:00:00",
                Some("daily"),
            )
            .await
            .unwrap();

        store.complete_task(&id, Some("daily")).await.unwrap();

        // Task should still be pending but with advanced due_at.
        let tasks = store.get_tasks_for_sender("user1").await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].2, "2020-01-02 09:00:00"); // Advanced by 1 day
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let store = test_store().await;
        let id = store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "Cancel me",
                "2099-12-31T00:00:00",
                None,
            )
            .await
            .unwrap();

        let prefix = &id[..8];
        let cancelled = store.cancel_task(prefix, "user1").await.unwrap();
        assert!(cancelled);

        let tasks = store.get_tasks_for_sender("user1").await.unwrap();
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn test_cancel_task_wrong_sender() {
        let store = test_store().await;
        let id = store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "My task",
                "2099-12-31T00:00:00",
                None,
            )
            .await
            .unwrap();

        let prefix = &id[..8];
        let cancelled = store.cancel_task(prefix, "user2").await.unwrap();
        assert!(!cancelled);

        // Task still exists.
        let tasks = store.get_tasks_for_sender("user1").await.unwrap();
        assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_get_fact() {
        let store = test_store().await;
        // Missing fact returns None.
        assert!(store.get_fact("user1", "color").await.unwrap().is_none());

        store.store_fact("user1", "color", "blue").await.unwrap();
        assert_eq!(
            store.get_fact("user1", "color").await.unwrap(),
            Some("blue".to_string())
        );
    }

    #[tokio::test]
    async fn test_delete_fact() {
        let store = test_store().await;
        // Delete non-existent returns false.
        assert!(!store.delete_fact("user1", "color").await.unwrap());

        store.store_fact("user1", "color", "blue").await.unwrap();
        assert!(store.delete_fact("user1", "color").await.unwrap());
        assert!(store.get_fact("user1", "color").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_is_new_user() {
        let store = test_store().await;

        // New user — no welcomed fact yet.
        assert!(store.is_new_user("fresh_user").await.unwrap());

        // Store the welcomed fact.
        store
            .store_fact("fresh_user", "welcomed", "true")
            .await
            .unwrap();

        // No longer new.
        assert!(!store.is_new_user("fresh_user").await.unwrap());
    }

    #[tokio::test]
    async fn test_get_all_facts() {
        let store = test_store().await;

        // Empty store returns empty vec.
        let facts = store.get_all_facts().await.unwrap();
        assert!(facts.is_empty());

        // Store some facts across different users.
        store.store_fact("user1", "name", "Alice").await.unwrap();
        store.store_fact("user2", "name", "Bob").await.unwrap();
        store.store_fact("user1", "timezone", "EST").await.unwrap();
        // Store a welcomed fact — should be excluded.
        store.store_fact("user1", "welcomed", "true").await.unwrap();

        let facts = store.get_all_facts().await.unwrap();
        assert_eq!(facts.len(), 3, "should exclude 'welcomed' facts");
        assert!(facts.iter().any(|(k, v)| k == "name" && v == "Alice"));
        assert!(facts.iter().any(|(k, v)| k == "name" && v == "Bob"));
        assert!(facts.iter().any(|(k, v)| k == "timezone" && v == "EST"));
    }

    #[tokio::test]
    async fn test_get_all_recent_summaries() {
        let store = test_store().await;

        // Empty store returns empty vec.
        let summaries = store.get_all_recent_summaries(3).await.unwrap();
        assert!(summaries.is_empty());

        // Create a conversation, close it with a summary.
        sqlx::query(
            "INSERT INTO conversations (id, channel, sender_id, status, summary, last_activity, updated_at) \
             VALUES ('c1', 'telegram', 'user1', 'closed', 'Discussed project planning', datetime('now'), datetime('now'))",
        )
        .execute(store.pool())
        .await
        .unwrap();

        let summaries = store.get_all_recent_summaries(3).await.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].0, "Discussed project planning");
    }
}
