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
            (
                "006_limitations",
                include_str!("../migrations/006_limitations.sql"),
            ),
            (
                "007_task_type",
                include_str!("../migrations/007_task_type.sql"),
            ),
            (
                "008_user_aliases",
                include_str!("../migrations/008_user_aliases.sql"),
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

        // Progressive onboarding: compute stage and inject hint on transitions.
        let real_fact_count = facts
            .iter()
            .filter(|(k, _)| !SYSTEM_FACT_KEYS.contains(&k.as_str()))
            .count();
        let has_tasks = !pending_tasks.is_empty();

        let current_stage: u8 = facts
            .iter()
            .find(|(k, _)| k == "onboarding_stage")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0);

        let new_stage = compute_onboarding_stage(current_stage, real_fact_count, has_tasks);

        let onboarding_hint = if new_stage != current_stage {
            // Stage advanced — store it and show the hint for the NEW stage.
            let _ = self
                .store_fact(
                    &incoming.sender_id,
                    "onboarding_stage",
                    &new_stage.to_string(),
                )
                .await;
            Some(new_stage)
        } else if current_stage == 0 && real_fact_count == 0 {
            // First contact — no stored stage yet, show intro.
            Some(0u8)
        } else {
            // Pre-existing user with no stage fact: silently store current stage, no hint.
            if facts.iter().all(|(k, _)| k != "onboarding_stage") && current_stage == 0 {
                let bootstrapped = compute_onboarding_stage(0, real_fact_count, has_tasks);
                // Walk through all stages up to current state.
                let final_stage = (0..=4).fold(0u8, |s, _| {
                    compute_onboarding_stage(s, real_fact_count, has_tasks)
                });
                if final_stage > 0 {
                    let _ = self
                        .store_fact(
                            &incoming.sender_id,
                            "onboarding_stage",
                            &final_stage.to_string(),
                        )
                        .await;
                }
                let _ = bootstrapped; // suppress unused warning
                None
            } else {
                None
            }
        };

        let system_prompt = build_system_prompt(
            base_system_prompt,
            &facts,
            &summaries,
            &recall,
            &pending_tasks,
            &language,
            onboarding_hint,
        );

        Ok(Context {
            system_prompt,
            history,
            current_message: incoming.text.clone(),
            mcp_servers: Vec::new(),
            max_turns: None,
            allowed_tools: None,
            model: None,
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

    /// Create a scheduled task. Deduplicates on two levels:
    /// 1. Exact match: same sender + description + normalized due_at.
    /// 2. Fuzzy match: same sender + similar description + due_at within 30 min.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_task(
        &self,
        channel: &str,
        sender_id: &str,
        reply_target: &str,
        description: &str,
        due_at: &str,
        repeat: Option<&str>,
        task_type: &str,
    ) -> Result<String, OmegaError> {
        let normalized_due = normalize_due_at(due_at);

        // Level 1: exact dedup on (sender, description, normalized due_at).
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM scheduled_tasks \
             WHERE sender_id = ? AND description = ? AND due_at = ? AND status = 'pending' \
             LIMIT 1",
        )
        .bind(sender_id)
        .bind(description)
        .bind(&normalized_due)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("dedup check failed: {e}")))?;

        if let Some((id,)) = existing {
            tracing::info!("scheduled task dedup: reusing existing {id}");
            return Ok(id);
        }

        // Level 2: fuzzy dedup — same sender, similar description, due_at within 30 min.
        let nearby: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, description, due_at FROM scheduled_tasks \
             WHERE sender_id = ? AND status = 'pending' \
             AND abs(strftime('%s', ?) - strftime('%s', due_at)) <= 1800",
        )
        .bind(sender_id)
        .bind(&normalized_due)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("fuzzy dedup check failed: {e}")))?;

        for (existing_id, existing_desc, _) in &nearby {
            if descriptions_are_similar(description, existing_desc) {
                tracing::info!(
                    "scheduled task fuzzy dedup: reusing {existing_id} (similar to new)"
                );
                return Ok(existing_id.clone());
            }
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO scheduled_tasks (id, channel, sender_id, reply_target, description, due_at, repeat, task_type) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(channel)
        .bind(sender_id)
        .bind(reply_target)
        .bind(description)
        .bind(&normalized_due)
        .bind(repeat)
        .bind(task_type)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("create task failed: {e}")))?;

        Ok(id)
    }

    /// Get tasks that are due for delivery.
    #[allow(clippy::type_complexity)]
    pub async fn get_due_tasks(
        &self,
    ) -> Result<
        Vec<(
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            String,
        )>,
        OmegaError,
    > {
        // Returns: (id, channel, sender_id, reply_target, description, repeat, task_type)
        let rows: Vec<(
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            String,
        )> = sqlx::query_as(
            "SELECT id, channel, sender_id, reply_target, description, repeat, task_type \
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
    ) -> Result<Vec<(String, String, String, Option<String>, String)>, OmegaError> {
        // Returns: (id, description, due_at, repeat, task_type)
        let rows: Vec<(String, String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT id, description, due_at, repeat, task_type \
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
        let prefix = format!("{id_prefix}%");

        // Try to cancel pending tasks first.
        let result = sqlx::query(
            "UPDATE scheduled_tasks SET status = 'cancelled' \
             WHERE id LIKE ? AND sender_id = ? AND status = 'pending'",
        )
        .bind(&prefix)
        .bind(sender_id)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("cancel task failed: {e}")))?;

        if result.rows_affected() > 0 {
            return Ok(true);
        }

        // Idempotent: if already cancelled, treat as success.
        let already: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM scheduled_tasks \
             WHERE id LIKE ? AND sender_id = ? AND status = 'cancelled'",
        )
        .bind(&prefix)
        .bind(sender_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("cancel task check failed: {e}")))?;

        Ok(already.0 > 0)
    }

    /// Update fields of a pending task by ID prefix (must match sender).
    ///
    /// Only non-`None` fields are updated. Returns `true` if a row was modified.
    pub async fn update_task(
        &self,
        id_prefix: &str,
        sender_id: &str,
        description: Option<&str>,
        due_at: Option<&str>,
        repeat: Option<&str>,
    ) -> Result<bool, OmegaError> {
        let mut sets = Vec::new();
        let mut values: Vec<String> = Vec::new();

        if let Some(d) = description {
            sets.push("description = ?");
            values.push(d.to_string());
        }
        if let Some(d) = due_at {
            sets.push("due_at = ?");
            values.push(d.to_string());
        }
        if let Some(r) = repeat {
            sets.push("repeat = ?");
            values.push(r.to_string());
        }

        if sets.is_empty() {
            return Ok(false);
        }

        let sql = format!(
            "UPDATE scheduled_tasks SET {} WHERE id LIKE ? AND sender_id = ? AND status = 'pending'",
            sets.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for v in &values {
            query = query.bind(v);
        }
        query = query.bind(format!("{id_prefix}%"));
        query = query.bind(sender_id);

        let result = query
            .execute(&self.pool)
            .await
            .map_err(|e| OmegaError::Memory(format!("update task failed: {e}")))?;

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

    // --- Limitations ---

    /// Store a limitation (deduplicates by title, case-insensitive).
    /// Returns `true` if the limitation is new, `false` if it already existed.
    pub async fn store_limitation(
        &self,
        title: &str,
        description: &str,
        proposed_plan: &str,
    ) -> Result<bool, OmegaError> {
        let id = Uuid::new_v4().to_string();
        let result = sqlx::query(
            "INSERT OR IGNORE INTO limitations (id, title, description, proposed_plan) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(title)
        .bind(description)
        .bind(proposed_plan)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("store limitation failed: {e}")))?;

        Ok(result.rows_affected() > 0)
    }

    /// Get all open limitations: (title, description, proposed_plan).
    pub async fn get_open_limitations(&self) -> Result<Vec<(String, String, String)>, OmegaError> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT title, description, proposed_plan FROM limitations \
             WHERE status = 'open' ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("get open limitations failed: {e}")))?;

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

    /// Resolve a sender_id to its canonical form via the user_aliases table.
    /// Returns the canonical sender_id if an alias exists, otherwise the original.
    pub async fn resolve_sender_id(&self, sender_id: &str) -> Result<String, OmegaError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT canonical_sender_id FROM user_aliases WHERE alias_sender_id = ?",
        )
        .bind(sender_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("resolve alias failed: {e}")))?;

        Ok(row.map(|(id,)| id).unwrap_or_else(|| sender_id.to_string()))
    }

    /// Create an alias mapping: alias_id → canonical_id.
    pub async fn create_alias(&self, alias_id: &str, canonical_id: &str) -> Result<(), OmegaError> {
        sqlx::query(
            "INSERT OR IGNORE INTO user_aliases (alias_sender_id, canonical_sender_id) \
             VALUES (?, ?)",
        )
        .bind(alias_id)
        .bind(canonical_id)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("create alias failed: {e}")))?;

        Ok(())
    }

    /// Find an existing welcomed user different from `sender_id` and return their sender_id.
    /// Used to create cross-channel aliases (e.g., WhatsApp phone → Telegram ID).
    pub async fn find_canonical_user(
        &self,
        exclude_sender_id: &str,
    ) -> Result<Option<String>, OmegaError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT sender_id FROM facts WHERE key = 'welcomed' AND sender_id != ? LIMIT 1",
        )
        .bind(exclude_sender_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        Ok(row.map(|(id,)| id))
    }
}

/// System fact keys filtered out of the user profile.
const SYSTEM_FACT_KEYS: &[&str] = &[
    "welcomed",
    "preferred_language",
    "active_project",
    "personality",
    "onboarding_stage",
];

/// Identity fact keys — shown first in the user profile.
const IDENTITY_KEYS: &[&str] = &["name", "preferred_name", "pronouns"];

/// Context fact keys — shown second in the user profile.
const CONTEXT_KEYS: &[&str] = &["timezone", "location", "occupation"];

/// Format user facts into a structured profile, filtering system keys
/// and grouping identity facts first, then context, then the rest.
///
/// Returns an empty string when only system facts exist.
pub fn format_user_profile(facts: &[(String, String)]) -> String {
    let user_facts: Vec<&(String, String)> = facts
        .iter()
        .filter(|(k, _)| !SYSTEM_FACT_KEYS.contains(&k.as_str()))
        .collect();

    if user_facts.is_empty() {
        return String::new();
    }

    let mut lines = vec!["User profile:".to_string()];

    // Identity group first.
    for key in IDENTITY_KEYS {
        if let Some((_, v)) = user_facts.iter().find(|(k, _)| k == key) {
            lines.push(format!("- {key}: {v}"));
        }
    }

    // Context group second.
    for key in CONTEXT_KEYS {
        if let Some((_, v)) = user_facts.iter().find(|(k, _)| k == key) {
            lines.push(format!("- {key}: {v}"));
        }
    }

    // Everything else, preserving original order.
    let known_keys: Vec<&str> = IDENTITY_KEYS
        .iter()
        .chain(CONTEXT_KEYS.iter())
        .copied()
        .collect();
    for (k, v) in &user_facts {
        if !known_keys.contains(&k.as_str()) {
            lines.push(format!("- {k}: {v}"));
        }
    }

    lines.join("\n")
}

/// Compute the next onboarding stage based on current state.
///
/// Stages are sequential — can't skip. Each fires exactly once then advances.
/// - Stage 0: First contact (intro)
/// - Stage 1: 1+ real facts → teach /help
/// - Stage 2: 3+ real facts → teach personality
/// - Stage 3: First task created → teach task management
/// - Stage 4: 5+ real facts → teach projects
/// - Stage 5: Done (no more hints)
fn compute_onboarding_stage(current_stage: u8, real_fact_count: usize, has_tasks: bool) -> u8 {
    match current_stage {
        0 if real_fact_count >= 1 => 1,
        1 if real_fact_count >= 3 => 2,
        2 if has_tasks => 3,
        3 if real_fact_count >= 5 => 4,
        4 => 5,
        _ => current_stage,
    }
}

/// Return the onboarding hint text for a given stage, or `None` if no hint.
fn onboarding_hint_text(stage: u8, language: &str) -> Option<String> {
    match stage {
        0 => Some(format!(
            "\n\nThis is your first conversation with this person. Respond ONLY with this \
             introduction in {language} (adapt naturally, do NOT translate literally):\n\n\
             Start with '\u{1f44b}' followed by an appropriate greeting in {language} on the same line.\n\n\
             Glad to have them here. You are *OMEGA \u{03a9}* (always bold), their personal agent — \
             but before jumping into action, you'd like to get to know them a bit.\n\n\
             Ask their name and what they do, so you can be more useful from the start.\n\n\
             Do NOT mention infrastructure, Rust, Claude, or any technical details. \
             Do NOT answer their message yet. Just this introduction, nothing else.",
        )),
        1 => Some(format!(
            "\n\nOnboarding hint: This person is new. At the end of your response, \
             casually mention that they can ask you anything or type /help to see what you can do. \
             Keep it brief and natural — one sentence max. Respond in {language}."
        )),
        2 => Some(format!(
            "\n\nOnboarding hint: This person hasn't customized your personality yet. \
             At the end of your response, casually mention they can tell you how to behave \
             (e.g. 'be more casual') or use /personality. One sentence max, only if it fits naturally. \
             Respond in {language}."
        )),
        3 => Some(format!(
            "\n\nOnboarding hint: This person just created their first task! \
             At the end of your response, briefly mention they can say 'show my tasks' \
             or type /tasks to see scheduled items. One sentence max. Respond in {language}."
        )),
        4 => Some(format!(
            "\n\nOnboarding hint: This person is getting comfortable. \
             At the end of your response, briefly mention they can organize work into projects — \
             just say 'create a project' or type /projects to see how. One sentence max. \
             Respond in {language}."
        )),
        _ => None,
    }
}

/// Build a dynamic system prompt enriched with facts, conversation history, and recalled messages.
fn build_system_prompt(
    base_rules: &str,
    facts: &[(String, String)],
    summaries: &[(String, String)],
    recall: &[(String, String, String)],
    pending_tasks: &[(String, String, String, Option<String>, String)],
    language: &str,
    onboarding_hint: Option<u8>,
) -> String {
    let mut prompt = String::from(base_rules);

    let profile = format_user_profile(facts);
    if !profile.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(&profile);
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
        for (id, desc, due_at, repeat, task_type) in pending_tasks {
            let r = repeat.as_deref().unwrap_or("once");
            let type_badge = if task_type == "action" {
                " [action]"
            } else {
                ""
            };
            prompt.push_str(&format!(
                "\n- [{id_short}] {desc}{type_badge} (due: {due_at}, {r})",
                id_short = &id[..8.min(id.len())]
            ));
        }
    }

    prompt.push_str(&format!("\n\nIMPORTANT: Always respond in {language}."));

    // Progressive onboarding: inject hint only when a stage transition fires.
    if let Some(stage) = onboarding_hint {
        if let Some(hint) = onboarding_hint_text(stage, language) {
            prompt.push_str(&hint);
        }
    }

    prompt.push_str(
        "\n\nIf the user explicitly asks you to change language (e.g. 'speak in French'), \
         respond in the requested language. Include LANG_SWITCH: <language> on its own line \
         at the END of your response.",
    );

    prompt.push_str(
        "\n\nTo schedule a task, include this marker on its own line at the END of your response:\n\
         SCHEDULE: <description> | <ISO 8601 datetime> | <once|daily|weekly|monthly|weekdays>\n\
         Example: SCHEDULE: Call John | 2026-02-17T15:00:00 | once\n\
         Use this when the user asks for a reminder AND proactively after any action you take \
         that warrants follow-up. After every action, ask yourself: does this need a check later? \
         If yes, schedule it. An autonomous agent closes its own loops.",
    );

    prompt.push_str(
        "\n\nAction Tasks: For tasks that require you to EXECUTE an action (not just remind), \
         use this marker on its own line at the END of your response:\n\
         SCHEDULE_ACTION: <what to do> | <ISO 8601 datetime> | <once|daily|weekly|monthly|weekdays>\n\
         When the time comes, you will be invoked with full tool access to carry out the action \
         autonomously. Use SCHEDULE for reminders (user needs to act), SCHEDULE_ACTION for \
         actions (you need to act).",
    );

    prompt.push_str(
        "\n\nTo add something to your periodic monitoring checklist, include this marker on its \
         own line at the END of your response:\n\
         HEARTBEAT_ADD: <description>\n\
         To remove something from monitoring:\n\
         HEARTBEAT_REMOVE: <description>\n\
         Use this when the user asks AND proactively when any action you take needs ongoing \
         monitoring. If something you did will evolve over time and could need attention, \
         add it to your watchlist. Don't wait to be told to keep an eye on your own actions.\n\
         To change the heartbeat check interval, include this marker on its own line:\n\
         HEARTBEAT_INTERVAL: <minutes>\n\
         Value must be between 1 and 1440 (24 hours). Use when the user asks to change how \
         often you check in (e.g., \"check every 15 minutes\").",
    );

    prompt.push_str(
        "\n\nSelf-Introspection: You are self-aware of your capabilities and limitations. \
         When you encounter something you CANNOT do but SHOULD be able to (missing tools, \
         unavailable services, missing integrations), report it using this marker on its own line:\n\
         LIMITATION: <short title> | <what you can't do and why> | <your proposed plan to fix it>\n\
         Only report genuine infrastructure/capability gaps, not user-specific requests. \
         Be specific and actionable in your proposed plan.",
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

/// Normalize a datetime string to a consistent format for dedup comparison.
///
/// Strips trailing `Z`, replaces `T` separator with space when not followed by
/// timezone info, so `2026-02-22T07:00:00Z` and `2026-02-22 07:00:00` match.
fn normalize_due_at(due_at: &str) -> String {
    let s = due_at.trim_end_matches('Z');
    s.replacen('T', " ", 1)
}

/// Check if two task descriptions are semantically similar via word overlap.
///
/// Extracts significant words (3+ chars, excluding stop words), returns true
/// if 50%+ of the smaller word set overlaps with the larger. Requires at least
/// 3 significant words in each description to avoid false positives on short text.
fn descriptions_are_similar(a: &str, b: &str) -> bool {
    let words_a = significant_words(a);
    let words_b = significant_words(b);

    // Require minimum 3 significant words to avoid false positives on short descriptions.
    if words_a.len() < 3 || words_b.len() < 3 {
        return false;
    }

    let (smaller, larger) = if words_a.len() <= words_b.len() {
        (&words_a, &words_b)
    } else {
        (&words_b, &words_a)
    };

    let overlap = smaller.iter().filter(|w| larger.contains(w)).count();
    let threshold = smaller.len().div_ceil(2);
    overlap >= threshold
}

/// Extract significant words from text (lowercase, 3+ chars, no stop words).
fn significant_words(text: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "the", "and", "for", "that", "this", "with", "from", "are", "was", "were", "been", "have",
        "has", "had", "will", "would", "could", "should", "may", "might", "can", "about", "into",
        "over", "after", "before", "between", "under", "again", "then", "once", "daily", "weekly",
        "monthly", "cada", "diario", "escribir", "enviar", "usar", "nunca", "siempre", "cada",
    ];
    text.split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 3 && !STOP_WORDS.contains(&w.as_str()))
        .collect()
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
                "reminder",
            )
            .await
            .unwrap();
        assert!(!id.is_empty());

        let tasks = store.get_tasks_for_sender("user1").await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].1, "Call John");
        assert_eq!(tasks[0].2, "2026-12-31 15:00:00");
        assert!(tasks[0].3.is_none());
        assert_eq!(tasks[0].4, "reminder");
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
                "reminder",
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
                "reminder",
            )
            .await
            .unwrap();

        let due = store.get_due_tasks().await.unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].4, "Past task");
        assert_eq!(due[0].6, "reminder");
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
                "reminder",
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
                "reminder",
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
                "reminder",
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
                "reminder",
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
    async fn test_update_task_description() {
        let store = test_store().await;
        let id = store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "Old desc",
                "2099-12-31T00:00:00",
                None,
                "reminder",
            )
            .await
            .unwrap();

        let prefix = &id[..8];
        let updated = store
            .update_task(prefix, "user1", Some("New desc"), None, None)
            .await
            .unwrap();
        assert!(updated);

        let tasks = store.get_tasks_for_sender("user1").await.unwrap();
        assert_eq!(tasks[0].1, "New desc");
    }

    #[tokio::test]
    async fn test_update_task_repeat() {
        let store = test_store().await;
        let id = store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "My task",
                "2099-12-31T00:00:00",
                Some("once"),
                "reminder",
            )
            .await
            .unwrap();

        let prefix = &id[..8];
        let updated = store
            .update_task(prefix, "user1", None, None, Some("daily"))
            .await
            .unwrap();
        assert!(updated);

        let tasks = store.get_tasks_for_sender("user1").await.unwrap();
        assert_eq!(tasks[0].3, Some("daily".to_string()));
    }

    #[tokio::test]
    async fn test_update_task_wrong_sender() {
        let store = test_store().await;
        let id = store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "My task",
                "2099-12-31T00:00:00",
                None,
                "reminder",
            )
            .await
            .unwrap();

        let prefix = &id[..8];
        let updated = store
            .update_task(prefix, "user2", Some("Hacked"), None, None)
            .await
            .unwrap();
        assert!(!updated);

        let tasks = store.get_tasks_for_sender("user1").await.unwrap();
        assert_eq!(tasks[0].1, "My task");
    }

    #[tokio::test]
    async fn test_update_task_no_fields() {
        let store = test_store().await;
        let id = store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "My task",
                "2099-12-31T00:00:00",
                None,
                "reminder",
            )
            .await
            .unwrap();

        let prefix = &id[..8];
        let updated = store
            .update_task(prefix, "user1", None, None, None)
            .await
            .unwrap();
        assert!(!updated);
    }

    #[tokio::test]
    async fn test_create_task_with_action_type() {
        let store = test_store().await;
        let id = store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "Check BTC price",
                "2026-12-31T14:00:00",
                Some("daily"),
                "action",
            )
            .await
            .unwrap();
        assert!(!id.is_empty());

        let tasks = store.get_tasks_for_sender("user1").await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].1, "Check BTC price");
        assert_eq!(tasks[0].4, "action");
    }

    #[tokio::test]
    async fn test_get_due_tasks_returns_task_type() {
        let store = test_store().await;
        store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "Reminder task",
                "2020-01-01T00:00:00",
                None,
                "reminder",
            )
            .await
            .unwrap();
        store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "Action task",
                "2020-01-01T00:00:00",
                None,
                "action",
            )
            .await
            .unwrap();

        let due = store.get_due_tasks().await.unwrap();
        assert_eq!(due.len(), 2);
        let reminder = due.iter().find(|t| t.4 == "Reminder task").unwrap();
        let action = due.iter().find(|t| t.4 == "Action task").unwrap();
        assert_eq!(reminder.6, "reminder");
        assert_eq!(action.6, "action");
    }

    #[tokio::test]
    async fn test_create_task_dedup() {
        let store = test_store().await;
        let id1 = store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "Close all positions",
                "2026-02-20T14:30:00",
                None,
                "action",
            )
            .await
            .unwrap();

        // Same sender + description + due_at → returns existing ID.
        let id2 = store
            .create_task(
                "telegram",
                "user1",
                "chat1",
                "Close all positions",
                "2026-02-20T14:30:00",
                None,
                "action",
            )
            .await
            .unwrap();
        assert_eq!(id1, id2);

        // Only one task exists.
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

    // --- Limitation tests ---

    #[tokio::test]
    async fn test_store_limitation_new() {
        let store = test_store().await;
        let is_new = store
            .store_limitation("No email", "Cannot send emails", "Add SMTP")
            .await
            .unwrap();
        assert!(is_new, "first insert should return true");
    }

    #[tokio::test]
    async fn test_store_limitation_duplicate() {
        let store = test_store().await;
        store
            .store_limitation("No email", "Cannot send emails", "Add SMTP")
            .await
            .unwrap();
        let is_new = store
            .store_limitation("No email", "Different desc", "Different plan")
            .await
            .unwrap();
        assert!(!is_new, "duplicate title should return false");
    }

    #[tokio::test]
    async fn test_store_limitation_case_insensitive() {
        let store = test_store().await;
        store
            .store_limitation("No Email", "Cannot send emails", "Add SMTP")
            .await
            .unwrap();
        let is_new = store
            .store_limitation("no email", "Different desc", "Different plan")
            .await
            .unwrap();
        assert!(
            !is_new,
            "case-insensitive duplicate title should return false"
        );
    }

    #[tokio::test]
    async fn test_get_open_limitations() {
        let store = test_store().await;
        store
            .store_limitation("No email", "Cannot send emails", "Add SMTP")
            .await
            .unwrap();
        store
            .store_limitation("No calendar", "Cannot access calendar", "Add Google Cal")
            .await
            .unwrap();

        let limitations = store.get_open_limitations().await.unwrap();
        assert_eq!(limitations.len(), 2);
        assert_eq!(limitations[0].0, "No email");
        assert_eq!(limitations[1].0, "No calendar");
    }

    // --- User profile tests ---

    #[test]
    fn test_user_profile_filters_system_facts() {
        let facts = vec![
            ("welcomed".to_string(), "true".to_string()),
            ("preferred_language".to_string(), "English".to_string()),
            ("active_project".to_string(), "omega".to_string()),
            ("name".to_string(), "Alice".to_string()),
        ];
        let profile = format_user_profile(&facts);
        assert!(profile.contains("name: Alice"));
        assert!(!profile.contains("welcomed"));
        assert!(!profile.contains("preferred_language"));
        assert!(!profile.contains("active_project"));
    }

    #[test]
    fn test_user_profile_groups_identity_first() {
        let facts = vec![
            ("timezone".to_string(), "EST".to_string()),
            ("interests".to_string(), "chess".to_string()),
            ("name".to_string(), "Alice".to_string()),
            ("pronouns".to_string(), "she/her".to_string()),
            ("occupation".to_string(), "engineer".to_string()),
        ];
        let profile = format_user_profile(&facts);
        let lines: Vec<&str> = profile.lines().collect();
        assert_eq!(lines[0], "User profile:");
        // Identity keys (name, pronouns) should come before context keys (timezone, occupation).
        let name_pos = lines.iter().position(|l| l.contains("name:")).unwrap();
        let pronouns_pos = lines.iter().position(|l| l.contains("pronouns:")).unwrap();
        let timezone_pos = lines.iter().position(|l| l.contains("timezone:")).unwrap();
        let occupation_pos = lines
            .iter()
            .position(|l| l.contains("occupation:"))
            .unwrap();
        let interests_pos = lines.iter().position(|l| l.contains("interests:")).unwrap();
        assert!(name_pos < timezone_pos);
        assert!(pronouns_pos < timezone_pos);
        assert!(timezone_pos < interests_pos);
        assert!(occupation_pos < interests_pos);
    }

    #[test]
    fn test_user_profile_empty_for_system_only() {
        let facts = vec![
            ("welcomed".to_string(), "true".to_string()),
            ("preferred_language".to_string(), "English".to_string()),
        ];
        let profile = format_user_profile(&facts);
        assert!(profile.is_empty());
    }

    // --- Onboarding hint tests ---

    #[test]
    fn test_build_system_prompt_shows_action_badge() {
        let facts = vec![
            ("welcomed".to_string(), "true".to_string()),
            ("preferred_language".to_string(), "English".to_string()),
            ("name".to_string(), "Alice".to_string()),
            ("occupation".to_string(), "engineer".to_string()),
            ("timezone".to_string(), "EST".to_string()),
        ];
        let tasks = vec![(
            "abcd1234-0000".to_string(),
            "Check BTC price".to_string(),
            "2026-02-18T14:00:00".to_string(),
            Some("daily".to_string()),
            "action".to_string(),
        )];
        let prompt = build_system_prompt("Rules", &facts, &[], &[], &tasks, "English", None);
        assert!(
            prompt.contains("[action]"),
            "should show [action] badge for action tasks"
        );
    }

    #[test]
    fn test_onboarding_stage0_first_conversation() {
        let facts = vec![
            ("welcomed".to_string(), "true".to_string()),
            ("preferred_language".to_string(), "Spanish".to_string()),
        ];
        let prompt = build_system_prompt("Rules", &facts, &[], &[], &[], "Spanish", Some(0));
        assert!(
            prompt.contains("first conversation"),
            "stage 0 should include first-conversation intro"
        );
    }

    #[test]
    fn test_onboarding_stage1_help_hint() {
        let facts = vec![
            ("welcomed".to_string(), "true".to_string()),
            ("preferred_language".to_string(), "English".to_string()),
            ("name".to_string(), "Alice".to_string()),
        ];
        let prompt = build_system_prompt("Rules", &facts, &[], &[], &[], "English", Some(1));
        assert!(
            prompt.contains("/help"),
            "stage 1 should mention /help command"
        );
    }

    #[test]
    fn test_onboarding_no_hint_when_none() {
        let facts = vec![
            ("welcomed".to_string(), "true".to_string()),
            ("preferred_language".to_string(), "English".to_string()),
            ("name".to_string(), "Alice".to_string()),
            ("occupation".to_string(), "engineer".to_string()),
            ("timezone".to_string(), "EST".to_string()),
        ];
        let prompt = build_system_prompt("Rules", &facts, &[], &[], &[], "English", None);
        assert!(
            !prompt.contains("Onboarding hint"),
            "should NOT include onboarding hint when None"
        );
        assert!(
            !prompt.contains("first conversation"),
            "should NOT include first-conversation intro when None"
        );
    }

    // --- compute_onboarding_stage tests ---

    #[test]
    fn test_compute_onboarding_stage_sequential() {
        // Stage 0 → 1 when 1+ real facts.
        assert_eq!(compute_onboarding_stage(0, 1, false), 1);
        // Stage 0 stays at 0 with no facts.
        assert_eq!(compute_onboarding_stage(0, 0, false), 0);
        // Stage 1 → 2 when 3+ real facts.
        assert_eq!(compute_onboarding_stage(1, 3, false), 2);
        // Stage 1 stays with only 2.
        assert_eq!(compute_onboarding_stage(1, 2, false), 1);
        // Stage 2 → 3 when has_tasks.
        assert_eq!(compute_onboarding_stage(2, 3, true), 3);
        // Stage 2 stays without tasks.
        assert_eq!(compute_onboarding_stage(2, 3, false), 2);
        // Stage 3 → 4 when 5+ real facts.
        assert_eq!(compute_onboarding_stage(3, 5, true), 4);
        // Stage 3 stays with 4 facts.
        assert_eq!(compute_onboarding_stage(3, 4, true), 3);
        // Stage 4 → 5 always.
        assert_eq!(compute_onboarding_stage(4, 5, true), 5);
        // Stage 5 stays done.
        assert_eq!(compute_onboarding_stage(5, 10, true), 5);
    }

    #[test]
    fn test_compute_onboarding_stage_no_skip() {
        // Even with many facts, can't skip from 0 to 2.
        assert_eq!(compute_onboarding_stage(0, 10, true), 1);
    }

    #[test]
    fn test_onboarding_hint_text_contains_commands() {
        // Stage 1 mentions /help.
        let hint1 = onboarding_hint_text(1, "English").unwrap();
        assert!(hint1.contains("/help"));
        // Stage 2 mentions /personality.
        let hint2 = onboarding_hint_text(2, "English").unwrap();
        assert!(hint2.contains("/personality"));
        // Stage 3 mentions /tasks.
        let hint3 = onboarding_hint_text(3, "English").unwrap();
        assert!(hint3.contains("/tasks"));
        // Stage 4 mentions /projects.
        let hint4 = onboarding_hint_text(4, "English").unwrap();
        assert!(hint4.contains("/projects"));
        // Stage 5 returns None.
        assert!(onboarding_hint_text(5, "English").is_none());
    }

    #[test]
    fn test_onboarding_hint_text_includes_language() {
        // Stage 0: should contain language name, no hardcoded '¡Hola'.
        let hint0 = onboarding_hint_text(0, "French").unwrap();
        assert!(
            hint0.contains("French"),
            "stage 0 should reference the language"
        );
        assert!(
            !hint0.contains("¡Hola"),
            "stage 0 should not have hardcoded Spanish greeting"
        );

        // Stages 1-4: should contain "Respond in {language}".
        for stage in 1..=4 {
            let hint = onboarding_hint_text(stage, "German").unwrap();
            assert!(
                hint.contains("Respond in German"),
                "stage {stage} should contain 'Respond in German'"
            );
        }
    }

    #[tokio::test]
    async fn test_build_context_advances_onboarding_stage() {
        let store = test_store().await;
        let sender = "onboard_user";

        // First contact: no facts at all → should show stage 0 (first conversation).
        let msg = IncomingMessage {
            id: uuid::Uuid::new_v4(),
            channel: "telegram".to_string(),
            sender_id: sender.to_string(),
            sender_name: None,
            text: "hello".to_string(),
            timestamp: chrono::Utc::now(),
            reply_to: None,
            attachments: vec![],
            reply_target: Some("chat1".to_string()),
            is_group: false,
        };
        let ctx = store.build_context(&msg, "Base rules").await.unwrap();
        assert!(
            ctx.system_prompt.contains("first conversation"),
            "first contact should trigger stage 0 intro"
        );

        // Store a real fact (simulating the AI learned the user's name).
        store.store_fact(sender, "welcomed", "true").await.unwrap();
        store.store_fact(sender, "name", "Alice").await.unwrap();

        // Second message: should advance to stage 1 and show /help hint.
        let ctx2 = store.build_context(&msg, "Base rules").await.unwrap();
        assert!(
            ctx2.system_prompt.contains("/help"),
            "after learning name, should show stage 1 /help hint"
        );

        // Third message: stage already at 1, no new transition → no hint.
        let ctx3 = store.build_context(&msg, "Base rules").await.unwrap();
        assert!(
            !ctx3.system_prompt.contains("Onboarding hint"),
            "no hint when stage hasn't changed"
        );
    }

    // --- User alias tests ---

    #[tokio::test]
    async fn test_resolve_sender_id_no_alias() {
        let store = test_store().await;
        // No alias → returns original.
        let resolved = store.resolve_sender_id("phone123").await.unwrap();
        assert_eq!(resolved, "phone123");
    }

    #[tokio::test]
    async fn test_create_and_resolve_alias() {
        let store = test_store().await;
        store.create_alias("phone123", "telegram456").await.unwrap();
        let resolved = store.resolve_sender_id("phone123").await.unwrap();
        assert_eq!(resolved, "telegram456");
    }

    #[tokio::test]
    async fn test_create_alias_idempotent() {
        let store = test_store().await;
        store.create_alias("phone123", "telegram456").await.unwrap();
        // Second insert is a no-op (INSERT OR IGNORE).
        store.create_alias("phone123", "telegram456").await.unwrap();
        let resolved = store.resolve_sender_id("phone123").await.unwrap();
        assert_eq!(resolved, "telegram456");
    }

    #[tokio::test]
    async fn test_find_canonical_user() {
        let store = test_store().await;
        // No users → None.
        assert!(store
            .find_canonical_user("new_user")
            .await
            .unwrap()
            .is_none());

        // Add an existing welcomed user.
        store
            .store_fact("telegram456", "welcomed", "true")
            .await
            .unwrap();

        // find_canonical_user from a different sender → returns the existing one.
        let canonical = store
            .find_canonical_user("phone123")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(canonical, "telegram456");

        // Excluding the existing user → None.
        assert!(store
            .find_canonical_user("telegram456")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn test_alias_shares_facts() {
        let store = test_store().await;
        // Store facts under canonical ID.
        store
            .store_fact("telegram456", "name", "Alice")
            .await
            .unwrap();
        store
            .store_fact("telegram456", "welcomed", "true")
            .await
            .unwrap();

        // Create alias.
        store.create_alias("phone123", "telegram456").await.unwrap();

        // Resolve alias and read facts using canonical ID.
        let resolved = store.resolve_sender_id("phone123").await.unwrap();
        let facts = store.get_facts(&resolved).await.unwrap();
        assert!(facts.iter().any(|(k, v)| k == "name" && v == "Alice"));
    }

    #[test]
    fn test_normalize_due_at_strips_z() {
        assert_eq!(
            normalize_due_at("2026-02-22T07:00:00Z"),
            "2026-02-22 07:00:00"
        );
    }

    #[test]
    fn test_normalize_due_at_replaces_t() {
        assert_eq!(
            normalize_due_at("2026-02-22T07:00:00"),
            "2026-02-22 07:00:00"
        );
    }

    #[test]
    fn test_normalize_due_at_already_normalized() {
        assert_eq!(
            normalize_due_at("2026-02-22 07:00:00"),
            "2026-02-22 07:00:00"
        );
    }

    #[test]
    fn test_descriptions_similar_email_variants() {
        assert!(descriptions_are_similar(
            "Enviar email de amor diario a Adriana (adri_navega@hotmail.com)",
            "Enviar email de amor diario a Adriana (adri_navega@hotmail.com) — escribir un mensaje"
        ));
    }

    #[test]
    fn test_descriptions_similar_hostinger() {
        assert!(descriptions_are_similar(
            "Cancel Hostinger plan — expires March 17",
            "Cancel Hostinger VPS — last reminder, expires TOMORROW"
        ));
    }

    #[test]
    fn test_descriptions_different() {
        assert!(!descriptions_are_similar(
            "Send good morning message to the team",
            "Cancel Hostinger plan and subscription"
        ));
    }

    #[test]
    fn test_descriptions_short_skipped() {
        // Short descriptions (< 3 significant words) skip fuzzy matching.
        assert!(!descriptions_are_similar("Reminder task", "Action task"));
    }

    #[tokio::test]
    async fn test_create_task_fuzzy_dedup() {
        let store = test_store().await;

        // Create first task.
        let id1 = store
            .create_task(
                "telegram",
                "user1",
                "reply1",
                "Send daily email to Adriana",
                "2026-02-22 07:00:00",
                Some("daily"),
                "action",
            )
            .await
            .unwrap();

        // Same task with different datetime format — should dedup.
        let id2 = store
            .create_task(
                "telegram",
                "user1",
                "reply1",
                "Send daily email to Adriana",
                "2026-02-22T07:00:00Z",
                Some("daily"),
                "action",
            )
            .await
            .unwrap();
        assert_eq!(id1, id2, "exact dedup with normalized datetime");

        // Similar description, same time window — should fuzzy dedup.
        let id3 = store
            .create_task(
                "telegram",
                "user1",
                "reply1",
                "Send daily love email to Adriana via gmail",
                "2026-02-22 07:05:00",
                Some("daily"),
                "action",
            )
            .await
            .unwrap();
        assert_eq!(id1, id3, "fuzzy dedup: similar description within 30min");

        // Different sender — should NOT dedup.
        let id4 = store
            .create_task(
                "telegram",
                "user2",
                "reply2",
                "Send daily email to Adriana",
                "2026-02-22 07:00:00",
                Some("daily"),
                "action",
            )
            .await
            .unwrap();
        assert_ne!(id1, id4, "different sender should create new task");
    }
}
