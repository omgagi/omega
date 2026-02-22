//! Conversation lifecycle — create, find, close, summaries, history, stats.

use super::{Store, CONVERSATION_TIMEOUT_MINUTES};
use omega_core::error::OmegaError;
use uuid::Uuid;

impl Store {
    /// Get or create an active conversation for a given channel + sender.
    ///
    /// Only returns conversations that are `active` AND have `last_activity`
    /// within the timeout window. Otherwise creates a new one.
    pub(crate) async fn get_or_create_conversation(
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
}
